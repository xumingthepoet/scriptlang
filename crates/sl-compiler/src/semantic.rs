use std::collections::BTreeSet;

use sl_core::{Form, ScriptLangError, TextSegment, TextTemplate};

use crate::const_eval::{
    ConstEnv, ConstValue, parse_const_value, rewrite_expr_with_consts, rewrite_template_with_consts,
};
use crate::form::{attr, child_forms, error_at, required_attr, trimmed_text_items};
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticProgram {
    pub(crate) modules: Vec<SemanticModule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticModule {
    pub(crate) name: String,
    pub(crate) consts: Vec<SemanticConst>,
    pub(crate) vars: Vec<SemanticVar>,
    pub(crate) scripts: Vec<SemanticScript>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticConst {
    pub(crate) name: String,
    pub(crate) value: ConstValue,
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
    let module_children = child_forms(form)?;
    let future_const_names = module_children
        .iter()
        .filter(|child| child.head == "const")
        .map(|child| required_attr(child, "name").map(str::to_string))
        .collect::<Result<BTreeSet<_>, _>>()?;

    let mut remaining_const_names = future_const_names;
    let mut const_env = ConstEnv::new();
    let mut consts = Vec::new();
    let mut vars = Vec::new();
    let mut scripts = Vec::new();

    for child in module_children {
        match child.head.as_str() {
            "const" => {
                let semantic_const = analyze_const(child, &const_env, &remaining_const_names)?;
                remaining_const_names.remove(&semantic_const.name);
                const_env.insert(semantic_const.name.clone(), semantic_const.value.clone());
                consts.push(semantic_const);
            }
            "var" => vars.push(SemanticVar {
                name: required_attr(child, "name")?.to_string(),
                expr: rewrite_expr_with_consts(
                    &trimmed_text_items(child)?,
                    &const_env,
                    &remaining_const_names,
                    &BTreeSet::new(),
                )?,
            }),
            "script" => scripts.push(analyze_script(child, &const_env, &remaining_const_names)?),
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
        consts,
        vars,
        scripts,
    })
}

fn analyze_const(
    form: &Form,
    env: &ConstEnv,
    remaining_const_names: &BTreeSet<String>,
) -> Result<SemanticConst, ScriptLangError> {
    let name = required_attr(form, "name")?.to_string();
    let raw = trimmed_text_items(form)?;
    let mut blocked = remaining_const_names.clone();
    blocked.remove(&name);
    let value = parse_const_value(&raw, env, &blocked)?;
    Ok(SemanticConst { name, value })
}

fn analyze_script(
    form: &Form,
    const_env: &ConstEnv,
    remaining_const_names: &BTreeSet<String>,
) -> Result<SemanticScript, ScriptLangError> {
    let mut shadowed_names = BTreeSet::new();
    Ok(SemanticScript {
        name: required_attr(form, "name")?.to_string(),
        body: analyze_block(
            &child_forms(form)?,
            const_env,
            remaining_const_names,
            &mut shadowed_names,
        )?,
    })
}

fn analyze_block(
    forms: &[&Form],
    const_env: &ConstEnv,
    remaining_const_names: &BTreeSet<String>,
    shadowed_names: &mut BTreeSet<String>,
) -> Result<Vec<SemanticStmt>, ScriptLangError> {
    let mut body = Vec::with_capacity(forms.len());
    for form in forms {
        let stmt = analyze_stmt(form, const_env, remaining_const_names, shadowed_names)?;
        if let SemanticStmt::Temp { name, .. } = &stmt {
            shadowed_names.insert(name.clone());
        }
        body.push(stmt);
    }
    Ok(body)
}

fn analyze_stmt(
    form: &Form,
    const_env: &ConstEnv,
    remaining_const_names: &BTreeSet<String>,
    shadowed_names: &mut BTreeSet<String>,
) -> Result<SemanticStmt, ScriptLangError> {
    match form.head.as_str() {
        "const" => Err(error_at(
            form,
            "<const> is only supported as a direct <module> child in MVP",
        )),
        "temp" => Ok(SemanticStmt::Temp {
            name: required_attr(form, "name")?.to_string(),
            expr: rewrite_expr_with_consts(
                &trimmed_text_items(form)?,
                const_env,
                remaining_const_names,
                shadowed_names,
            )?,
        }),
        "code" => Ok(SemanticStmt::Code {
            code: rewrite_expr_with_consts(
                &trimmed_text_items(form)?,
                const_env,
                remaining_const_names,
                shadowed_names,
            )?,
        }),
        "text" => Ok(SemanticStmt::Text {
            template: rewrite_template_with_consts(
                parse_text_template(&trimmed_text_items(form)?),
                const_env,
                remaining_const_names,
                shadowed_names,
            )?,
            tag: attr(form, "tag").map(str::to_string),
        }),
        "if" => Ok(SemanticStmt::If {
            when: rewrite_expr_with_consts(
                required_attr(form, "when")?,
                const_env,
                remaining_const_names,
                shadowed_names,
            )?,
            body: analyze_block(
                &child_forms(form)?,
                const_env,
                remaining_const_names,
                shadowed_names,
            )?,
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
                    text: rewrite_template_with_consts(
                        parse_text_template(required_attr(option, "text")?),
                        const_env,
                        remaining_const_names,
                        shadowed_names,
                    )?,
                    body: analyze_block(
                        &child_forms(option)?,
                        const_env,
                        remaining_const_names,
                        shadowed_names,
                    )?,
                });
            }
            Ok(SemanticStmt::Choice {
                prompt: attr(form, "text")
                    .map(parse_text_template)
                    .map(|template| {
                        rewrite_template_with_consts(
                            template,
                            const_env,
                            remaining_const_names,
                            shadowed_names,
                        )
                    })
                    .transpose()?,
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

fn parse_text_template(source: &str) -> TextTemplate {
    let mut segments = Vec::new();
    let mut cursor = 0usize;

    while let Some(start_offset) = source[cursor..].find("${") {
        let start = cursor + start_offset;
        if start > cursor {
            segments.push(TextSegment::Literal(source[cursor..start].to_string()));
        }

        let expr_start = start + 2;
        let Some(end_offset) = source[expr_start..].find('}') else {
            if let Some(TextSegment::Literal(prefix)) = segments.last_mut() {
                prefix.push_str(&source[start..]);
            } else {
                segments.push(TextSegment::Literal(source[start..].to_string()));
            }
            cursor = source.len();
            break;
        };
        let expr_end = expr_start + end_offset;
        segments.push(TextSegment::Expr(
            source[expr_start..expr_end].trim().to_string(),
        ));
        cursor = expr_end + 1;
    }

    if cursor < source.len() {
        segments.push(TextSegment::Literal(source[cursor..].to_string()));
    }
    if segments.is_empty() {
        segments.push(TextSegment::Literal(source.to_string()));
    }

    TextTemplate { segments }
}

#[cfg(test)]
mod tests {
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition, TextSegment};

    use crate::const_eval::ConstValue;

    use super::{SemanticStmt, analyze_forms, parse_text_template};

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
                child(node("const", vec![("name", "bonus")], vec![text("41")])),
                child(node(
                    "var",
                    vec![("name", "answer")],
                    vec![text("bonus + 1")],
                )),
                child(node(
                    "script",
                    vec![("name", "entry")],
                    vec![
                        child(node("temp", vec![("name", "x")], vec![text("bonus")])),
                        child(node(
                            "text",
                            vec![("tag", "line")],
                            vec![text("hello ${bonus}")],
                        )),
                        child(node(
                            "choice",
                            vec![("text", "pick ${bonus}")],
                            vec![child(node(
                                "option",
                                vec![("text", "left ${bonus}")],
                                vec![child(node("end", vec![], vec![]))],
                            ))],
                        )),
                    ],
                )),
            ],
        )])
        .expect("analyze");

        assert_eq!(program.modules.len(), 1);
        assert_eq!(program.modules[0].consts.len(), 1);
        assert_eq!(program.modules[0].consts[0].name, "bonus");
        assert_eq!(program.modules[0].consts[0].value, ConstValue::Integer(41));
        assert_eq!(program.modules[0].vars[0].expr, "41 + 1");
        assert!(matches!(
            &program.modules[0].scripts[0].body[0],
            SemanticStmt::Temp { name, expr } if name == "x" && expr == "41"
        ));
        assert!(matches!(
            &program.modules[0].scripts[0].body[1],
            SemanticStmt::Text { template, tag }
                if matches!(&template.segments[..], [TextSegment::Literal(_), TextSegment::Expr(expr)] if expr == "41")
                    && tag.as_deref() == Some("line")
        ));
    }

    #[test]
    fn analyze_forms_supports_const_refs_and_shadowing() {
        let program = analyze_forms(&[node(
            "module",
            vec![("name", "main")],
            vec![
                child(node("const", vec![("name", "base")], vec![text("1")])),
                child(node(
                    "const",
                    vec![("name", "answer")],
                    vec![text("#{value: base}")],
                )),
                child(node(
                    "script",
                    vec![("name", "entry")],
                    vec![
                        child(node("temp", vec![("name", "answer")], vec![text("2")])),
                        child(node("code", vec![], vec![text("answer += base;")])),
                        child(node("text", vec![], vec![text("${answer}")])),
                    ],
                )),
            ],
        )])
        .expect("analyze");

        assert_eq!(
            program.modules[0].consts[1].value,
            ConstValue::Object(std::collections::BTreeMap::from([(
                "value".to_string(),
                ConstValue::Integer(1)
            )]))
        );
        assert!(matches!(
            &program.modules[0].scripts[0].body[1],
            SemanticStmt::Code { code } if code == "answer += 1;"
        ));
        assert!(matches!(
            &program.modules[0].scripts[0].body[2],
            SemanticStmt::Text { template, .. }
                if matches!(&template.segments[..], [TextSegment::Expr(expr)] if expr == "answer")
        ));
    }

    #[test]
    fn analyze_forms_rejects_invalid_const_usage() {
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

        let nested_const = analyze_forms(&[node(
            "module",
            vec![("name", "main")],
            vec![child(node(
                "script",
                vec![("name", "entry")],
                vec![child(node("const", vec![("name", "x")], vec![text("1")]))],
            ))],
        )])
        .expect_err("nested const should fail");
        assert!(
            nested_const
                .to_string()
                .contains("<const> is only supported as a direct <module> child")
        );

        let forward_ref = analyze_forms(&[node(
            "module",
            vec![("name", "main")],
            vec![
                child(node("const", vec![("name", "x")], vec![text("later")])),
                child(node("const", vec![("name", "later")], vec![text("1")])),
            ],
        )])
        .expect_err("forward ref should fail");
        assert!(
            forward_ref
                .to_string()
                .contains("cannot be referenced before it is defined")
        );

        let unsupported = analyze_forms(&[node(
            "module",
            vec![("name", "main")],
            vec![child(node(
                "const",
                vec![("name", "x")],
                vec![text("call()")],
            ))],
        )])
        .expect_err("call should fail");
        assert!(
            unsupported
                .to_string()
                .contains("unsupported const reference `call`")
        );
    }

    #[test]
    fn parse_text_template_covers_literal_and_expression_shapes() {
        let empty = parse_text_template("");
        assert_eq!(empty.segments.len(), 1);
        assert!(matches!(&empty.segments[0], TextSegment::Literal(text) if text.is_empty()));

        let literal = parse_text_template("hello");
        assert!(matches!(&literal.segments[..], [TextSegment::Literal(text)] if text == "hello"));

        let expr_only = parse_text_template("${ value }");
        assert!(matches!(&expr_only.segments[..], [TextSegment::Expr(text)] if text == "value"));

        let unclosed = parse_text_template("hello ${name");
        assert!(
            matches!(&unclosed.segments[..], [TextSegment::Literal(text)] if text == "hello ${name")
        );

        let mixed = parse_text_template("a ${left} b ${ } c");
        assert_eq!(mixed.segments.len(), 5);
        assert!(matches!(&mixed.segments[0], TextSegment::Literal(text) if text == "a "));
        assert!(matches!(&mixed.segments[1], TextSegment::Expr(text) if text == "left"));
        assert!(matches!(&mixed.segments[2], TextSegment::Literal(text) if text == " b "));
        assert!(matches!(&mixed.segments[3], TextSegment::Expr(text) if text.is_empty()));
        assert!(matches!(&mixed.segments[4], TextSegment::Literal(text) if text == " c"));
    }
}
