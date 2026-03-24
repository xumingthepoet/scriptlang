use std::collections::BTreeMap;

use sl_core::{Form, FormField, FormItem, FormValue, ScriptLangError, SourcePosition};

use super::dispatch::{ExpandRuleScope, expand_form_items};
use super::macro_env::MacroEnv;
use super::macro_eval::eval_unquote;
use super::macro_values::MacroValue;
use crate::semantic::attr;
use crate::semantic::env::ExpandEnv;
use crate::semantic::error_at;
use crate::semantic::expr::rewrite_expr_idents;

// Helper: build a minimal FormMeta for synthetic forms.
fn minimal_form_meta() -> sl_core::FormMeta {
    sl_core::FormMeta {
        source_name: None,
        start: SourcePosition { row: 0, column: 0 },
        end: SourcePosition { row: 0, column: 0 },
        start_byte: 0,
        end_byte: 0,
    }
}

pub(crate) fn quote_items(
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
    runtime: &mut MacroEnv,
    items: &[FormItem],
) -> Result<Vec<FormItem>, ScriptLangError> {
    let mut renames = BTreeMap::new();
    quote_ast_items(invocation, env, scope, runtime, items, &mut renames)
}

fn quote_ast_items(
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
    runtime: &mut MacroEnv,
    items: &[FormItem],
    renames: &mut BTreeMap<String, String>,
) -> Result<Vec<FormItem>, ScriptLangError> {
    let mut output = Vec::new();
    for item in items {
        match item {
            FormItem::Text(text) => {
                output.push(FormItem::Text(splice_string_slots(text, runtime)?))
            }
            FormItem::Form(form) if form.head == "unquote" => match eval_unquote(form, runtime)? {
                MacroValue::AstItems(items) => output.extend(items),
                MacroValue::String(text) => output.push(FormItem::Text(text)),
                MacroValue::List(items) => {
                    // Each element of the list becomes a FormItem (as text nodes or AST items)
                    for item in items {
                        output.push(macro_value_to_form_item(&item)?);
                    }
                }
                MacroValue::Keyword(kv_pairs) => {
                    // Keyword in AST children: stringify as "key:val,..." text
                    output.push(FormItem::Text(macro_keyword_to_string(&kv_pairs)));
                }
                MacroValue::Nil => {
                    return Err(error_at(form, "<unquote> requires a value, but got nil"));
                }
                MacroValue::Expr(_) | MacroValue::Bool(_) | MacroValue::Int(_) => {
                    return Err(error_at(
                        form,
                        "<unquote> in AST children position requires `ast` or `string` value",
                    ));
                }
            },
            FormItem::Form(form) => {
                let quoted = quote_form(invocation, env, scope, runtime, form, renames)?;
                output.extend(expand_quoted_result(&quoted, env, scope)?);
            }
        }
    }
    Ok(output)
}

fn quote_form(
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
    runtime: &mut MacroEnv,
    form: &Form,
    renames: &mut BTreeMap<String, String>,
) -> Result<Form, ScriptLangError> {
    let temp_name = if form.head == "temp" {
        attr(form, "name").map(str::to_string)
    } else {
        None
    };

    let mut fields = Vec::with_capacity(form.fields.len());
    for field in &form.fields {
        let value = match (&field.name[..], &field.value) {
            (field_name, FormValue::String(text)) => {
                if is_expr_attr(&form.head, field_name) {
                    FormValue::String(rewrite_expr_idents(
                        &splice_expr_slots(text, runtime)?,
                        renames,
                    )?)
                } else {
                    FormValue::String(splice_string_slots(text, runtime)?)
                }
            }
            ("children", FormValue::Sequence(items)) if is_expr_body_form(&form.head) => {
                let expr = quote_expr(items, runtime, renames)?;
                FormValue::Sequence(vec![FormItem::Text(expr)])
            }
            ("children", FormValue::Sequence(items)) => {
                let mut nested = renames.clone();
                FormValue::Sequence(quote_ast_items(
                    invocation,
                    env,
                    scope,
                    runtime,
                    items,
                    &mut nested,
                )?)
            }
            (_, FormValue::Sequence(items)) => FormValue::Sequence(items.clone()),
        };
        fields.push(FormField {
            name: field.name.clone(),
            value,
        });
    }

    let mut quoted = Form {
        head: form.head.clone(),
        meta: invocation.meta.clone(),
        fields,
    };

    if let Some(original_name) = temp_name {
        let fresh = gensym(runtime, &original_name);
        for field in &mut quoted.fields {
            if field.name == "name" {
                field.value = FormValue::String(fresh.clone());
            }
        }
        renames.insert(original_name, fresh);
    }

    Ok(quoted)
}

