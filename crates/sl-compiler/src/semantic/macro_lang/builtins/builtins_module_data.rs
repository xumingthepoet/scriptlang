//! Module-level compile-time state builtin functions.

use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::macro_lang::builtins::{BuiltinRegistry, BuiltinResult};
use crate::semantic::macro_lang::{CtEnv, CtValue};
use sl_core::ScriptLangError;

/// `module_get(name)`: Read a value from the current module's compile-time state.
/// Returns Nil if the key does not exist.
pub(crate) fn builtin_module_get(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "module_get() requires exactly 1 argument (name)".to_string(),
        });
    }

    let name = match &args[0] {
        CtValue::String(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "module_get() first argument (name) must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let state = expand_env.get_module_state();
    Ok(state.get(&name).cloned().unwrap_or(CtValue::Nil))
}

/// `module_put(name, value)`: Write a value to the current module's compile-time state.
/// Returns the written value.
pub(crate) fn builtin_module_put(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 2 {
        return Err(ScriptLangError::Message {
            message: "module_put() requires exactly 2 arguments (name, value)".to_string(),
        });
    }

    let name = match &args[0] {
        CtValue::String(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "module_put() first argument (name) must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let value = args[1].clone();
    let state = expand_env.get_module_state_mut();
    if state.contains_key(&name) {
        return Err(ScriptLangError::Message {
            message: format!(
                "module_put() conflict: key `{}` already exists in module state. \
                 Use module_update() to overwrite, or choose a different key name.",
                name
            ),
        });
    }
    state.insert(name, value.clone());
    Ok(value)
}

/// `module_update(name, new_value)`: Reads the current value for the key (returns Nil if absent),
/// then writes new_value. Enables read-modify-write accumulation patterns.
/// Returns new_value.
pub(crate) fn builtin_module_update(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 2 {
        return Err(ScriptLangError::Message {
            message: "module_update() requires exactly 2 arguments (name, new_value)".to_string(),
        });
    }

    let name = match &args[0] {
        CtValue::String(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "module_update() first argument (name) must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let new_value = args[1].clone();
    expand_env
        .get_module_state_mut()
        .insert(name, new_value.clone());
    Ok(new_value)
}
