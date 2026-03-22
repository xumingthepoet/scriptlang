//! Convert old XML macro body format to new compile-time AST.
//!
//! This module bridges the gap between the old template-based macro body
//! and the new compile-time language AST.

use super::{CtBlock, CtExpr, CtStmt, CtValue};
use crate::semantic::{attr, error_at, required_attr};
use sl_core::{Form, FormItem, ScriptLangError};

/// Convert old XML macro body to CtBlock.
///
/// All forms (including <quote>) are converted to CtStmt:
/// - <quote> children become CtStmt::Return { value: CtExpr::QuoteForms { items } }
/// - <let>, <set>, <if>, <return> are converted to their CtStmt equivalents
#[allow(dead_code)]
pub fn convert_macro_body(body: &[FormItem]) -> Result<CtBlock, ScriptLangError> {
    let mut stmts = Vec::new();

    for item in body {
        match item {
            FormItem::Text(text) if text.trim().is_empty() => {}
            FormItem::Text(_) => {
                return Err(ScriptLangError::message(
                    "unexpected top-level text inside macro body",
                ));
            }
            FormItem::Form(form) => {
                if form.head == "quote" {
                    // Inline quote as return with QuoteForms
                    let children = extract_form_children(form)?;
                    stmts.push(CtStmt::Return {
                        value: CtExpr::QuoteForms { items: children },
                    });
                } else {
                    let stmt = convert_form_to_stmt(form)?;
                    stmts.push(stmt);
                }
            }
        }
    }

    Ok(CtBlock { stmts })
}

/// Convert a form to a compile-time statement.
#[allow(dead_code)]
fn convert_form_to_stmt(form: &Form) -> Result<CtStmt, ScriptLangError> {
    match form.head.as_str() {
        "let" => convert_let_form(form),
        "set" => convert_set_form(form),
        "if" => convert_if_form(form),
        "return" => convert_return_form(form),
        other => Err(error_at(
            form,
            format!("unsupported compile-time macro form <{}>", other),
        )),
    }
}

/// Convert `<let name="..." type="...">provider</let>` to CtStmt::Let.
#[allow(dead_code)]
fn convert_let_form(form: &Form) -> Result<CtStmt, ScriptLangError> {
    let name = required_attr(form, "name")?.to_string();
    let type_name = required_attr(form, "type")?;
    let provider = single_child_form(form)?;

    // Handle special forms that are valid as let providers
    if provider.head.as_str() == "caller_module" {
        // <caller_module/> as a let provider: call builtin_caller_module()
        let value = CtExpr::BuiltinCall {
            name: "caller_module".to_string(),
            args: vec![],
        };
        return Ok(CtStmt::Let { name, value });
    }

    let value = convert_provider_to_expr(&provider, type_name)?;

    Ok(CtStmt::Let { name, value })
}

/// Convert `<set name="...">expr</set>` to CtStmt::Set.
#[allow(dead_code)]
fn convert_set_form(form: &Form) -> Result<CtStmt, ScriptLangError> {
    let name = required_attr(form, "name")?.to_string();
    let expr_form = single_child_form(form)?;
    let value = convert_expr_form(&expr_form)?;

    Ok(CtStmt::Set { name, value })
}

/// Convert `<if>cond then else?</if>` to CtStmt::If.
#[allow(dead_code)]
fn convert_if_form(form: &Form) -> Result<CtStmt, ScriptLangError> {
    let children = extract_form_children(form)?;

    if children.is_empty() {
        return Err(error_at(
            form,
            "<if> requires at least condition and then block",
        ));
    }

    // First child is the condition expression
    let cond_form = child_form_at(&children, 0, form, "<if> condition")?;
    let cond = convert_expr_form(cond_form)?;

    // Second child is the then block
    let then_form = child_form_at(&children, 1, form, "<if> <then>")?;
    if then_form.head != "then" {
        return Err(error_at(form, "<if> second child must be <then> block"));
    }
    let then_items = extract_form_children(then_form)?;
    let then_block = convert_macro_body(&then_items)?;

    // Optional third child is the else block
    let else_block = if children.len() > 2 {
        let else_form = child_form_at(&children, 2, form, "<if> <else>")?;
        if else_form.head != "else" {
            return Err(error_at(form, "<if> third child must be <else> block"));
        }
        let else_items = extract_form_children(else_form)?;
        Some(convert_macro_body(&else_items)?)
    } else {
        None
    };

    Ok(CtStmt::If {
        cond,
        then_block,
        else_block,
    })
}

/// Convert `<return>expr</return>` to CtStmt::Return.
#[allow(dead_code)]
fn convert_return_form(form: &Form) -> Result<CtStmt, ScriptLangError> {
    let children = extract_form_children(form)?;
    if children.is_empty() {
        // Return nil
        Ok(CtStmt::Return {
            value: CtExpr::Literal(CtValue::Nil),
        })
    } else {
        let expr_form = child_form_at(&children, 0, form, "<return>")?;
        let value = convert_expr_form(expr_form)?;
        Ok(CtStmt::Return { value })
    }
}

/// Extract the children field from a form.
#[allow(dead_code)]
fn extract_form_children(form: &Form) -> Result<Vec<FormItem>, ScriptLangError> {
    form.fields
        .iter()
        .find_map(|field| match (&field.name[..], &field.value) {
            ("children", sl_core::FormValue::Sequence(items)) => Some(items.clone()),
            _ => None,
        })
        .ok_or_else(|| error_at(form, format!("<{}> requires `children`", form.head)))
}

