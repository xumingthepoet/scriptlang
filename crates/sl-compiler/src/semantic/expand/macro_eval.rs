use sl_core::{Form, FormItem, FormValue, ScriptLangError};

use super::dispatch::ExpandRuleScope;
use super::macro_env::MacroEnv;
use super::macro_values::MacroValue;
use super::quote::quote_items;
use super::raw_body_text;
use crate::semantic::env::ExpandEnv;
use crate::semantic::{attr, error_at, required_attr};

pub(crate) fn evaluate_macro_items(
    body: &[FormItem],
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    let mut runtime = MacroEnv::from_invocation(
        env,
        &invocation.head,
        invocation_attributes(invocation),
        invocation_children(invocation),
    );
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
    _invocation: &Form,
    runtime: &mut MacroEnv,
) -> Result<(), ScriptLangError> {
    let name = required_attr(form, "name")?.to_string();
    let type_name = required_attr(form, "type")?;
    let provider = single_child_form(form)?;
    let value = match (type_name, provider.head.as_str()) {
        ("expr", "get-attribute") => {
            let attr_name = required_attr(provider, "name")?;
            let value = runtime.attributes.get(attr_name).ok_or_else(|| {
                ScriptLangError::message(format!(
                    "{} is missing invocation attribute `{attr_name}`",
                    runtime.context_label()
                ))
            })?;
            MacroValue::Expr(value.to_string())
        }
        ("string", "get-attribute") => {
            let attr_name = required_attr(provider, "name")?;
            let value = runtime.attributes.get(attr_name).ok_or_else(|| {
                ScriptLangError::message(format!(
                    "{} is missing invocation attribute `{attr_name}`",
                    runtime.context_label()
                ))
            })?;
            MacroValue::String(value.to_string())
        }
        ("bool", "get-attribute") => {
            let attr_name = required_attr(provider, "name")?;
            let value = runtime.attributes.get(attr_name).ok_or_else(|| {
                ScriptLangError::message(format!(
                    "{} is missing invocation attribute `{attr_name}`",
                    runtime.context_label()
                ))
            })?;
            let parsed = match value.as_str() {
                "true" => true,
                "false" => false,
                other => {
                    return Err(error_at(
                        provider,
                        format!("cannot parse `{other}` as macro bool attribute"),
                    ));
                }
            };
            MacroValue::Bool(parsed)
        }
        ("int", "get-attribute") => {
            let attr_name = required_attr(provider, "name")?;
            let value = runtime.attributes.get(attr_name).ok_or_else(|| {
                ScriptLangError::message(format!(
                    "{} is missing invocation attribute `{attr_name}`",
                    runtime.context_label()
                ))
            })?;
            let parsed = value.parse::<i64>().map_err(|_| {
                error_at(
                    provider,
                    format!("cannot parse `{value}` as macro int attribute"),
                )
            })?;
            MacroValue::Int(parsed)
        }
        ("ast", "get-content") => {
            MacroValue::AstItems(select_invocation_content(runtime, provider)?)
        }
        ("ast", "quote") => MacroValue::AstItems(quote_items(
            _invocation,
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

pub(super) fn eval_unquote(form: &Form, runtime: &MacroEnv) -> Result<MacroValue, ScriptLangError> {
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

fn invocation_attributes(invocation: &Form) -> std::collections::BTreeMap<String, String> {
    invocation
        .fields
        .iter()
        .filter_map(|field| match (&field.name[..], &field.value) {
            ("children", _) => None,
            (name, FormValue::String(value)) => Some((name.to_string(), value.clone())),
            _ => None,
        })
        .collect()
}

fn select_invocation_content(
    runtime: &MacroEnv,
    provider: &Form,
) -> Result<Vec<FormItem>, ScriptLangError> {
    match attr(provider, "head") {
        None => Ok(runtime.content.clone()),
        Some(head) => {
            let mut selected = Vec::new();
            for item in &runtime.content {
                let item = item.clone();
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
                        "while",
                        vec![
                            ("when", "!condition"),
                            ("__sl_skip_loop_control_capture", "true"),
                        ],
                        vec![
                            child(node("code", vec![], vec![text_item("condition = true;")])),
                            child(node("unquote", vec![], vec![text_item("content_ast")])),
                        ],
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
        assert_eq!(attr(first, "name"), Some("__macro_unless_1_condition_1"));
        assert_eq!(raw_body_text(first).as_deref(), Some("flag"));
        assert_eq!(second.head, "while");
        assert_eq!(attr(second, "when"), Some("!__macro_unless_1_condition_1"));
        let second_children = invocation_children(second);
        assert_eq!(second_children.len(), 2);
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

    #[test]
    fn macro_evaluator_rejects_missing_quote_and_unexpected_top_level_text() {
        let invocation = node("m", vec![], vec![]);

        let missing_quote = evaluate_macro_items(
            &[],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("missing quote");
        assert!(
            missing_quote
                .to_string()
                .contains("requires one <quote> block")
        );

        let text_error = evaluate_macro_items(
            &[text_item("unexpected")],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("top-level text");
        assert!(text_error.to_string().contains("unexpected top-level text"));
    }

    #[test]
    fn macro_evaluator_covers_let_scalar_types_and_error_paths() {
        let invocation = node(
            "sample",
            vec![("flag", "true"), ("count", "7"), ("name", "neo")],
            vec![],
        );
        let body = vec![
            child(node(
                "let",
                vec![("name", "flag_v"), ("type", "bool")],
                vec![child(node("get-attribute", vec![("name", "flag")], vec![]))],
            )),
            child(node(
                "let",
                vec![("name", "count_v"), ("type", "int")],
                vec![child(node(
                    "get-attribute",
                    vec![("name", "count")],
                    vec![],
                ))],
            )),
            child(node(
                "let",
                vec![("name", "name_v"), ("type", "string")],
                vec![child(node("get-attribute", vec![("name", "name")], vec![]))],
            )),
            child(node(
                "quote",
                vec![],
                vec![child(node(
                    "text",
                    vec![("tag", "${name_v}")],
                    vec![text_item("${count_v}:${flag_v}")],
                ))],
            )),
        ];

        let items = evaluate_macro_items(
            &body,
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect("macro eval");
        let text_form = match &items[0] {
            FormItem::Form(form) => form,
            _ => panic!("expected text form"),
        };
        assert_eq!(attr(text_form, "tag"), Some("neo"));
        assert_eq!(raw_body_text(text_form).as_deref(), Some("7:true"));

        let bad_bool = evaluate_macro_items(
            &[
                child(node(
                    "let",
                    vec![("name", "flag_v"), ("type", "bool")],
                    vec![child(node("get-attribute", vec![("name", "name")], vec![]))],
                )),
                child(node("quote", vec![], vec![])),
            ],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("bad bool");
        assert!(
            bad_bool
                .to_string()
                .contains("cannot parse `neo` as macro bool attribute")
        );

        let bad_type = evaluate_macro_items(
            &[
                child(node(
                    "let",
                    vec![("name", "x"), ("type", "float")],
                    vec![child(node(
                        "get-attribute",
                        vec![("name", "count")],
                        vec![],
                    ))],
                )),
                child(node("quote", vec![], vec![])),
            ],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("bad type");
        assert!(bad_type.to_string().contains("unsupported macro let type"));

        let bad_provider = evaluate_macro_items(
            &[
                child(node(
                    "let",
                    vec![("name", "x"), ("type", "expr")],
                    vec![child(node("get-content", vec![], vec![]))],
                )),
                child(node("quote", vec![], vec![])),
            ],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("bad provider");
        assert!(
            bad_provider
                .to_string()
                .contains("unsupported <get-content> provider")
        );
    }

    #[test]
    fn macro_evaluator_covers_quote_provider_and_single_child_errors() {
        let invocation = node(
            "m",
            vec![("name", "neo")],
            vec![child(node("text", vec![], vec![text_item("hello")]))],
        );

        let quoted_provider = evaluate_macro_items(
            &[
                child(node(
                    "let",
                    vec![("name", "ast_v"), ("type", "ast")],
                    vec![child(node(
                        "quote",
                        vec![],
                        vec![child(node("text", vec![], vec![text_item("quoted")]))],
                    ))],
                )),
                child(node(
                    "quote",
                    vec![],
                    vec![child(node("unquote", vec![], vec![text_item("ast_v")]))],
                )),
            ],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect("quoted provider");
        let quoted_text = match &quoted_provider[0] {
            FormItem::Form(form) => form,
            _ => panic!("expected text form"),
        };
        assert_eq!(raw_body_text(quoted_text).as_deref(), Some("quoted"));

        let multi_child = evaluate_macro_items(
            &[
                child(node(
                    "let",
                    vec![("name", "x"), ("type", "string")],
                    vec![
                        child(node("get-attribute", vec![("name", "name")], vec![])),
                        child(node("get-attribute", vec![("name", "name")], vec![])),
                    ],
                )),
                child(node("quote", vec![], vec![])),
            ],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("multi child");
        assert!(
            multi_child
                .to_string()
                .contains("requires exactly one meaningful child")
        );

        let missing_attr = evaluate_macro_items(
            &[
                child(node(
                    "let",
                    vec![("name", "x"), ("type", "string")],
                    vec![child(node(
                        "get-attribute",
                        vec![("name", "missing")],
                        vec![],
                    ))],
                )),
                child(node("quote", vec![], vec![])),
            ],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("missing attr");
        assert!(
            missing_attr
                .to_string()
                .contains("missing invocation attribute `missing`")
        );
    }

    #[test]
    fn macro_evaluator_covers_unsupported_forms_and_helper_errors() {
        let invocation = node("m", vec![("name", "neo")], vec![text_item("plain")]);

        let unsupported = evaluate_macro_items(
            &[
                child(node("if", vec![], vec![])),
                child(node("quote", vec![], vec![])),
            ],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("unsupported compile-time form");
        assert!(
            unsupported
                .to_string()
                .contains("unsupported compile-time macro form <if>")
        );

        let missing_unquote_body =
            eval_unquote(&node("unquote", vec![], vec![]), &MacroEnv::default())
                .expect_err("missing local name");
        assert!(
            missing_unquote_body
                .to_string()
                .contains("requires local name body")
        );

        let unknown_unquote = eval_unquote(
            &node("unquote", vec![], vec![text_item("missing")]),
            &MacroEnv::default(),
        )
        .expect_err("unknown local");
        assert!(
            unknown_unquote
                .to_string()
                .contains("unknown macro local `missing`")
        );

        let expected_form = evaluate_macro_items(
            &[
                child(node(
                    "let",
                    vec![("name", "x"), ("type", "string")],
                    vec![text_item("oops")],
                )),
                child(node("quote", vec![], vec![])),
            ],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("text child");
        assert!(expected_form.to_string().contains("expected child form"));

        let missing_children = evaluate_macro_items(
            &[
                child(form(
                    "let",
                    vec![attr_field("name", "x"), attr_field("type", "string")],
                )),
                child(node("quote", vec![], vec![])),
            ],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("missing children");
        assert!(
            missing_children
                .to_string()
                .contains("<let> requires `children`")
        );

        let filtered = select_invocation_content(
            &MacroEnv {
                current_module: Some("main".to_string()),
                imports: vec![],
                requires: vec![],
                aliases: Default::default(),
                macro_name: "m".to_string(),
                attributes: Default::default(),
                content: vec![
                    text_item("ignored"),
                    child(node("do", vec![], vec![text_item("a")])),
                    child(form("do", vec![])),
                ],
                locals: Default::default(),
                gensym_seed: 0,
                gensym_counter: 0,
            },
            &node("get-content", vec![("head", "do")], vec![]),
        )
        .expect_err("filtered malformed child");
        assert!(filtered.to_string().contains("<do> requires `children`"));
    }

    #[test]
    fn macro_evaluator_covers_missing_attributes_and_single_child_edge_cases() {
        let invocation = node("m", vec![], vec![]);

        let missing_expr = evaluate_macro_items(
            &[
                child(node(
                    "let",
                    vec![("name", "when_expr"), ("type", "expr")],
                    vec![child(node("get-attribute", vec![("name", "when")], vec![]))],
                )),
                child(node("quote", vec![], vec![])),
            ],
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("missing expr attr");
        assert!(
            missing_expr
                .to_string()
                .contains("missing invocation attribute `when`")
        );

        let bad_bool = evaluate_macro_items(
            &[
                child(node(
                    "let",
                    vec![("name", "flag"), ("type", "bool")],
                    vec![child(node("get-attribute", vec![("name", "flag")], vec![]))],
                )),
                child(node("quote", vec![], vec![])),
            ],
            &node("m", vec![("flag", "maybe")], vec![]),
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("bad bool attr");
        assert!(
            bad_bool
                .to_string()
                .contains("cannot parse `maybe` as macro bool attribute")
        );

        let bad_int = evaluate_macro_items(
            &[
                child(node(
                    "let",
                    vec![("name", "count"), ("type", "int")],
                    vec![child(node(
                        "get-attribute",
                        vec![("name", "count")],
                        vec![],
                    ))],
                )),
                child(node("quote", vec![], vec![])),
            ],
            &node("m", vec![("count", "abc")], vec![]),
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect_err("bad int attr");
        assert!(
            bad_int
                .to_string()
                .contains("cannot parse `abc` as macro int attribute")
        );

        let no_children = single_child_form(&form("let", vec![children_field(Vec::new())]))
            .expect_err("no child");
        assert!(
            no_children
                .to_string()
                .contains("requires exactly one meaningful child")
        );

        let text_child =
            single_child_form(&form("let", vec![children_field(vec![text_item("x")])]))
                .expect_err("text child");
        assert!(text_child.to_string().contains("expected child form"));

        assert!(invocation_children(&invocation).is_empty());
        assert_eq!(
            invocation_attributes(&node("m", vec![("count", "1")], vec![]))
                .get("count")
                .map(String::as_str),
            Some("1")
        );
    }
}
