use sl_core::{Form, FormMeta, FormValue, ScriptLangError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClassifiedForm {
    pub(crate) head: String,
    pub(crate) meta: FormMeta,
    pub(crate) attrs: Vec<ClassifiedAttr>,
    pub(crate) body: ClassifiedBody,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClassifiedAttr {
    pub(crate) name: String,
    pub(crate) value: String,
    pub(crate) kind: SlotKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ClassifiedBody {
    Statements(Vec<ClassifiedForm>),
    Expr(String),
    Template(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlotKind {
    PlainText,
    Expr,
    Template,
    Ident,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BodyKind {
    Statements,
    Expr,
    Template,
}

pub(crate) fn classify_forms(forms: &[Form]) -> Result<Vec<ClassifiedForm>, ScriptLangError> {
    forms.iter().map(classify_form).collect()
}

pub(crate) fn attr<'a>(form: &'a ClassifiedForm, name: &str) -> Option<&'a str> {
    form.attrs
        .iter()
        .find(|attr| attr.name == name)
        .map(|attr| attr.value.as_str())
}

pub(crate) fn required_attr<'a>(
    form: &'a ClassifiedForm,
    name: &str,
) -> Result<&'a str, ScriptLangError> {
    attr(form, name).ok_or_else(|| error_at(form, format!("<{}> requires `{name}`", form.head)))
}

pub(crate) fn body_expr(form: &ClassifiedForm) -> Result<&str, ScriptLangError> {
    match &form.body {
        ClassifiedBody::Expr(expr) => Ok(expr),
        _ => Err(error_at(
            form,
            format!("<{}> body is not classified as an expression", form.head),
        )),
    }
}

pub(crate) fn body_template(form: &ClassifiedForm) -> Result<&str, ScriptLangError> {
    match &form.body {
        ClassifiedBody::Template(text) => Ok(text),
        _ => Err(error_at(
            form,
            format!("<{}> body is not classified as a template", form.head),
        )),
    }
}

pub(crate) fn child_forms(form: &ClassifiedForm) -> Result<Vec<&ClassifiedForm>, ScriptLangError> {
    match &form.body {
        ClassifiedBody::Statements(children) => Ok(children.iter().collect()),
        _ => Err(error_at(
            form,
            format!("<{}> body does not support nested statements", form.head),
        )),
    }
}

pub(crate) fn error_at(form: &ClassifiedForm, message: impl Into<String>) -> ScriptLangError {
    ScriptLangError::message(format!("{} at {}", message.into(), location(&form.meta)))
}

fn classify_form(form: &Form) -> Result<ClassifiedForm, ScriptLangError> {
    let body_kind = body_kind(&form.head);
    let attrs = form
        .fields
        .iter()
        .filter_map(|field| match &field.value {
            FormValue::String(value) => Some(ClassifiedAttr {
                name: field.name.clone(),
                value: value.clone(),
                kind: attr_kind(&form.head, &field.name),
            }),
            FormValue::Sequence(_) => None,
        })
        .collect::<Vec<_>>();
    let body = classify_body(form, body_kind)?;

    Ok(ClassifiedForm {
        head: form.head.clone(),
        meta: form.meta.clone(),
        attrs,
        body,
    })
}

fn classify_body(form: &Form, kind: BodyKind) -> Result<ClassifiedBody, ScriptLangError> {
    let items = form
        .fields
        .iter()
        .find_map(|field| match (&field.name[..], &field.value) {
            ("children", FormValue::Sequence(items)) => Some(items.as_slice()),
            _ => None,
        })
        .ok_or_else(|| {
            ScriptLangError::message(format!(
                "<{}> is missing `children` field at {}",
                form.head,
                location(&form.meta)
            ))
        })?;

    match kind {
        BodyKind::Statements => {
            let mut children = Vec::new();
            for item in items {
                match item {
                    sl_core::FormItem::Form(child) => children.push(classify_form(child)?),
                    sl_core::FormItem::Text(text) if text.trim().is_empty() => {}
                    sl_core::FormItem::Text(_) => {
                        return Err(ScriptLangError::message(format!(
                            "<{}> does not support text items in MVP at {}",
                            form.head,
                            location(&form.meta)
                        )));
                    }
                }
            }
            Ok(ClassifiedBody::Statements(children))
        }
        BodyKind::Expr | BodyKind::Template => {
            let mut text = String::new();
            for item in items {
                match item {
                    sl_core::FormItem::Text(segment) => text.push_str(segment),
                    sl_core::FormItem::Form(node) => {
                        return Err(ScriptLangError::message(format!(
                            "nested <{}> is not supported inside <{}> in MVP at {}",
                            node.head,
                            form.head,
                            location(&node.meta)
                        )));
                    }
                }
            }
            let text = text.trim().to_string();
            Ok(match kind {
                BodyKind::Expr => ClassifiedBody::Expr(text),
                BodyKind::Template => ClassifiedBody::Template(text),
                BodyKind::Statements => unreachable!("handled above"),
            })
        }
    }
}

fn attr_kind(head: &str, name: &str) -> SlotKind {
    match (head, name) {
        ("module", "name")
        | ("import", "name")
        | ("script", "name")
        | ("var", "name")
        | ("var", "type")
        | ("temp", "name")
        | ("temp", "type")
        | ("const", "name")
        | ("const", "type") => SlotKind::Ident,
        ("if", "when") | ("goto", "script") => SlotKind::Expr,
        ("choice", "text") | ("option", "text") => SlotKind::Template,
        _ => SlotKind::PlainText,
    }
}

fn body_kind(head: &str) -> BodyKind {
    match head {
        "var" | "temp" | "const" | "code" => BodyKind::Expr,
        "text" => BodyKind::Template,
        _ => BodyKind::Statements,
    }
}

fn location(meta: &FormMeta) -> String {
    match &meta.source_name {
        Some(source_name) => format!("{source_name}:{}:{}", meta.start.row, meta.start.column),
        None => format!("{}:{}", meta.start.row, meta.start.column),
    }
}

#[cfg(test)]
mod tests {
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use super::{
        ClassifiedBody, SlotKind, attr, body_expr, body_template, child_forms, classify_forms,
    };

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

    fn attr_field(name: &str, value: &str) -> FormField {
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

    #[test]
    fn classify_slots_marks_expr_and_template_positions() {
        let forms = vec![form(
            "module",
            vec![
                attr_field("name", "main"),
                children(vec![
                    FormItem::Form(form(
                        "var",
                        vec![
                            attr_field("name", "next"),
                            attr_field("type", "script"),
                            children(vec![FormItem::Text("@loop".to_string())]),
                        ],
                    )),
                    FormItem::Form(form(
                        "script",
                        vec![
                            attr_field("name", "main"),
                            children(vec![
                                FormItem::Form(form(
                                    "if",
                                    vec![
                                        attr_field("when", "true"),
                                        children(vec![FormItem::Form(form(
                                            "goto",
                                            vec![
                                                attr_field("script", "next"),
                                                children(Vec::new()),
                                            ],
                                        ))]),
                                    ],
                                )),
                                FormItem::Form(form(
                                    "text",
                                    vec![children(vec![FormItem::Text("${next}".to_string())])],
                                )),
                            ]),
                        ],
                    )),
                ]),
            ],
        )];

        let classified = classify_forms(&forms).expect("classify");
        let module = &classified[0];
        assert_eq!(module.attrs[0].kind, SlotKind::Ident);
        let children = child_forms(module).expect("children");
        assert_eq!(children[0].attrs[1].kind, SlotKind::Ident);
        assert_eq!(body_expr(children[0]).expect("expr"), "@loop");

        let script = children[1];
        let stmts = child_forms(script).expect("script stmts");
        assert_eq!(attr(stmts[0], "when"), Some("true"));
        assert_eq!(stmts[0].attrs[0].kind, SlotKind::Expr);
        let goto = child_forms(stmts[0]).expect("if body")[0];
        assert_eq!(goto.attrs[0].kind, SlotKind::Expr);
        match &stmts[1].body {
            ClassifiedBody::Template(text) => assert_eq!(text, "${next}"),
            other => panic!("expected template, got {other:?}"),
        }
        assert_eq!(body_template(stmts[1]).expect("template"), "${next}");
    }
}
