use std::collections::BTreeMap;

use sl_core::{Form, FormItem, FormValue, ScriptLangError};

use super::dispatch::ExpandRuleScope;
use super::quote::quote_items;
use super::raw_body_text;
use crate::semantic::env::ExpandEnv;
use crate::semantic::{attr, error_at, required_attr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MacroValue {
    String(String),
    Expr(String),
    AstItems(Vec<FormItem>),
}

#[derive(Clone, Debug, Default)]
pub(super) struct MacroRuntimeEnv {
    pub(super) locals: BTreeMap<String, MacroValue>,
    pub(super) gensym_counter: usize,
}

pub(crate) fn uses_macro_evaluator(body: &[FormItem]) -> bool {
    body.iter().any(|item| match item {
        FormItem::Text(_) => false,
        FormItem::Form(form) => matches!(form.head.as_str(), "let" | "quote"),
    })
}

pub(crate) fn evaluate_macro_items(
    body: &[FormItem],
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    let mut runtime = MacroRuntimeEnv::default();
    let forms = meaningful_macro_forms(body)?;
    let mut quoted = None;

    for form in forms {
        match form.head.as_str() {
            "let" => eval_let(form, invocation, &mut runtime)?,
            "quote" => {
                let items =
                    quote_items(invocation, env, scope, &mut runtime, form_children(form)?)?;
                quoted = Some(items);
            }
            other => {
                return Err(error_at(
                    form,
                    format!("unsupported compile-time macro form <{other}>"),
                ));
            }
        }
    }

    quoted.ok_or_else(|| ScriptLangError::message("macro evaluator requires one <quote> block"))
}

fn meaningful_macro_forms(body: &[FormItem]) -> Result<Vec<&Form>, ScriptLangError> {
    let mut forms = Vec::new();
    for item in body {
        match item {
            FormItem::Text(text) if text.trim().is_empty() => {}
            FormItem::Text(_) => {
                return Err(ScriptLangError::message(
                    "unexpected top-level text inside macro body",
                ));
            }
            FormItem::Form(form) => forms.push(form),
        }
    }
    Ok(forms)
}

fn eval_let(
    form: &Form,
    invocation: &Form,
    runtime: &mut MacroRuntimeEnv,
) -> Result<(), ScriptLangError> {
    let name = required_attr(form, "name")?.to_string();
    let type_name = required_attr(form, "type")?;
    let provider = single_child_form(form)?;
    let value = match (type_name, provider.head.as_str()) {
        ("expr", "get-attribute") => {
            let attr_name = required_attr(provider, "name")?;
            let value = attr(invocation, attr_name).ok_or_else(|| {
                ScriptLangError::message(format!(
                    "macro invocation `<{}>` is missing attribute `{attr_name}`",
                    invocation.head
                ))
            })?;
            MacroValue::Expr(value.to_string())
        }
        ("string", "get-attribute") => {
            let attr_name = required_attr(provider, "name")?;
            let value = attr(invocation, attr_name).ok_or_else(|| {
                ScriptLangError::message(format!(
                    "macro invocation `<{}>` is missing attribute `{attr_name}`",
                    invocation.head
                ))
            })?;
            MacroValue::String(value.to_string())
        }
        ("ast", "get-content") => {
            MacroValue::AstItems(select_invocation_content(invocation, provider)?)
        }
        ("ast", "quote") => MacroValue::AstItems(quote_items(
            invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
            runtime,
            form_children(provider)?,
        )?),
        ("expr", other) | ("ast", other) | ("string", other) => {
            return Err(error_at(
                provider,
                format!("unsupported <{other}> provider for macro let type `{type_name}`"),
            ));
        }
        (other, _) => {
            return Err(error_at(
                form,
                format!("unsupported macro let type `{other}`"),
            ));
        }
    };
    runtime.locals.insert(name, value);
    Ok(())
}

pub(super) fn eval_unquote(
    form: &Form,
    runtime: &MacroRuntimeEnv,
) -> Result<MacroValue, ScriptLangError> {
    let name = raw_body_text(form)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| error_at(form, "<unquote> requires local name body"))?;
    runtime.locals.get(&name).cloned().ok_or_else(|| {
        error_at(
            form,
            format!("unknown macro local `{name}` referenced by <unquote>"),
        )
    })
}

fn single_child_form(form: &Form) -> Result<&Form, ScriptLangError> {
    let children = form_children(form)?;
    let meaningful = children
        .iter()
        .filter(|item| !matches!(item, FormItem::Text(text) if text.trim().is_empty()))
        .collect::<Vec<_>>();
    if meaningful.len() != 1 {
        return Err(error_at(
            form,
            "macro compile-time form requires exactly one meaningful child",
        ));
    }
    match meaningful[0] {
        FormItem::Form(child) => Ok(child),
        FormItem::Text(_) => Err(error_at(form, "expected child form")),
    }
}

fn form_children(form: &Form) -> Result<&[FormItem], ScriptLangError> {
    form.fields
        .iter()
        .find_map(|field| match (&field.name[..], &field.value) {
            ("children", FormValue::Sequence(items)) => Some(items.as_slice()),
            _ => None,
        })
        .ok_or_else(|| error_at(form, format!("<{}> requires `children`", form.head)))
}

