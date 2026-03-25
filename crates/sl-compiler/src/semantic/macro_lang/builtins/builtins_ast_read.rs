//! AST read builtin functions.

use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::macro_lang::builtins::{BuiltinRegistry, BuiltinResult};
use crate::semantic::macro_lang::{CtEnv, CtValue};
use sl_core::ScriptLangError;
use sl_core::{Form, FormItem, FormValue};

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

/// Convert a FormValue to CtValue.
fn form_value_to_ct_value(value: &FormValue) -> CtValue {
    match value {
        FormValue::String(s) => CtValue::String(s.clone()),
        FormValue::Sequence(items) => CtValue::Ast(items.clone()),
    }
}

/// `ast_head(ast)`: Return the head string of the first form in the AST.
/// Returns an error if the AST is empty or contains only text nodes.
pub(crate) fn builtin_ast_head(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "ast_head() requires exactly 1 argument".to_string(),
        });
    }

    let ast = match &args[0] {
        CtValue::Ast(items) => items,
        other => {
            return Err(ScriptLangError::Message {
                message: format!("ast_head() argument must be ast, got {}", other.type_name()),
            });
        }
    };

    let form = extract_first_form(ast)?;
    Ok(CtValue::String(form.head.clone()))
}

/// `ast_children(ast)`: Return the children of the first form in the AST as a new ast.
/// Returns an empty ast if the AST is empty or contains only text nodes.
pub(crate) fn builtin_ast_children(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "ast_children() requires exactly 1 argument".to_string(),
        });
    }

    let ast = match &args[0] {
        CtValue::Ast(items) => items,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "ast_children() argument must be ast, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let form = extract_first_form(ast)?;
    // Extract the "children" field value; default to empty sequence if not present.
    let children = form
        .fields
        .iter()
        .find(|f| f.name == "children")
        .map(|f| match &f.value {
            FormValue::Sequence(items) => items.clone(),
            FormValue::String(s) => vec![FormItem::Text(s.clone())],
        })
        .unwrap_or_default();

    Ok(CtValue::Ast(children))
}

/// `ast_attr_get(ast, key)`: Get the value of an attribute on the first form in the AST.
/// Returns the attribute value as CtValue (string or ast), or an error if not found.
pub(crate) fn builtin_ast_attr_get(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 2 {
        return Err(ScriptLangError::Message {
            message: "ast_attr_get() requires exactly 2 arguments: ast, key".to_string(),
        });
    }

    let ast = match &args[0] {
        CtValue::Ast(items) => items,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "ast_attr_get() first argument must be ast, got {}",
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
                    "ast_attr_get() second argument (key) must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let form = extract_first_form(ast)?;
    let value = form
        .fields
        .iter()
        .find(|f| f.name == *key)
        .map(|f| form_value_to_ct_value(&f.value))
        .ok_or_else(|| ScriptLangError::Message {
            message: format!("attribute '{}' not found on form '{}'", key, form.head),
        })?;

    Ok(value)
}

/// `ast_attr_keys(ast)`: Return a list of all attribute keys on the first form in the AST.
pub(crate) fn builtin_ast_attr_keys(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "ast_attr_keys() requires exactly 1 argument".to_string(),
        });
    }

    let ast = match &args[0] {
        CtValue::Ast(items) => items,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "ast_attr_keys() argument must be ast, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let form = extract_first_form(ast)?;
    let keys: Vec<CtValue> = form
        .fields
        .iter()
        .filter(|f| f.name != "children") // Exclude internal "children" field
        .map(|f| CtValue::String(f.name.clone()))
        .collect();

    Ok(CtValue::List(keys))
}
