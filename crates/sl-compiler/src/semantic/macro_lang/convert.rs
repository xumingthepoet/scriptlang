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
                    // Inline quote as return with QuoteForms (eager by default)
                    let children = extract_form_children(form)?;
                    stmts.push(CtStmt::Return {
                        value: CtExpr::QuoteForms {
                            items: children,
                            lazy: false,
                        },
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
fn convert_form_to_stmt(form: &Form) -> Result<CtStmt, ScriptLangError> {
    match form.head.as_str() {
        "let" => convert_let_form(form),
        "set" => convert_set_form(form),
        "if" => convert_if_form(form),
        "return" => convert_return_form(form),
        // Step 5: support standalone builtins as statements (for side effects)
        "require_module" => {
            // <require_module><attr name="module"/></require_module>
            let child = single_child_form(form)?;
            let arg = convert_expr_form(&child)?;
            Ok(CtStmt::Expr {
                expr: CtExpr::BuiltinCall {
                    name: "require_module".to_string(),
                    args: vec![arg],
                },
            })
        }
        "expand_alias" => {
            let child = single_child_form(form)?;
            let arg = convert_expr_form(&child)?;
            Ok(CtStmt::Expr {
                expr: CtExpr::BuiltinCall {
                    name: "expand_alias".to_string(),
                    args: vec![arg],
                },
            })
        }
        "keyword_attr" => {
            let name = required_attr(form, "name")?;
            Ok(CtStmt::Expr {
                expr: CtExpr::BuiltinCall {
                    name: "keyword_attr".to_string(),
                    args: vec![CtExpr::Literal(CtValue::String(name.to_string()))],
                },
            })
        }
        "invoke_macro" => {
            let expr = convert_expr_form(form)?;
            Ok(CtStmt::Return { value: expr })
        }
        // Step 5.2: <builtin name="fn"><arg1/><arg2/></builtin> as a statement (for side effects like module_put)
        "builtin" => {
            let name = required_attr(form, "name")?;
            let children = extract_form_children(form)?;
            let args = extract_expr_forms(&children);
            Ok(CtStmt::Expr {
                expr: CtExpr::BuiltinCall {
                    name: name.to_string(),
                    args,
                },
            })
        }
        // Step 5: <quote> at top level returns QuoteForms
        "quote" => {
            let children = extract_form_children(form)?;
            Ok(CtStmt::Return {
                value: CtExpr::QuoteForms {
                    items: children,
                    lazy: false,
                },
            })
        }
        other => Err(unsupported_form_error(form, "compile-time macro", other)),
    }
}

/// Convert `<let name="..." type="...">provider</let>` to CtStmt::Let.
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

    // Handle <builtin name="..."> as a let provider (Step 3.2)
    if provider.head.as_str() == "builtin" {
        let builtin_name = required_attr(&provider, "name")?;
        let children = extract_form_children(&provider)?;
        let args = extract_expr_forms(&children);
        let value = CtExpr::BuiltinCall {
            name: builtin_name.to_string(),
            args,
        };
        return Ok(CtStmt::Let { name, value });
    }

    // Handle <require_module> as a let provider (returns expanded module name as string)
    if provider.head.as_str() == "require_module" {
        let inner = single_child_form(&provider)?;
        let arg = convert_expr_form(&inner)?;
        return Ok(CtStmt::Let {
            name,
            value: CtExpr::BuiltinCall {
                name: "require_module".to_string(),
                args: vec![arg],
            },
        });
    }

    let value = convert_provider_to_expr(&provider, type_name)?;

    Ok(CtStmt::Let { name, value })
}

/// Convert `<set name="...">expr</set>` to CtStmt::Set.
fn convert_set_form(form: &Form) -> Result<CtStmt, ScriptLangError> {
    let name = required_attr(form, "name")?.to_string();
    let expr_form = single_child_form(form)?;
    let value = convert_expr_form(&expr_form)?;

    Ok(CtStmt::Set { name, value })
}

/// Convert `<if>cond then else?</if>` to CtStmt::If.
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
fn child_form_at<'a>(
    children: &'a [FormItem],
    index: usize,
    context: &Form,
    context_name: &str,
) -> Result<&'a Form, ScriptLangError> {
    let meaningful = meaningful_items(children);

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