fn invocation_children(invocation: &Form) -> Vec<FormItem> {
    invocation
        .fields
        .iter()
        .find_map(|field| match (&field.name[..], &field.value) {
            ("children", FormValue::Sequence(items)) => Some(items.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn select_invocation_content(
    invocation: &Form,
    provider: &Form,
) -> Result<Vec<FormItem>, ScriptLangError> {
    match attr(provider, "head") {
        None => Ok(invocation_children(invocation)),
        Some(head) => {
            let mut selected = Vec::new();
            for item in invocation_children(invocation) {
                let FormItem::Form(form) = item else {
                    continue;
                };
                if form.head != head {
                    continue;
                }
                selected.extend(form_children(&form)?.to_vec());
            }
            Ok(selected)
        }
    }
}

#[cfg(test)]
mod tests {
    use sl_core::{FormField, FormMeta, SourcePosition};

    use super::*;

    fn meta() -> FormMeta {
        FormMeta {
            source_name: Some("main.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 20 },
            start_byte: 0,
            end_byte: 20,
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

    fn children_field(items: Vec<FormItem>) -> FormField {
        FormField {
            name: "children".to_string(),
            value: FormValue::Sequence(items),
        }
    }

    fn text_item(value: &str) -> FormItem {
        FormItem::Text(value.to_string())
    }

    fn node(head: &str, attrs: Vec<(&str, &str)>, items: Vec<FormItem>) -> Form {
        let mut fields = attrs
            .into_iter()
            .map(|(name, value)| attr_field(name, value))
            .collect::<Vec<_>>();
        fields.push(children_field(items));
        form(head, fields)
    }

    fn child(form: Form) -> FormItem {
        FormItem::Form(form)
    }

    #[test]
    fn macro_evaluator_builds_unless_style_items() {
        let invocation = node(
            "unless",
            vec![("when", "flag")],
            vec![child(node("text", vec![], vec![text_item("hello")]))],
        );
        let body = vec![
            child(node(
                "let",
                vec![("name", "when_expr"), ("type", "expr")],
                vec![child(node("get-attribute", vec![("name", "when")], vec![]))],
            )),
            child(node(
                "let",
                vec![("name", "content_ast"), ("type", "ast")],
                vec![child(node("get-content", vec![], vec![]))],
            )),
            child(node(
                "quote",
                vec![],
                vec![
                    child(node(
                        "temp",
                        vec![("name", "condition"), ("type", "bool")],
                        vec![child(node("unquote", vec![], vec![text_item("when_expr")]))],
                    )),
                    child(node(
                        "if",
                        vec![("when", "!condition")],
                        vec![child(node(
                            "unquote",
                            vec![],
                            vec![text_item("content_ast")],
                        ))],
                    )),
                ],
            )),
        ];

        let mut env = ExpandEnv::default();
        let items = evaluate_macro_items(&body, &invocation, &mut env, ExpandRuleScope::Statement)
            .expect("macro eval");
        assert_eq!(items.len(), 2);
        let first = match &items[0] {
            FormItem::Form(form) => form,
            _ => panic!("expected form"),
        };
        let second = match &items[1] {
            FormItem::Form(form) => form,
            _ => panic!("expected form"),
        };
        assert_eq!(first.head, "temp");
        assert_eq!(attr(first, "name"), Some("__macro_condition_1"));
        assert_eq!(raw_body_text(first).as_deref(), Some("flag"));
        assert_eq!(second.head, "if");
        assert_eq!(attr(second, "when"), Some("!__macro_condition_1"));
        let second_children = invocation_children(second);
        assert_eq!(second_children.len(), 1);
    }

    #[test]
    fn macro_evaluator_supports_filtered_get_content_slots() {
        let invocation = node(
            "if-else",
            vec![("when", "flag")],
            vec![
                child(node(
                    "do",
                    vec![],
                    vec![child(node("text", vec![], vec![text_item("a")]))],
                )),
                child(node(
                    "else",
                    vec![],
                    vec![child(node("text", vec![], vec![text_item("b")]))],
                )),
            ],
        );
        let body = vec![
            child(node(
                "let",
                vec![("name", "do_ast"), ("type", "ast")],
                vec![child(node("get-content", vec![("head", "do")], vec![]))],
            )),
            child(node(
                "let",
                vec![("name", "else_ast"), ("type", "ast")],
                vec![child(node("get-content", vec![("head", "else")], vec![]))],
            )),
            child(node(
                "quote",
                vec![],
                vec![
                    child(node("unquote", vec![], vec![text_item("do_ast")])),
                    child(node("unquote", vec![], vec![text_item("else_ast")])),
                ],
            )),
        ];

        let mut env = ExpandEnv::default();
        let items = evaluate_macro_items(&body, &invocation, &mut env, ExpandRuleScope::Statement)
            .expect("macro eval");
        assert_eq!(items.len(), 2);
        let first = match &items[0] {
            FormItem::Form(form) => form,
            _ => panic!("expected form"),
        };
        let second = match &items[1] {
            FormItem::Form(form) => form,
            _ => panic!("expected form"),
        };
        assert_eq!(first.head, "text");
        assert_eq!(second.head, "text");
        assert_eq!(raw_body_text(first).as_deref(), Some("a"));
        assert_eq!(raw_body_text(second).as_deref(), Some("b"));
    }
}