fn expand_quoted_result(
    form: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    expand_form_items(form, env, scope)
}

/// Convert a MacroValue to a FormItem for use in AST children position.
/// Keywords and Lists are stringified (they cannot become valid statement forms).
fn macro_value_to_form_item(mv: &MacroValue) -> Result<FormItem, ScriptLangError> {
    match mv {
        MacroValue::String(s) => Ok(FormItem::Text(s.clone())),
        MacroValue::AstItems(items) => {
            if items.len() == 1 {
                Ok(items[0].clone())
            } else {
                // Multiple AST items: wrap in a synthetic <list> form
                Ok(FormItem::Form(synthetic_list_form(items.clone())))
            }
        }
        MacroValue::Bool(b) => Ok(FormItem::Text(
            if *b { "true" } else { "false" }.to_string(),
        )),
        MacroValue::Int(i) => Ok(FormItem::Text(i.to_string())),
        MacroValue::Nil => Ok(FormItem::Text("nil".to_string())),
        MacroValue::Keyword(kv) => Ok(FormItem::Text(macro_keyword_to_string(kv))),
        MacroValue::Expr(code) => Ok(FormItem::Text(code.clone())),
        MacroValue::List(_items) => {
            // Lists must be handled at quote_ast_items level (to extend output)
            // This arm exists only for recursive calls from list element handling.
            Err(error_at(
                &Form {
                    head: "?".to_string(),
                    meta: minimal_form_meta(),
                    fields: Vec::new(),
                },
                "list cannot appear as a single item in AST children; each element is expanded",
            ))
        }
    }
}

/// Convert a MacroValue Keyword to a human-readable string "key1:val1,key2:val2".
fn macro_keyword_to_string(kv_pairs: &[(String, MacroValue)]) -> String {
    kv_pairs
        .iter()
        .map(|(k, v)| format!("{}:{}", k, macro_value_to_string(v)))
        .collect::<Vec<_>>()
        .join(",")
}