/// Parse the module expression from child form elements (<var>, <get-attribute>, or <literal>).
/// Used as fallback when `module` attribute is absent.
fn parse_module_from_child(form: &Form) -> Result<CtExpr, ScriptLangError> {
    let children = extract_form_children(form)?;
    let meaningful = meaningful_items(&children);
    let child = meaningful.first().ok_or_else(|| {
        error_at(
            form,
            "<invoke_macro> requires module attribute or <var>/<get-attribute>/<literal> child",
        )
    })?;
    let child_form = match child {
        FormItem::Form(f) => f,
        FormItem::Text(_) => {
            return Err(error_at(
                form,
                "<invoke_macro> child must be a form element",
            ));
        }
    };
    match child_form.head.as_str() {
        "var" => {
            let var_name = attr(child_form, "name").unwrap_or("module");
            Ok(CtExpr::Var {
                name: var_name.to_string(),
            })
        }
        "get-attribute" | "literal" => convert_expr_form(child_form),
        _ => Err(error_at(
            form,
            "<invoke_macro> child must be <var>, <get-attribute>, or <literal>",
        )),
    }
}

/// Parse the opts expression from a child <keyword_attr name="opts"/> form.
/// Used as fallback when `opts` attribute is absent.
fn parse_opts_from_child(form: &Form) -> Result<CtExpr, ScriptLangError> {
    let children = extract_form_children(form)?;
    let meaningful = meaningful_items(&children);
    let child = meaningful
        .iter()
        .find(|c| matches!(c, FormItem::Form(f) if f.head == "keyword_attr"))
        .ok_or_else(|| {
            error_at(
                form,
                "<invoke_macro> requires opts=\"opts\" attribute or <keyword_attr name=\"opts\"/> child",
            )
        })?;
    match child {
        FormItem::Form(child_form) => convert_expr_form(child_form),
        FormItem::Text(_) => Err(error_at(
            form,
            "<invoke_macro> child must be a form element",
        )),
    }
}

/// Extract meaningful (non-whitespace-text) FormItems from a sequence.
fn meaningful_items(items: &[FormItem]) -> Vec<&FormItem> {
    items
        .iter()
        .filter(|item| !matches!(item, FormItem::Text(text) if text.trim().is_empty()))
        .collect()
}

/// Convert a provider form to a CtExpr based on its type.
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
            require_ast_type("get-content", type_name, form)?;
            let args = compile_content_call(form);
            Ok(CtExpr::BuiltinCall {
                name: "content".to_string(),
                args,
            })
        }
        "quote" => {
            // <quote> as a provider: extract children and return as QuoteForms (eager)
            require_ast_type("quote", type_name, form)?;
            let children = extract_form_children(form)?;
            Ok(CtExpr::QuoteForms {
                items: children,
                lazy: false,
            })
        }
        other => Err(unsupported_form_error(
            form,
            "provider for macro let",
            other,
        )),
    }
}