/// Get a child form at the given index, filtering out empty text.
#[allow(dead_code)]
fn child_form_at<'a>(
    children: &'a [FormItem],
    index: usize,
    context: &Form,
    context_name: &str,
) -> Result<&'a Form, ScriptLangError> {
    let meaningful: Vec<_> = children
        .iter()
        .filter(|item| !matches!(item, FormItem::Text(text) if text.trim().is_empty()))
        .collect();

    let item = meaningful.get(index).ok_or_else(|| {
        error_at(
            context,
            format!("{} requires child at position {}", context_name, index),
        )
    })?;

    match item {
        FormItem::Form(form) => Ok(form),
        FormItem::Text(_) => Err(error_at(
            context,
            format!("{} expected form child", context_name),
        )),
    }
}

/// Convert a provider form to a CtExpr based on its type.
#[allow(dead_code)]
fn convert_provider_to_expr(form: &Form, type_name: &str) -> Result<CtExpr, ScriptLangError> {
    match form.head.as_str() {
        "get-attribute" => {
            let attr_name = required_attr(form, "name")?;
            let attr_expr = CtExpr::BuiltinCall {
                name: "attr".to_string(),
                args: vec![CtExpr::Literal(CtValue::String(attr_name.to_string()))],
            };

            // Apply type conversion based on the declared type
            match type_name {
                "expr" | "string" => Ok(attr_expr),
                "bool" => Ok(CtExpr::BuiltinCall {
                    name: "parse_bool".to_string(),
                    args: vec![attr_expr],
                }),
                "int" => Ok(CtExpr::BuiltinCall {
                    name: "parse_int".to_string(),
                    args: vec![attr_expr],
                }),
                "keyword" => {
                    // keyword_attr(name) retrieves the keyword MacroValue from macro_env.locals
                    // and converts it to CtValue::Keyword
                    Ok(CtExpr::BuiltinCall {
                        name: "keyword_attr".to_string(),
                        args: vec![CtExpr::Literal(CtValue::String(attr_name.to_string()))],
                    })
                }
                other => Err(error_at(
                    form,
                    format!("unsupported macro let type `{}` for <get-attribute>", other),
                )),
            }
        }
        "get-content" => {
            if type_name != "ast" {
                return Err(error_at(
                    form,
                    format!(
                        "<get-content> provider requires type `ast`, got `{}`",
                        type_name
                    ),
                ));
            }
            let head_filter = attr(form, "head");
            let args = if let Some(head) = head_filter {
                vec![CtExpr::Literal(CtValue::Keyword(vec![(
                    "head".to_string(),
                    CtValue::String(head.to_string()),
                )]))]
            } else {
                vec![]
            };
            Ok(CtExpr::BuiltinCall {
                name: "content".to_string(),
                args,
            })
        }
        "quote" => {
            // <quote> as a provider: extract children and return as QuoteForms
            if type_name != "ast" {
                return Err(error_at(
                    form,
                    format!("<quote> provider requires type `ast`, got `{}`", type_name),
                ));
            }
            let children = extract_form_children(form)?;
            Ok(CtExpr::QuoteForms { items: children })
        }
        other => Err(error_at(
            form,
            format!("unsupported <{}> provider for macro let", other),
        )),
    }
}

/// Convert an expression form to CtExpr.
#[allow(dead_code)]
fn convert_expr_form(form: &Form) -> Result<CtExpr, ScriptLangError> {
    match form.head.as_str() {
        "get-attribute" => {
            let attr_name = required_attr(form, "name")?;
            Ok(CtExpr::BuiltinCall {
                name: "attr".to_string(),
                args: vec![CtExpr::Literal(CtValue::String(attr_name.to_string()))],
            })
        }
        "get-content" => {
            let head_filter = attr(form, "head");
            let args = if let Some(head) = head_filter {
                vec![CtExpr::Literal(CtValue::Keyword(vec![(
                    "head".to_string(),
                    CtValue::String(head.to_string()),
                )]))]
            } else {
                vec![]
            };
            Ok(CtExpr::BuiltinCall {
                name: "content".to_string(),
                args,
            })
        }
        "caller_module" => {
            // No attributes needed; calls builtin_caller_module()
            Ok(CtExpr::BuiltinCall {
                name: "caller_module".to_string(),
                args: vec![],
            })
        }
        other => Err(error_at(
            form,
            format!("unsupported expression form <{}>", other),
        )),
    }
}

/// Get the single meaningful child form, cloning it.
#[allow(dead_code)]
fn single_child_form(form: &Form) -> Result<Form, ScriptLangError> {
    let children = extract_form_children(form)?;
    let meaningful: Vec<_> = children
        .iter()
        .filter(|item| !matches!(item, FormItem::Text(text) if text.trim().is_empty()))
        .collect();

    if meaningful.len() != 1 {
        return Err(error_at(
            form,
            "macro compile-time form requires exactly one meaningful child",
        ));
    }

    match meaningful[0] {
        FormItem::Form(child) => Ok(child.clone()),
        FormItem::Text(_) => Err(error_at(form, "expected child form")),
    }
}
