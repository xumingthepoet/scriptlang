use std::collections::BTreeMap;

use sl_core::{Form, FormField, FormItem, FormValue, ScriptLangError};

use super::dispatch::{ExpandRuleScope, expand_form_items};
use super::macro_env::MacroEnv;
use super::macro_eval::eval_unquote;
use super::macro_values::MacroValue;
use super::string_attr;
use crate::semantic::env::ExpandEnv;
use crate::semantic::error_at;
use crate::semantic::expr::rewrite_expr_idents;

pub(super) fn quote_items(
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
        string_attr(form, "name").map(str::to_string)
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
                MacroValue::AstItems(_) => {
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
    matches!((head, field_name), ("if", "when") | ("goto", "script"))
}

fn gensym(runtime: &mut MacroEnv, prefix: &str) -> String {
    runtime.gensym_counter += 1;
    format!("__macro_{}_{}", prefix, runtime.gensym_counter)
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
            MacroValue::AstItems(_) => {
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
            MacroValue::Expr(_) | MacroValue::AstItems(_) => {
                return Err(ScriptLangError::message(format!(
                    "macro local `{key}` cannot be spliced into string slot"
                )));
            }
        }
        cursor = end + 1;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}