/// Convert an expression form to CtExpr.
fn convert_expr_form(form: &Form) -> Result<CtExpr, ScriptLangError> {
    match form.head.as_str() {
        "get-attribute" => {
            let attr_name = required_attr(form, "name")?;
            Ok(CtExpr::BuiltinCall {
                name: "attr".to_string(),
                args: vec![CtExpr::Literal(CtValue::String(attr_name.to_string()))],
            })
        }
        // Step 5: <var name="X"/> -> CtExpr::Var (reference a bound macro parameter)
        "var" => {
            let var_name = required_attr(form, "name")?;
            Ok(CtExpr::Var {
                name: var_name.to_string(),
            })
        }
        "get-content" => {
            let args = compile_content_call(form);
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
        // Step 5: <require_module><child_expr/></require_module> -> builtin_call
        "require_module" => {
            let child = single_child_form(form)?;
            let arg = convert_expr_form(&child)?;
            Ok(CtExpr::BuiltinCall {
                name: "require_module".to_string(),
                args: vec![arg],
            })
        }
        // Step 5: <expand_alias><child_expr/></expand_alias> -> builtin_call
        "expand_alias" => {
            let child = single_child_form(form)?;
            let arg = convert_expr_form(&child)?;
            Ok(CtExpr::BuiltinCall {
                name: "expand_alias".to_string(),
                args: vec![arg],
            })
        }
        // Step 5: <keyword_attr name="x"/> -> builtin_call
        "keyword_attr" => {
            let name = required_attr(form, "name")?;
            Ok(CtExpr::BuiltinCall {
                name: "keyword_attr".to_string(),
                args: vec![CtExpr::Literal(CtValue::String(name.to_string()))],
            })
        }
        // Step 3.2: <builtin name="fn"><arg1/><arg2/>...</builtin> -> builtin_call
        "builtin" => {
            let name = required_attr(form, "name")?;
            let children = extract_form_children(form)?;
            let args: Vec<CtExpr> = children
                .iter()
                .filter_map(|item| {
                    if let FormItem::Form(f) = item {
                        convert_expr_form(f).ok()
                    } else {
                        None
                    }
                })
                .collect();
            Ok(CtExpr::BuiltinCall {
                name: name.to_string(),
                args,
            })
        }
        // Step 3.2: <literal value="..."/> -> CtExpr::Literal(CtValue::String(...))
        "literal" => {
            let value = required_attr(form, "value")?;
            Ok(CtExpr::Literal(CtValue::String(value.to_string())))
        }
        // Step 5: <invoke_macro module="..." macro_name="__using__" opts="opts"/> -> builtin_call
        "invoke_macro" => {
            // module: if it's a bound variable name (alphanumeric + underscore), use CtExpr::Var;
            // otherwise treat as a literal string (attr() builtin)
            let module_expr = match required_attr(form, "module") {
                Ok(module_attr) if module_attr.chars().all(|c| c.is_alphanumeric() || c == '_') => {
                    CtExpr::Var {
                        name: module_attr.to_string(),
                    }
                }
                Ok(module_attr) => CtExpr::BuiltinCall {
                    name: "attr".to_string(),
                    args: vec![CtExpr::Literal(CtValue::String(module_attr.to_string()))],
                },
                Err(_) => parse_module_from_child(form)?,
            };
            // macro_name attribute: "__using__"
            let macro_name = required_attr(form, "macro_name")?;
            // opts: if opts="opts" attribute is present, use CtExpr::Var; else parse from child
            let opts_expr = match attr(form, "opts") {
                Some("opts") => CtExpr::Var {
                    name: "opts".to_string(),
                },
                Some(opts_name) => {
                    return Err(error_at(
                        form,
                        format!(
                            "<invoke_macro> opts attribute must be 'opts', got '{}'",
                            opts_name
                        ),
                    ));
                }
                None => parse_opts_from_child(form)?,
            };
            Ok(CtExpr::BuiltinCall {
                name: "invoke_macro".to_string(),
                args: vec![
                    module_expr,
                    CtExpr::Literal(CtValue::String(macro_name.to_string())),
                    opts_expr,
                ],
            })
        }
        // Step 5: <quote>...</quote> as expression: produce QuoteForms.
        // Supports lazy="true" attribute for list_map/list_fold callbacks where
        // string interpolation (${var}) must resolve with loop variable bound.
        "quote" => {
            let children = extract_form_children(form)?;
            let lazy = attr(form, "lazy").map(|v| v == "true").unwrap_or(false);
            Ok(CtExpr::QuoteForms {
                items: children,
                lazy,
            })
        }
        other => Err(unsupported_form_error(form, "expression", other)),
    }
}

/// Convert form children to a list of CtExpr, skipping non-Form items.
fn extract_expr_forms(children: &[FormItem]) -> Vec<CtExpr> {
    children
        .iter()
        .filter_map(|item| {
            if let FormItem::Form(f) = item {
                convert_expr_form(f).ok()
            } else {
                None
            }
        })
        .collect()
}

/// Get the single meaningful child form, cloning it.
fn single_child_form(form: &Form) -> Result<Form, ScriptLangError> {
    let children = extract_form_children(form)?;
    let meaningful = meaningful_items(&children);

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

/// Check that a provider requires `ast` declared type, returning an error if not.
fn require_ast_type(provider: &str, type_name: &str, form: &Form) -> Result<(), ScriptLangError> {
    if type_name != "ast" {
        return Err(error_at(
            form,
            format!(
                "<{}> provider requires type `ast`, got `{}`",
                provider, type_name
            ),
        ));
    }
    Ok(())
}

/// Build the args vector for a `content()` builtin call from the optional `head` attribute.
fn compile_content_call(form: &Form) -> Vec<CtExpr> {
    match attr(form, "head") {
        Some(head) => vec![CtExpr::Literal(CtValue::Keyword(vec![(
            "head".to_string(),
            CtValue::String(head.to_string()),
        )]))],
        None => vec![],
    }
}

/// Build a standardized "unsupported X form" error.
fn unsupported_form_error(form: &Form, kind: &str, name: &str) -> ScriptLangError {
    error_at(form, format!("unsupported {} form <{}>", kind, name))
}
