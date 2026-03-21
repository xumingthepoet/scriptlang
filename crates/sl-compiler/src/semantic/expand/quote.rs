use std::collections::BTreeMap;

use sl_core::{Form, FormField, FormItem, FormValue, ScriptLangError};

use super::dispatch::{ExpandRuleScope, expand_form_items};
use super::macro_eval::{MacroRuntimeEnv, MacroValue, eval_unquote};
use super::string_attr;
use crate::semantic::env::ExpandEnv;
use crate::semantic::error_at;
use crate::semantic::expr::rewrite_expr_idents;

pub(super) fn quote_items(
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
    runtime: &mut MacroRuntimeEnv,
    items: &[FormItem],
) -> Result<Vec<FormItem>, ScriptLangError> {
    let mut renames = BTreeMap::new();
    quote_ast_items(invocation, env, scope, runtime, items, &mut renames)
}

fn quote_ast_items(
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
    runtime: &mut MacroRuntimeEnv,
    items: &[FormItem],
    renames: &mut BTreeMap<String, String>,
) -> Result<Vec<FormItem>, ScriptLangError> {
    let mut output = Vec::new();
    for item in items {
        match item {
            FormItem::Text(text) => output.push(FormItem::Text(text.clone())),
            FormItem::Form(form) if form.head == "unquote" => match eval_unquote(form, runtime)? {
                MacroValue::AstItems(items) => output.extend(items),
                MacroValue::Expr(_) | MacroValue::String(_) => {
                    return Err(error_at(
                        form,
                        "<unquote> in AST children position requires `ast` value",
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
    runtime: &mut MacroRuntimeEnv,
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
                    FormValue::String(rewrite_expr_idents(text, renames)?)
                } else {
                    FormValue::String(text.clone())
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
    runtime: &mut MacroRuntimeEnv,
    renames: &BTreeMap<String, String>,
) -> Result<String, ScriptLangError> {
    let mut expr = String::new();
    for item in items {
        match item {
            FormItem::Text(text) => expr.push_str(text),
            FormItem::Form(form) if form.head == "unquote" => match eval_unquote(form, runtime)? {
                MacroValue::Expr(value) | MacroValue::String(value) => expr.push_str(&value),
                MacroValue::AstItems(_) => {
                    return Err(error_at(
                        form,
                        "<unquote> in expr position requires `expr` or `string` value",
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

fn gensym(runtime: &mut MacroRuntimeEnv, prefix: &str) -> String {
    runtime.gensym_counter += 1;
    format!("__macro_{}_{}", prefix, runtime.gensym_counter)
}
