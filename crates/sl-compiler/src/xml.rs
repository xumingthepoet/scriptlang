use sl_core::{ScriptLangError, XmlContentItem, XmlField, XmlForm, XmlMeta, XmlValue};

pub(crate) fn required_attr<'a>(form: &'a XmlForm, name: &str) -> Result<&'a str, ScriptLangError> {
    match attr(form, name) {
        Some(value) => Ok(value),
        None => Err(error_at(form, format!("<{}> requires `{name}`", form.tag))),
    }
}

pub(crate) fn attr<'a>(form: &'a XmlForm, name: &str) -> Option<&'a str> {
    field(form, name).and_then(|field| match &field.value {
        XmlValue::String(value) => Some(value.as_str()),
        XmlValue::Content(_) => None,
    })
}

pub(crate) fn child_elements(form: &XmlForm) -> Result<Vec<&XmlForm>, ScriptLangError> {
    let mut children = Vec::new();
    for item in content(form)? {
        match item {
            XmlContentItem::Node(node) => children.push(node),
            XmlContentItem::Text(text) if text.trim().is_empty() => {}
            XmlContentItem::Text(_) => {
                return Err(error_at(
                    form,
                    format!("<{}> does not support text content in MVP", form.tag),
                ));
            }
        }
    }
    Ok(children)
}

pub(crate) fn trimmed_text_content(form: &XmlForm) -> Result<String, ScriptLangError> {
    let mut text = String::new();
    for item in content(form)? {
        match item {
            XmlContentItem::Text(segment) => text.push_str(segment),
            XmlContentItem::Node(node) => {
                return Err(error_at(
                    node,
                    format!(
                        "nested <{}> is not supported inside <{}> in MVP",
                        node.tag, form.tag
                    ),
                ));
            }
        }
    }
    Ok(text.trim().to_string())
}

pub(crate) fn location(meta: &XmlMeta) -> String {
    match &meta.source_name {
        Some(source_name) => format!("{source_name}:{}:{}", meta.start.row, meta.start.column),
        None => format!("{}:{}", meta.start.row, meta.start.column),
    }
}

pub(crate) fn error_at(form: &XmlForm, message: impl Into<String>) -> ScriptLangError {
    ScriptLangError::message(format!("{} at {}", message.into(), location(&form.meta)))
}

fn content(form: &XmlForm) -> Result<&[XmlContentItem], ScriptLangError> {
    match field(form, "content") {
        Some(XmlField {
            value: XmlValue::Content(items),
            ..
        }) => Ok(items),
        Some(XmlField {
            value: XmlValue::String(_),
            ..
        }) => Err(error_at(
            form,
            format!("<{}> has invalid `content` field shape", form.tag),
        )),
        None => Err(error_at(
            form,
            format!("<{}> is missing `content` field", form.tag),
        )),
    }
}

fn field<'a>(form: &'a XmlForm, name: &str) -> Option<&'a XmlField> {
    form.fields.iter().find(|field| field.name == name)
}

#[cfg(test)]
mod tests {
    use sl_core::{XmlContentItem, XmlField, XmlForm, XmlMeta, XmlPosition, XmlValue};

    use super::{attr, child_elements, required_attr, trimmed_text_content};

    fn form(tag: &str, fields: Vec<XmlField>) -> XmlForm {
        XmlForm {
            tag: tag.to_string(),
            meta: XmlMeta {
                source_name: Some("main.xml".to_string()),
                start: XmlPosition { row: 1, column: 1 },
                end: XmlPosition { row: 1, column: 20 },
                start_byte: 0,
                end_byte: 20,
            },
            fields,
        }
    }

    #[test]
    fn xml_helpers_read_attributes_and_content() {
        let module = form(
            "module",
            vec![
                XmlField {
                    name: "name".to_string(),
                    value: XmlValue::String("main".to_string()),
                },
                XmlField {
                    name: "content".to_string(),
                    value: XmlValue::Content(vec![XmlContentItem::Node(form(
                        "script",
                        vec![XmlField {
                            name: "content".to_string(),
                            value: XmlValue::Content(Vec::new()),
                        }],
                    ))]),
                },
            ],
        );

        assert_eq!(attr(&module, "name"), Some("main"));
        assert_eq!(required_attr(&module, "name").expect("required"), "main");
        assert_eq!(child_elements(&module).expect("children")[0].tag, "script");
    }

    #[test]
    fn xml_helpers_reject_invalid_text_or_missing_attrs() {
        let script = form(
            "script",
            vec![XmlField {
                name: "content".to_string(),
                value: XmlValue::Content(vec![XmlContentItem::Text("bad".to_string())]),
            }],
        );
        let temp = form(
            "temp",
            vec![XmlField {
                name: "content".to_string(),
                value: XmlValue::Content(vec![XmlContentItem::Node(form(
                    "inner",
                    vec![XmlField {
                        name: "content".to_string(),
                        value: XmlValue::Content(Vec::new()),
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
            child_elements(&script)
                .expect_err("unexpected text")
                .to_string()
                .contains("does not support text content")
        );
        assert!(
            trimmed_text_content(&temp)
                .expect_err("nested node")
                .to_string()
                .contains("nested <inner> is not supported")
        );
    }
}