/// Convert a MacroValue to a string for use in keyword stringification.
fn macro_value_to_string(mv: &MacroValue) -> String {
    match mv {
        MacroValue::String(s) => s.clone(),
        MacroValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        MacroValue::Int(i) => i.to_string(),
        MacroValue::Nil => "nil".to_string(),
        MacroValue::Keyword(kv) => format!("{{{}}}", macro_keyword_to_string(kv)),
        MacroValue::List(items) => {
            let inner = items
                .iter()
                .map(macro_value_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", inner)
        }
        MacroValue::AstItems(_) => "[ast]".to_string(),
        MacroValue::Expr(code) => code.clone(),
    }
}

/// Convert a MacroValue to a FormValue for use in form attributes.
#[allow(dead_code)]
fn macro_value_to_form_value(mv: &MacroValue) -> FormValue {
    match mv {
        MacroValue::String(s) => FormValue::String(s.clone()),
        MacroValue::Bool(b) => FormValue::String(if *b { "true" } else { "false" }.to_string()),
        MacroValue::Int(i) => FormValue::String(i.to_string()),
        MacroValue::Nil => FormValue::String("nil".to_string()),
        MacroValue::AstItems(items) => FormValue::Sequence(items.clone()),
        MacroValue::Keyword(kv) => FormValue::Sequence(
            kv.iter()
                .map(|(k, v)| {
                    FormItem::Form(Form {
                        head: "item".to_string(),
                        meta: minimal_form_meta(),
                        fields: vec![
                            FormField {
                                name: "key".to_string(),
                                value: FormValue::String(k.clone()),
                            },
                            FormField {
                                name: "value".to_string(),
                                value: macro_value_to_form_value(v),
                            },
                        ],
                    })
                })
                .collect(),
        ),
        MacroValue::List(items) => {
            FormValue::Sequence(items.iter().map(macro_value_to_form_item_solo).collect())
        }
        MacroValue::Expr(code) => FormValue::String(code.clone()),
    }
}

/// Convert a MacroValue to a single FormItem (never a list wrapper).
/// Keywords are stringified since they cannot appear as valid statement forms.
#[allow(dead_code)]
fn macro_value_to_form_item_solo(mv: &MacroValue) -> FormItem {
    match mv {
        MacroValue::String(s) => FormItem::Text(s.clone()),
        MacroValue::Bool(b) => FormItem::Text(if *b { "true" } else { "false" }.to_string()),
        MacroValue::Int(i) => FormItem::Text(i.to_string()),
        MacroValue::Nil => FormItem::Text("nil".to_string()),
        MacroValue::AstItems(items) if items.len() == 1 => items[0].clone(),
        MacroValue::AstItems(items) => FormItem::Form(synthetic_list_form(items.clone())),
        MacroValue::Keyword(kv) => FormItem::Text(macro_keyword_to_string(kv)),
        MacroValue::List(items) => FormItem::Form(synthetic_list_form(
            items.iter().map(macro_value_to_form_item_solo).collect(),
        )),
        MacroValue::Expr(code) => FormItem::Text(code.clone()),
    }
}

fn synthetic_list_form(items: Vec<FormItem>) -> Form {
    Form {
        head: "list".to_string(),
        meta: minimal_form_meta(),
        fields: vec![FormField {
            name: "children".to_string(),
            value: FormValue::Sequence(items),
        }],
    }
}

fn quote_expr(
    items: &[FormItem],
    runtime: &mut MacroEnv,
    renames: &BTreeMap<String, String>,
) -> Result<String, ScriptLangError> {
    let mut expr = String::new();
    for item in items {
        match item {
            FormItem::Text(text) => expr.push_str(text),
            FormItem::Form(form) if form.head == "unquote" => match eval_unquote(form, runtime)? {
                MacroValue::Expr(value) | MacroValue::String(value) => expr.push_str(&value),
                MacroValue::Bool(value) => expr.push_str(if value { "true" } else { "false" }),
                MacroValue::Int(value) => expr.push_str(&value.to_string()),
                MacroValue::Nil
                | MacroValue::AstItems(_)
                | MacroValue::Keyword(_)
                | MacroValue::List(_) => {
                    return Err(error_at(
                        form,
                        "<unquote> in expr position requires scalar compile-time value",
                    ));
                }
            },
            FormItem::Form(form) => {
                return Err(error_at(
                    form,
                    "expr quote slot only supports text and <unquote>",
                ));
            }
        }
    }
    rewrite_expr_idents(&expr, renames)
}

fn is_expr_body_form(head: &str) -> bool {
    matches!(head, "temp" | "var" | "const" | "code")
}

fn is_expr_attr(head: &str, field_name: &str) -> bool {
    matches!((head, field_name), ("while", "when") | ("goto", "script"))
}

fn gensym(runtime: &mut MacroEnv, prefix: &str) -> String {
    runtime.gensym_counter += 1;
    let macro_name = runtime
        .macro_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!(
        "__macro_{}_{}_{}_{}",
        macro_name, runtime.gensym_seed, prefix, runtime.gensym_counter
    )
}

fn splice_expr_slots(source: &str, runtime: &MacroEnv) -> Result<String, ScriptLangError> {
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(start_rel) = source[cursor..].find("${") {
        let start = cursor + start_rel;
        output.push_str(&source[cursor..start]);
        let expr_start = start + 2;
        let Some(end_rel) = source[expr_start..].find('}') else {
            output.push_str(&source[start..]);
            return Ok(output);
        };
        let end = expr_start + end_rel;
        let key = source[expr_start..end].trim();
        let value = runtime.locals.get(key).ok_or_else(|| {
            ScriptLangError::message(format!(
                "unknown macro local `{key}` referenced in expr slot"
            ))
        })?;
        match value {
            MacroValue::Expr(text) | MacroValue::String(text) => output.push_str(text),
            MacroValue::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
            MacroValue::Int(value) => output.push_str(&value.to_string()),
            MacroValue::AstItems(_)
            | MacroValue::Nil
            | MacroValue::Keyword(_)
            | MacroValue::List(_) => {
                return Err(ScriptLangError::message(format!(
                    "macro local `{key}` cannot be spliced into expr slot"
                )));
            }
        }
        cursor = end + 1;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

fn splice_string_slots(source: &str, runtime: &MacroEnv) -> Result<String, ScriptLangError> {
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(start_rel) = source[cursor..].find("${") {
        let start = cursor + start_rel;
        output.push_str(&source[cursor..start]);
        let expr_start = start + 2;
        let Some(end_rel) = source[expr_start..].find('}') else {
            output.push_str(&source[start..]);
            return Ok(output);
        };
        let end = expr_start + end_rel;
        let key = source[expr_start..end].trim();
        let value = runtime.locals.get(key).ok_or_else(|| {
            ScriptLangError::message(format!(
                "unknown macro local `{key}` referenced in string slot"
            ))
        })?;
        match value {
            MacroValue::String(text) => output.push_str(text),
            MacroValue::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
            MacroValue::Int(value) => output.push_str(&value.to_string()),
            MacroValue::List(items) => {
                let formatted = items
                    .iter()
                    .map(macro_value_to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&format!("[{}]", formatted));
            }
            MacroValue::Keyword(kv_pairs) => {
                output.push_str(&macro_keyword_to_string(kv_pairs));
            }
            MacroValue::Expr(_) | MacroValue::AstItems(_) => {
                return Err(ScriptLangError::message(format!(
                    "macro local `{key}` cannot be spliced into string slot"
                )));
            }
            // Nil renders as empty string (enables first-call without prior module state)
            MacroValue::Nil => {}
        }
        cursor = end + 1;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sl_core::{FormMeta, SourcePosition};

    use super::*;
    use crate::semantic::expand::macro_values::MacroValue;

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

    fn runtime_with_locals(locals: BTreeMap<String, MacroValue>) -> MacroEnv {
        MacroEnv {
            current_module: Some("main".to_string()),
            imports: vec!["kernel".to_string()],
            requires: Vec::new(),
            aliases: BTreeMap::new(),
            macro_name: "m".to_string(),
            attributes: BTreeMap::new(),
            content: Vec::new(),
            locals,
            gensym_seed: 0,
            gensym_counter: 0,
            source_file: None,
            line: None,
            column: None,
        }
    }

    #[test]
    fn splice_helpers_cover_scalars_missing_names_and_type_errors() {
        let runtime = runtime_with_locals(BTreeMap::from([
            (
                "expr".to_string(),
                MacroValue::Expr("left + right".to_string()),
            ),
            ("text".to_string(), MacroValue::String("hello".to_string())),
            ("flag".to_string(), MacroValue::Bool(true)),
            ("count".to_string(), MacroValue::Int(3)),
            ("ast".to_string(), MacroValue::AstItems(Vec::new())),
        ]));

        assert_eq!(
            splice_expr_slots("x = ${expr}; ok=${flag}; n=${count}", &runtime)
                .expect("expr splice"),
            "x = left + right; ok=true; n=3"
        );
        assert_eq!(
            splice_string_slots("msg=${text}; ok=${flag}; n=${count}", &runtime)
                .expect("string splice"),
            "msg=hello; ok=true; n=3"
        );
        assert_eq!(
            splice_expr_slots("unterminated ${expr", &runtime).expect("unterminated passthrough"),
            "unterminated ${expr"
        );
        assert_eq!(
            splice_string_slots("unterminated ${text", &runtime)
                .expect("unterminated string passthrough"),
            "unterminated ${text"
        );

        let missing = splice_expr_slots("x=${missing}", &runtime).expect_err("missing local");
        assert!(
            missing
                .to_string()
                .contains("unknown macro local `missing`")
        );

        let wrong_string = splice_string_slots("x=${expr}", &runtime).expect_err("expr in string");
        assert!(
            wrong_string
                .to_string()
                .contains("cannot be spliced into string slot")
        );

        let wrong_expr = splice_expr_slots("x=${ast}", &runtime).expect_err("ast in expr");
        assert!(
            wrong_expr
                .to_string()
                .contains("cannot be spliced into expr slot")
        );
    }

    #[test]
    fn quote_expr_rewrites_scalar_unquotes_and_gensymmed_refs() {
        let mut runtime = runtime_with_locals(BTreeMap::from([
            (
                "when_expr".to_string(),
                MacroValue::Expr("flag".to_string()),
            ),
            ("text".to_string(), MacroValue::String("\"ok\"".to_string())),
            ("flag".to_string(), MacroValue::Bool(false)),
            ("count".to_string(), MacroValue::Int(2)),
            (
                "ast".to_string(),
                MacroValue::AstItems(vec![child(node("text", vec![], vec![]))]),
            ),
        ]));

        let expr = quote_expr(
            &[
                text_item("cond && "),
                child(node("unquote", vec![], vec![text_item("when_expr")])),
                text_item(" && "),
                child(node("unquote", vec![], vec![text_item("flag")])),
                text_item(" && "),
                child(node("unquote", vec![], vec![text_item("count")])),
            ],
            &mut runtime,
            &BTreeMap::from([("cond".to_string(), "__macro_m_0_cond_1".to_string())]),
        )
        .expect("quote expr");
        assert_eq!(expr, "__macro_m_0_cond_1 && flag && false && 2");

        let ast_error = quote_expr(
            &[child(node("unquote", vec![], vec![text_item("ast")]))],
            &mut runtime,
            &BTreeMap::new(),
        )
        .expect_err("ast in expr");
        assert!(
            ast_error
                .to_string()
                .contains("requires scalar compile-time value")
        );

        let form_error = quote_expr(
            &[child(node("text", vec![], vec![]))],
            &mut runtime,
            &BTreeMap::new(),
        )
        .expect_err("nested form");
        assert!(
            form_error
                .to_string()
                .contains("only supports text and <unquote>")
        );
    }

    #[test]
    fn quote_items_cover_temp_hygiene_ast_unquote_and_string_splice() {
        let invocation = node("unless", vec![], vec![]);
        let mut env = ExpandEnv::default();
        let mut runtime = runtime_with_locals(BTreeMap::from([
            (
                "when_expr".to_string(),
                MacroValue::Expr("flag".to_string()),
            ),
            (
                "content_ast".to_string(),
                MacroValue::AstItems(vec![child(node("text", vec![], vec![text_item("hello")]))]),
            ),
            (
                "label".to_string(),
                MacroValue::String("greeting".to_string()),
            ),
        ]));

        let items = quote_items(
            &invocation,
            &mut env,
            ExpandRuleScope::Statement,
            &mut runtime,
            &[
                child(node(
                    "temp",
                    vec![("name", "condition"), ("type", "bool")],
                    vec![child(node("unquote", vec![], vec![text_item("when_expr")]))],
                )),
                child(node(
                    "while",
                    vec![("when", "!condition")],
                    vec![child(node(
                        "unquote",
                        vec![],
                        vec![text_item("content_ast")],
                    ))],
                )),
                child(node(
                    "text",
                    vec![("tag", "${label}")],
                    vec![text_item("ok")],
                )),
                child(node("unquote", vec![], vec![text_item("label")])),
            ],
        )
        .expect("quote items");

        assert_eq!(items.len(), 4);
        let temp = match &items[0] {
            FormItem::Form(form) => form,
            _ => panic!("expected temp form"),
        };
        assert_eq!(attr(temp, "name"), Some("__macro_m_0_condition_1"));
        let while_form = match &items[1] {
            FormItem::Form(form) => form,
            _ => panic!("expected while form"),
        };
        assert_eq!(attr(while_form, "when"), Some("!__macro_m_0_condition_1"));
        let text_form = match &items[2] {
            FormItem::Form(form) => form,
            _ => panic!("expected text form"),
        };
        assert_eq!(attr(text_form, "tag"), Some("greeting"));
        assert!(matches!(&items[3], FormItem::Text(text) if text == "greeting"));
    }

    #[test]
    fn quote_items_and_helpers_cover_error_paths() {
        let invocation = node("m", vec![], vec![]);
        let mut env = ExpandEnv::default();
        let mut runtime = runtime_with_locals(BTreeMap::from([
            ("expr".to_string(), MacroValue::Expr("flag".to_string())),
            ("label".to_string(), MacroValue::String("hello".to_string())),
        ]));

        let ast_child_error = quote_items(
            &invocation,
            &mut env,
            ExpandRuleScope::Statement,
            &mut runtime,
            &[child(node("unquote", vec![], vec![text_item("expr")]))],
        )
        .expect_err("expr in ast child position");
        assert!(
            ast_child_error
                .to_string()
                .contains("requires `ast` or `string` value")
        );

        let form_result = quote_form(
            &invocation,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
            &mut runtime_with_locals(BTreeMap::from([(
                "label".to_string(),
                MacroValue::String("hello".to_string()),
            )])),
            &form(
                "goto",
                vec![
                    attr_field("script", "${label}"),
                    children_field(vec![text_item("ignored")]),
                ],
            ),
            &mut BTreeMap::new(),
        )
        .expect("quote form");
        assert_eq!(attr(&form_result, "script"), Some("hello"));

        assert_eq!(gensym(&mut runtime, "label"), "__macro_m_0_label_1");
    }

    #[test]
    fn splice_string_slots_reports_missing_and_wrong_types() {
        let runtime = runtime_with_locals(BTreeMap::from([
            ("expr".to_string(), MacroValue::Expr("flag".to_string())),
            ("text".to_string(), MacroValue::String("hello".to_string())),
        ]));

        let missing = splice_string_slots("x=${missing}", &runtime).expect_err("missing");
        assert!(
            missing
                .to_string()
                .contains("unknown macro local `missing` referenced in string slot")
        );

        let wrong = splice_string_slots("x=${expr}", &runtime).expect_err("wrong type");
        assert!(
            wrong
                .to_string()
                .contains("cannot be spliced into string slot")
        );

        assert_eq!(
            splice_string_slots("x=${text", &runtime).expect("unterminated passthrough"),
            "x=${text"
        );
    }

    #[test]
    fn macro_keyword_to_string_coverts_kv_pairs_to_text() {
        use super::macro_keyword_to_string;
        // Simple keyword
        assert_eq!(
            macro_keyword_to_string(&[
                ("mode".to_string(), MacroValue::String("debug".to_string())),
                ("async".to_string(), MacroValue::Bool(true)),
            ]),
            "mode:debug,async:true"
        );
        // Nested keyword
        assert_eq!(
            macro_keyword_to_string(&[(
                "config".to_string(),
                MacroValue::Keyword(vec![("timeout".to_string(), MacroValue::Int(30)),])
            ),]),
            "config:{timeout:30}"
        );
        // List in keyword
        assert_eq!(
            macro_keyword_to_string(&[(
                "items".to_string(),
                MacroValue::List(vec![
                    MacroValue::String("a".to_string()),
                    MacroValue::String("b".to_string()),
                ])
            ),]),
            "items:[a, b]"
        );
    }

    #[test]
    fn quote_ast_items_expands_list_unquote_into_multiple_form_items() {
        // When a MacroValue::List is unquoted in AST children position,
        // each element should become a separate FormItem::Text
        let invocation = node("quote", vec![], vec![]);
        let mut env = ExpandEnv::default();
        let mut runtime = runtime_with_locals(BTreeMap::from([(
            "items".to_string(),
            MacroValue::List(vec![
                MacroValue::String("foo".to_string()),
                MacroValue::String("bar".to_string()),
            ]),
        )]));

        let items = quote_items(
            &invocation,
            &mut env,
            ExpandRuleScope::Statement,
            &mut runtime,
            &[
                text_item("prefix "),
                child(node("unquote", vec![], vec![text_item("items")])),
                text_item(" suffix"),
            ],
        )
        .expect("quote with list unquote");

        assert_eq!(items.len(), 4);
        let texts: Vec<&str> = items
            .iter()
            .filter_map(|i| match i {
                FormItem::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec!["prefix ", "foo", "bar", " suffix"]);
    }

    #[test]
    fn quote_ast_items_stringifies_keyword_unquote() {
        // When a MacroValue::Keyword is unquoted in AST children position,
        // it should stringify to "key:val,..." format
        let invocation = node("quote", vec![], vec![]);
        let mut env = ExpandEnv::default();
        let mut runtime = runtime_with_locals(BTreeMap::from([(
            "opts".to_string(),
            MacroValue::Keyword(vec![
                ("mode".to_string(), MacroValue::String("debug".to_string())),
                ("verbose".to_string(), MacroValue::Bool(true)),
            ]),
        )]));

        let items = quote_items(
            &invocation,
            &mut env,
            ExpandRuleScope::Statement,
            &mut runtime,
            &[
                text_item("cfg: "),
                child(node("unquote", vec![], vec![text_item("opts")])),
            ],
        )
        .expect("quote with keyword unquote");

        assert_eq!(items.len(), 2);
        match &items[1] {
            FormItem::Text(t) => assert_eq!(t, "mode:debug,verbose:true"),
            other => panic!("expected Text, got {:?}", other),
        }
    }
}
