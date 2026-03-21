use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, ScriptLangError};

const CHILDREN_FIELD: &str = "children";

pub(crate) fn required_attr<'a>(form: &'a Form, name: &str) -> Result<&'a str, ScriptLangError> {
    attr(form, name).ok_or_else(|| error_at(form, format!("<{}> requires `{name}`", form.head)))
}

pub(crate) fn attr<'a>(form: &'a Form, name: &str) -> Option<&'a str> {
    field(form, name).and_then(|field| match &field.value {
        FormValue::String(value) => Some(value.as_str()),
        FormValue::Sequence(_) => None,
    })
}

pub(crate) fn body_expr(form: &Form) -> Result<String, ScriptLangError> {
    if !matches!(form.head.as_str(), "var" | "temp" | "const" | "code") {
        return Err(error_at(
            form,
            format!("<{}> body is not classified as an expression", form.head),
        ));
    }
    trimmed_text_items(form)
}

pub(crate) fn body_template(form: &Form) -> Result<String, ScriptLangError> {
    if form.head != "text" {
        return Err(error_at(
            form,
            format!("<{}> body is not classified as a template", form.head),
        ));
    }
    trimmed_text_items(form)
}

pub(crate) fn child_forms(form: &Form) -> Result<Vec<&Form>, ScriptLangError> {
    if matches!(
        form.head.as_str(),
        "var" | "temp" | "const" | "code" | "text"
    ) {
        return Err(error_at(
            form,
            format!("<{}> body does not support nested statements", form.head),
        ));
    }

    let mut children = Vec::new();
    for item in children_items(form)? {
        match item {
            FormItem::Form(node) => children.push(node),
            FormItem::Text(text) if text.trim().is_empty() => {}
            FormItem::Text(_) => {
                return Err(error_at(
                    form,
                    format!("<{}> does not support text items in MVP", form.head),
                ));
            }
        }
    }
    Ok(children)
}

pub(crate) fn trimmed_text_items(form: &Form) -> Result<String, ScriptLangError> {
    let mut text = String::new();
    for item in children_items(form)? {
        match item {
            FormItem::Text(segment) => text.push_str(segment),
            FormItem::Form(node) => {
                return Err(error_at(
                    node,
                    format!(
                        "nested <{}> is not supported inside <{}> in MVP",
                        node.head, form.head
                    ),
                ));
            }
        }
    }
    Ok(text.trim().to_string())
}

pub(crate) fn location(meta: &FormMeta) -> String {
    match &meta.source_name {
        Some(source_name) => format!("{source_name}:{}:{}", meta.start.row, meta.start.column),
        None => format!("{}:{}", meta.start.row, meta.start.column),
    }
}

pub(crate) fn error_at(form: &Form, message: impl Into<String>) -> ScriptLangError {
    ScriptLangError::message(format!("{} at {}", message.into(), location(&form.meta)))
}

fn children_items(form: &Form) -> Result<&[FormItem], ScriptLangError> {
    match field(form, CHILDREN_FIELD) {
        Some(FormField {
            value: FormValue::Sequence(items),
            ..
        }) => Ok(items),
        Some(FormField {
            value: FormValue::String(_),
            ..
        }) => Err(error_at(
            form,
            format!("<{}> has invalid `{CHILDREN_FIELD}` field shape", form.head),
        )),
        None => Err(error_at(
            form,
            format!("<{}> is missing `{CHILDREN_FIELD}` field", form.head),
        )),
    }
}

fn field<'a>(form: &'a Form, name: &str) -> Option<&'a FormField> {
    form.fields.iter().find(|field| field.name == name)
}

#[cfg(test)]
mod tests {
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use super::{
        attr, body_expr, body_template, child_forms, location, required_attr, trimmed_text_items,
    };

    fn form(head: &str, fields: Vec<FormField>) -> Form {
        Form {
            head: head.to_string(),
            meta: FormMeta {
                source_name: Some("main.xml".to_string()),
                start: SourcePosition { row: 1, column: 1 },
                end: SourcePosition { row: 1, column: 20 },
                start_byte: 0,
                end_byte: 20,
            },
            fields,
        }
    }

    #[test]
    fn raw_form_helpers_read_attributes_and_supported_bodies() {
        let module = form(
            "module",
            vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("main".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Form(form(
                        "script",
                        vec![FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(Vec::new()),
                        }],
                    ))]),
                },
            ],
        );
        let var = form(
            "var",
            vec![
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Text("@loop".to_string())]),
                },
                FormField {
                    name: "type".to_string(),
                    value: FormValue::String("script".to_string()),
                },
            ],
        );
        let text = form(
            "text",
            vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Text("${next}".to_string())]),
            }],
        );

        assert_eq!(attr(&module, "name"), Some("main"));
        assert_eq!(required_attr(&module, "name").expect("required"), "main");
        assert_eq!(child_forms(&module).expect("children")[0].head, "script");
        assert_eq!(body_expr(&var).expect("expr"), "@loop");
        assert_eq!(body_template(&text).expect("template"), "${next}");
    }

    #[test]
    fn raw_form_helpers_reject_invalid_shapes() {
        let script = form(
            "script",
            vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Text("bad".to_string())]),
            }],
        );
        let temp = form(
            "temp",
            vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(form(
                    "inner",
                    vec![FormField {
                        name: "children".to_string(),
                        value: FormValue::Sequence(Vec::new()),
                    }],
                ))]),
            }],
        );

        assert!(
            required_attr(&script, "name")
                .expect_err("missing attr")
                .to_string()
                .contains("<script> requires `name`")
        );
        assert!(
            child_forms(&script)
                .expect_err("unexpected text")
                .to_string()
                .contains("does not support text items")
        );
        assert!(
            trimmed_text_items(&temp)
                .expect_err("nested form")
                .to_string()
                .contains("nested <inner> is not supported")
        );
    }

    #[test]
    fn raw_form_helpers_cover_additional_error_paths() {
        let seq_attr_form = form(
            "module",
            vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::Sequence(Vec::new()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(Vec::new()),
                },
            ],
        );
        let wrong_head = form(
            "script",
            vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Text("x".to_string())]),
            }],
        );
        let missing_children = form("script", Vec::new());
        let malformed_children = form(
            "script",
            vec![FormField {
                name: "children".to_string(),
                value: FormValue::String("bad".to_string()),
            }],
        );
        let inline_meta = FormMeta {
            source_name: None,
            start: SourcePosition { row: 3, column: 5 },
            end: SourcePosition { row: 3, column: 9 },
            start_byte: 0,
            end_byte: 4,
        };

        assert_eq!(attr(&seq_attr_form, "name"), None);
        assert!(
            body_expr(&wrong_head)
                .expect_err("wrong head")
                .to_string()
                .contains("body is not classified as an expression")
        );
        assert!(
            body_template(&wrong_head)
                .expect_err("wrong head")
                .to_string()
                .contains("body is not classified as a template")
        );
        assert!(
            child_forms(&missing_children)
                .expect_err("missing children")
                .to_string()
                .contains("missing `children` field")
        );
        assert!(
            trimmed_text_items(&malformed_children)
                .expect_err("malformed children")
                .to_string()
                .contains("invalid `children` field shape")
        );
        assert_eq!(location(&inline_meta), "3:5");
    }
}
