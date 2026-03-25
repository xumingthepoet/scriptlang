//! Scalar conversion builtin functions.

use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::macro_lang::builtins::{BuiltinRegistry, BuiltinResult};
use crate::semantic::macro_lang::{CtEnv, CtValue};
use sl_core::ScriptLangError;

/// `list_length(list)`: Get the length of a list.
pub(crate) fn builtin_list_length(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "list_length() requires exactly 1 argument".to_string(),
        });
    }

    match &args[0] {
        CtValue::List(items) => Ok(CtValue::Int(items.len() as i64)),
        CtValue::Keyword(kv) => Ok(CtValue::Int(kv.len() as i64)),
        other => Err(ScriptLangError::Message {
            message: format!(
                "list_length() argument must be list or keyword, got {}",
                other.type_name()
            ),
        }),
    }
}

/// `to_string(value)`: Convert a value to string.
pub(crate) fn builtin_to_string(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "to_string() requires exactly 1 argument".to_string(),
        });
    }

    let s = match &args[0] {
        CtValue::Nil => "nil".to_string(),
        CtValue::Bool(b) => b.to_string(),
        CtValue::Int(i) => i.to_string(),
        CtValue::String(s) => s.clone(),
        other => format!("{:?}", other),
    };

    Ok(CtValue::String(s))
}

/// `parse_bool(value)`: Parse a string value as bool.
pub(crate) fn builtin_parse_bool(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "parse_bool() requires exactly 1 argument".to_string(),
        });
    }

    let s = match &args[0] {
        CtValue::String(s) => s,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "parse_bool() argument must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    match s.as_str() {
        "true" => Ok(CtValue::Bool(true)),
        "false" => Ok(CtValue::Bool(false)),
        other => Err(ScriptLangError::Message {
            message: format!("cannot parse `{}` as macro bool attribute", other),
        }),
    }
}

/// `parse_int(value)`: Parse a string value as int.
pub(crate) fn builtin_parse_int(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "parse_int() requires exactly 1 argument".to_string(),
        });
    }

    let s = match &args[0] {
        CtValue::String(s) => s,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "parse_int() argument must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    s.parse::<i64>()
        .map(CtValue::Int)
        .map_err(|_| ScriptLangError::Message {
            message: format!("cannot parse `{}` as macro int attribute", s),
        })
}
