//! Keyword operation builtin functions.

use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::expand::macro_values::MacroValue;
use crate::semantic::macro_lang::builtins::{BuiltinRegistry, BuiltinResult};
use crate::semantic::macro_lang::eval::macro_value_to_ct_value;
use crate::semantic::macro_lang::{CtEnv, CtValue};
use sl_core::ScriptLangError;

/// `keyword_get(keyword, key)`: Get a value from a keyword list.
pub(crate) fn builtin_keyword_get(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 2 {
        return Err(ScriptLangError::Message {
            message: "keyword_get() requires exactly 2 arguments".to_string(),
        });
    }

    let keyword = match &args[0] {
        CtValue::Keyword(kv) => kv,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "keyword_get() first argument must be keyword, got {}",
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
                    "keyword_get() second argument must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    keyword
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .ok_or_else(|| ScriptLangError::Message {
            message: format!("Key '{}' not found in keyword list", key),
        })
}

/// `keyword_attr(name)`: Get a keyword from macro_env.locals and return it as CtValue::Keyword.
/// This is used by the macro body when a param is declared with type "keyword".
pub(crate) fn builtin_keyword_attr(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "keyword_attr() requires exactly 1 argument".to_string(),
        });
    }

    let name = match &args[0] {
        CtValue::String(s) => s,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "keyword_attr() argument must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    // Look up the keyword from macro_env.locals
    // First try direct lookup: if "items" is a top-level keyword param, return it directly.
    // Then try nested lookup: if "items" is inside a keyword-type param (e.g. "opts"),
    // extract just the "items" key-value pair and return it as a standalone keyword.
    let ct_value = if let Some(value) = macro_env.locals.get(name) {
        match value {
            MacroValue::Keyword(_) => {
                // Direct hit: return the full keyword
                convert_macro_value_to_ct_value(value)
            }
            _ => {
                return Err(ScriptLangError::Message {
                    message: format!(
                        "keyword_attr('{}') must reference a keyword, got {:?}",
                        name, value
                    ),
                });
            }
        }
    } else {
        // Nested lookup: search through all MacroValue::Keyword entries for the key
        // Return the VALUE directly (not wrapped in a keyword), so that:
        // - If items="foo,bar,baz" is passed, keyword_attr("items") returns the List directly
        // - If enabled="true" is passed, keyword_attr("enabled") returns Bool(true) directly
        let mut found: Option<CtValue> = None;
        for value in macro_env.locals.values() {
            if let MacroValue::Keyword(kv_pairs) = value {
                for (k, v) in kv_pairs {
                    if k == name {
                        found = Some(macro_value_to_ct_value(v));
                        break;
                    }
                }
            }
            if found.is_some() {
                break;
            }
        }
        found.ok_or_else(|| ScriptLangError::Message {
            message: format!("keyword '{}' not found in macro locals", name),
        })?
    };

    // Return the CtValue (already converted)
    Ok(ct_value)
}

// Helper: convert a full MacroValue::Keyword to CtValue::Keyword
fn convert_macro_value_to_ct_value(value: &MacroValue) -> CtValue {
    match value {
        MacroValue::Keyword(items) => {
            let ct_items: Vec<(String, CtValue)> = items
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        match v {
                            MacroValue::Nil => CtValue::Nil,
                            MacroValue::Bool(b) => CtValue::Bool(*b),
                            MacroValue::Int(i) => CtValue::Int(*i),
                            MacroValue::String(s) => CtValue::String(s.clone()),
                            MacroValue::Expr(s) => CtValue::String(s.clone()),
                            MacroValue::AstItems(items) => CtValue::Ast(items.clone()),
                            MacroValue::List(items) => {
                                CtValue::List(items.iter().map(macro_value_to_ct_value).collect())
                            }
                            MacroValue::Keyword(nested) => CtValue::Keyword(
                                nested
                                    .iter()
                                    .map(|(nk, nv)| (nk.clone(), macro_value_to_ct_value(nv)))
                                    .collect(),
                            ),
                        },
                    )
                })
                .collect();
            CtValue::Keyword(ct_items)
        }
        other => CtValue::String(format!("{:?}", other)),
    }
}

/// `keyword_has(keyword, key)`: Check if a keyword list has a key.
pub(crate) fn builtin_keyword_has(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 2 {
        return Err(ScriptLangError::Message {
            message: "keyword_has() requires exactly 2 arguments".to_string(),
        });
    }

    let keyword = match &args[0] {
        CtValue::Keyword(kv) => kv,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "keyword_has() first argument must be keyword, got {}",
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
                    "keyword_has() second argument must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    Ok(CtValue::Bool(keyword.iter().any(|(k, _)| k == key)))
}

/// `keyword_keys(keyword)`: Get all keys from a keyword list.
pub(crate) fn builtin_keyword_keys(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "keyword_keys() requires exactly 1 argument".to_string(),
        });
    }

    let keyword = match &args[0] {
        CtValue::Keyword(kv) => kv,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "keyword_keys() argument must be keyword, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let keys: Vec<CtValue> = keyword
        .iter()
        .map(|(k, _)| CtValue::String(k.clone()))
        .collect();

    Ok(CtValue::List(keys))
}

/// `keyword_values(keyword)`: Get all values from a keyword list as a list.
pub(crate) fn builtin_keyword_values(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "keyword_values() requires exactly 1 argument".to_string(),
        });
    }

    let keyword = match &args[0] {
        CtValue::Keyword(kv) => kv,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "keyword_values() argument must be keyword, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let values: Vec<CtValue> = keyword.iter().map(|(_, v)| v.clone()).collect();

    Ok(CtValue::List(values))
}

/// `keyword_pairs(keyword)`: Get all key-value pairs from a keyword list.
/// Returns a list of [key, value] pairs.
pub(crate) fn builtin_keyword_pairs(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "keyword_pairs() requires exactly 1 argument".to_string(),
        });
    }

    let keyword = match &args[0] {
        CtValue::Keyword(kv) => kv,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "keyword_pairs() argument must be keyword, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let pairs: Vec<CtValue> = keyword
        .iter()
        .map(|(k, v)| CtValue::List(vec![CtValue::String(k.clone()), v.clone()]))
        .collect();

    Ok(CtValue::List(pairs))
}
