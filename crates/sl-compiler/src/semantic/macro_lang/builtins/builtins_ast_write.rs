//! AST write builtin functions.

use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::macro_lang::builtins::{BuiltinRegistry, BuiltinResult};
use crate::semantic::macro_lang::{CtEnv, CtValue};
use sl_core::ScriptLangError;
use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

/// Extract the first Form from a CtValue::Ast, or error if none exists.
fn extract_first_form(ast: &[FormItem]) -> Result<&Form, ScriptLangError> {
    for item in ast {
        if let FormItem::Form(form) = item {
            return Ok(form);
        }
    }
    Err(ScriptLangError::Message {
        message: "ast has no form elements (only text)".to_string(),
    })
}

/// `ast_attr_set(ast, key, value)`: Return a new ast with the attribute set on the first form.
/// Does NOT modify the original ast (immutable).
pub(crate) fn builtin_ast_attr_set(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 3 {
        return Err(ScriptLangError::Message {
            message: "ast_attr_set() requires exactly 3 arguments: ast, key, value".to_string(),
        });
    }

    let ast = match &args[0] {
        CtValue::Ast(items) => items,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "ast_attr_set() first argument must be ast, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let key = match &args[1] {
        CtValue::String(s) => s,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "ast_attr_set() second argument (key) must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let value = match &args[2] {
        CtValue::String(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "ast_attr_set() third argument (value) must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let form = extract_first_form(ast)?;
    // Build new fields: update or insert the attribute
    let mut new_fields: Vec<FormField> = form
        .fields
        .iter()
        .filter(|f| f.name != *key)
        .cloned()
        .collect();
    new_fields.push(FormField {
        name: key.clone(),
        value: FormValue::String(value),
    });

    let new_form = Form {
        head: form.head.clone(),
        meta: form.meta.clone(),
        fields: new_fields,
    };

    // Replace the first form in the AST with the new form
    let mut new_items: Vec<FormItem> = Vec::with_capacity(ast.len());
    let mut replaced = false;
    for item in ast {
        if !replaced && matches!(item, FormItem::Form(_)) {
            new_items.push(FormItem::Form(new_form.clone()));
            replaced = true;
            continue;
        }
        new_items.push(item.clone());
    }

    Ok(CtValue::Ast(new_items))
}

/// Convert a CtValue to a FormValue for use in form fields.
fn ct_value_to_form_field_value(v: &CtValue) -> FormValue {
    match v {
        CtValue::String(s) => FormValue::String(s.clone()),
        CtValue::Int(i) => FormValue::String(i.to_string()),
        CtValue::Bool(b) => FormValue::String(b.to_string()),
        _ => FormValue::String(format!("{:?}", v)),
    }
}

/// `ast_wrap(inner_ast, head, extra_attrs?)`: Wrap the inner AST items in a new form with the given head.
/// Optionally accepts extra attributes as a keyword list: ast_wrap(ast, "head", [key1: val1, key2: val2])
/// Returns CtValue::Ast containing a single FormItem::Form.
pub(crate) fn builtin_ast_wrap(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 2 && args.len() != 3 {
        return Err(ScriptLangError::Message {
            message: "ast_wrap() requires 2 or 3 arguments: inner_ast, head, extra_attrs?"
                .to_string(),
        });
    }

    let inner_ast = match &args[0] {
        CtValue::Ast(items) => items.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "ast_wrap() first argument must be ast, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let head = match &args[1] {
        CtValue::String(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "ast_wrap() second argument (head) must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    // Parse extra_attrs: either a keyword list or a list of [key, value] pairs
    let mut extra_fields: Vec<FormField> = Vec::new();
    if args.len() == 3 {
        match &args[2] {
            CtValue::Keyword(kv) => {
                for (k, v) in kv {
                    extra_fields.push(FormField {
                        name: k.clone(),
                        value: ct_value_to_form_field_value(v),
                    });
                }
            }
            CtValue::List(items) => {
                for item in items {
                    if let CtValue::Keyword(kv) = item {
                        for (k, v) in kv {
                            extra_fields.push(FormField {
                                name: k.clone(),
                                value: ct_value_to_form_field_value(v),
                            });
                        }
                    }
                }
            }
            other => {
                return Err(ScriptLangError::Message {
                    message: format!(
                        "ast_wrap() third argument (extra_attrs) must be keyword or list, got {}",
                        other.type_name()
                    ),
                });
            }
        }
    }

    let mut fields = vec![FormField {
        name: "children".to_string(),
        value: FormValue::Sequence(inner_ast),
    }];
    fields.extend(extra_fields);

    let wrapper = Form {
        head,
        meta: FormMeta {
            source_name: None,
            start: SourcePosition { row: 0, column: 0 },
            start_byte: 0,
            end: SourcePosition { row: 0, column: 0 },
            end_byte: 0,
        },
        fields,
    };

    Ok(CtValue::Ast(vec![FormItem::Form(wrapper)]))
}

/// `ast_concat(...asts)`: Concatenate multiple ASTs into one.
/// Accepts either varargs: ast_concat(ast1, ast2, ...) or a list: ast_concat([ast1, ast2, ...]).
/// Returns a flat list of FormItems.
pub(crate) fn builtin_ast_concat(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    // Collect all AST items from varargs or a single list argument
    let mut all_items: Vec<CtValue> = Vec::new();

    if args.is_empty() {
        return Err(ScriptLangError::Message {
            message: "ast_concat() requires at least 1 argument".to_string(),
        });
    }

    // Check if first arg is a list (old-style single-list argument)
    if args.len() == 1 {
        if let CtValue::List(items) = &args[0] {
            all_items.extend(items.iter().cloned());
        } else if let CtValue::Ast(items) = &args[0] {
            // Single AST argument: treat it as a single-item "list"
            all_items.push(CtValue::Ast(items.clone()));
        } else {
            return Err(ScriptLangError::Message {
                message: format!(
                    "ast_concat() argument must be ast or list, got {}",
                    args[0].type_name()
                ),
            });
        }
    } else {
        // Varargs style: each argument must be an AST
        for arg in args {
            match arg {
                CtValue::Ast(items) => all_items.push(CtValue::Ast(items.clone())),
                other => {
                    return Err(ScriptLangError::Message {
                        message: format!(
                            "ast_concat() arguments must be ast, got {}",
                            other.type_name()
                        ),
                    });
                }
            }
        }
    }

    let mut result = Vec::new();
    for item in all_items {
        match item {
            CtValue::Ast(items) => {
                result.extend(items.clone());
            }
            other => {
                return Err(ScriptLangError::Message {
                    message: format!(
                        "ast_concat() elements must be ast, got {}",
                        other.type_name()
                    ),
                });
            }
        }
    }

    Ok(CtValue::Ast(result))
}

/// `ast_filter_head(ast, predicate_head)`: Filter the AST to only include forms
/// whose head matches the predicate. Text nodes are excluded.
/// Returns a new ast (immutable).
pub(crate) fn builtin_ast_filter_head(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 2 {
        return Err(ScriptLangError::Message {
            message: "ast_filter_head() requires exactly 2 arguments: ast, predicate_head"
                .to_string(),
        });
    }

    let ast = match &args[0] {
        CtValue::Ast(items) => items,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "ast_filter_head() first argument must be ast, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let predicate = match &args[1] {
        CtValue::String(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "ast_filter_head() second argument (predicate_head) must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let filtered: Vec<FormItem> = ast
        .iter()
        .filter_map(|item| {
            if let FormItem::Form(form) = item
                && form.head == predicate
            {
                Some(item.clone())
            } else {
                None
            }
        })
        .collect();

    Ok(CtValue::Ast(filtered))
}
