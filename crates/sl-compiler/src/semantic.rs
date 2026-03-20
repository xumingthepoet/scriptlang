use sl_core::{Form, ScriptLangError, TextTemplate};

use crate::form::{attr, child_forms, error_at, required_attr, trimmed_text_items};
use crate::text::parse_text_template;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticProgram {
    pub(crate) modules: Vec<SemanticModule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticModule {
    pub(crate) name: String,
    pub(crate) vars: Vec<SemanticVar>,
    pub(crate) scripts: Vec<SemanticScript>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticVar {
    pub(crate) name: String,
    pub(crate) expr: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticScript {
    pub(crate) name: String,
    pub(crate) body: Vec<SemanticStmt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SemanticStmt {
    Temp {
        name: String,
        expr: String,
    },
    Code {
        code: String,
    },
    Text {
        template: TextTemplate,
        tag: Option<String>,
    },
    If {
        when: String,
        body: Vec<SemanticStmt>,
    },
    Choice {
        prompt: Option<TextTemplate>,
        options: Vec<SemanticChoiceOption>,
    },
    Goto {
        target_script_ref: String,
    },
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticChoiceOption {
    pub(crate) text: TextTemplate,
    pub(crate) body: Vec<SemanticStmt>,
}

pub(crate) fn analyze_forms(forms: &[Form]) -> Result<SemanticProgram, ScriptLangError> {
    let modules = forms
        .iter()
        .map(analyze_module)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SemanticProgram { modules })
}

fn analyze_module(form: &Form) -> Result<SemanticModule, ScriptLangError> {
    if form.head != "module" {
        return Err(error_at(
            form,
            format!("top-level <{}> is not supported in MVP", form.head),
        ));
    }

    let name = required_attr(form, "name")?.to_string();
    let mut vars = Vec::new();
    let mut scripts = Vec::new();

    for child in child_forms(form)? {
        match child.head.as_str() {
            "var" => vars.push(SemanticVar {
                name: required_attr(child, "name")?.to_string(),
                expr: trimmed_text_items(child)?,
            }),
            "script" => scripts.push(analyze_script(child)?),
            other => {
                return Err(error_at(
                    child,
                    format!("unsupported <module> child <{other}> in MVP"),
                ));
            }
        }
    }

    Ok(SemanticModule {
        name,
        vars,
        scripts,
    })
}

fn analyze_script(form: &Form) -> Result<SemanticScript, ScriptLangError> {
    Ok(SemanticScript {
        name: required_attr(form, "name")?.to_string(),
        body: analyze_block(&child_forms(form)?)?,
    })
}

fn analyze_block(forms: &[&Form]) -> Result<Vec<SemanticStmt>, ScriptLangError> {
    forms.iter().map(|form| analyze_stmt(form)).collect()
}

fn analyze_stmt(form: &Form) -> Result<SemanticStmt, ScriptLangError> {
    match form.head.as_str() {
        "temp" => Ok(SemanticStmt::Temp {
            name: required_attr(form, "name")?.to_string(),
            expr: trimmed_text_items(form)?,
        }),
        "code" => Ok(SemanticStmt::Code {
            code: trimmed_text_items(form)?,
        }),
        "text" => Ok(SemanticStmt::Text {
            template: parse_text_template(&trimmed_text_items(form)?),
            tag: attr(form, "tag").map(str::to_string),
        }),
        "if" => Ok(SemanticStmt::If {
            when: required_attr(form, "when")?.to_string(),
            body: analyze_block(&child_forms(form)?)?,
        }),
        "choice" => {
            let mut options = Vec::new();
            for option in child_forms(form)? {
                if option.head != "option" {
                    return Err(error_at(
                        option,
                        format!(
                            "<choice> only supports <option> children in MVP, got <{}>",
                            option.head
                        ),
                    ));
                }
                options.push(SemanticChoiceOption {
                    text: parse_text_template(required_attr(option, "text")?),
                    body: analyze_block(&child_forms(option)?)?,
                });
            }
            Ok(SemanticStmt::Choice {
                prompt: attr(form, "text").map(parse_text_template),
                options,
            })
        }
        "goto" => Ok(SemanticStmt::Goto {
            target_script_ref: required_attr(form, "script")?.to_string(),
        }),
        "end" => Ok(SemanticStmt::End),
        other => Err(error_at(
            form,
            format!("unsupported statement <{other}> in MVP"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition, TextSegment};

    use super::{SemanticStmt, analyze_forms};

    fn meta() -> FormMeta {
        FormMeta {
            source_name: Some("main.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 50 },
            start_byte: 0,
            end_byte: 50,
        }
    }

    fn form(head: &str, fields: Vec<FormField>) -> Form {
        Form {
            head: head.to_string(),
            meta: meta(),
            fields,
        }
    }

    fn attr(name: &str, value: &str) -> FormField {
        FormField {
            name: name.to_string(),
            value: FormValue::String(value.to_string()),
        }
    }

    fn children(items: Vec<FormItem>) -> FormField {
        FormField {
            name: "children".to_string(),
            value: FormValue::Sequence(items),
        }
    }

    fn node(head: &str, attrs: Vec<(&str, &str)>, items: Vec<FormItem>) -> Form {
        let mut fields = attrs
            .into_iter()
            .map(|(k, v)| attr(k, v))
            .collect::<Vec<_>>();
        fields.push(children(items));
        form(head, fields)
    }

    fn text(text: &str) -> FormItem {
        FormItem::Text(text.to_string())
    }

    fn child(form: Form) -> FormItem {
        FormItem::Form(form)
    }

    #[test]
    fn analyze_forms_converts_form_tree_into_semantic_program() {
        let program = analyze_forms(&[node(
            "module",
            vec![("name", "main")],
            vec![
                child(node("var", vec![("name", "answer")], vec![text("40 + 2")])),
                child(node(
                    "script",
                    vec![("name", "entry")],
                    vec![
                        child(node("temp", vec![("name", "x")], vec![text("1")])),
                        child(node(
                            "text",
                            vec![("tag", "line")],
                            vec![text("hello ${x}")],
                        )),
                        child(node(
                            "choice",
                            vec![("text", "pick")],
                            vec![child(node(
                                "option",
                                vec![("text", "left")],
                                vec![child(node("end", vec![], vec![]))],
                            ))],
                        )),
                    ],
                )),
            ],
        )])
        .expect("analyze");

        assert_eq!(program.modules.len(), 1);
        assert_eq!(program.modules[0].vars[0].name, "answer");
        assert!(matches!(
            &program.modules[0].scripts[0].body[0],
            SemanticStmt::Temp { name, expr } if name == "x" && expr == "1"
        ));
        assert!(matches!(
            &program.modules[0].scripts[0].body[1],
            SemanticStmt::Text { template, tag }
                if matches!(&template.segments[..], [TextSegment::Literal(_), TextSegment::Expr(expr)] if expr == "x")
                    && tag.as_deref() == Some("line")
        ));
    }

    #[test]
    fn analyze_forms_rejects_unsupported_shapes() {
        let top_level = analyze_forms(&[node("script", vec![("name", "entry")], vec![])])
            .expect_err("top-level should fail");
        assert!(
            top_level
                .to_string()
                .contains("top-level <script> is not supported in MVP")
        );

        let bad_child = analyze_forms(&[node(
            "module",
            vec![("name", "main")],
            vec![child(node("while", vec![], vec![]))],
        )])
        .expect_err("bad child should fail");
        assert!(
            bad_child
                .to_string()
                .contains("unsupported <module> child <while> in MVP")
        );
    }
}
