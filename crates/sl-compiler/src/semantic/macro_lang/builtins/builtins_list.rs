//! List and iteration builtin functions.

use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::macro_lang::builtins::{BuiltinRegistry, BuiltinResult};
use crate::semantic::macro_lang::eval::{quote_items_callback, sync_ct_env_to_macro_env};
use crate::semantic::macro_lang::{CtEnv, CtValue};
use sl_core::ScriptLangError;
use sl_core::{Form, FormItem, FormValue};

/// `list(...items)`: Packs all arguments into a CtValue::List.
pub(crate) fn builtin_list(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    Ok(CtValue::List(args.to_vec()))
}

/// `list_concat(...lists)`: Concatenates all list arguments into a single flat list.
/// Nil arguments are treated as empty lists (for seamless use with module_get on first call).
pub(crate) fn builtin_list_concat(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    let mut result = Vec::new();
    for arg in args {
        match arg {
            CtValue::List(items) => result.extend(items.clone()),
            CtValue::Nil => { /* treat as empty, skip */ }
            other => {
                return Err(ScriptLangError::Message {
                    message: format!(
                        "list_concat() all arguments must be list or nil, got {}",
                        other.type_name()
                    ),
                });
            }
        }
    }
    Ok(CtValue::List(result))
}

const LOOP_ITEM_NAME: &str = "_item";

/// Evaluate a callback AST (a list of FormItems) with the current list item
/// bound to `_item` in ct_env.
///
/// The callback is written as `<quote><unquote>_item</unquote></quote>`.
/// After processing through `quote_items_callback`, the result for such a callback
/// is a single text item (the unquoted value). We unwrap this to return the
/// actual scalar `CtValue::String` so that `list_map`/`list_fold` get proper values.
fn evaluate_callback(
    callback_ast: &[FormItem],
    macro_env: &mut MacroEnv,
    ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    _builtins: &BuiltinRegistry,
) -> Result<CtValue, ScriptLangError> {
    // Clone macro_env once (quote_items needs &mut for gensym)
    let mut runtime = macro_env.clone();
    // Sync ct_env (including _item binding) so eval_unquote can find it
    sync_ct_env_to_macro_env(ct_env, &mut runtime);
    // Process the callback form items through quote_items (handles hygiene + unquote)
    let processed = quote_items_callback(expand_env, &mut runtime, callback_ast)?;

    // Unwrap scalar results from quote:
    // For `[<unquote>_item</unquote>]` where _item = "a",
    // processed = [FormItem::Text("a")].
    // We want the original CtValue (String/Int/Bool), not CtValue::Ast([...]).
    if processed.len() == 1 {
        // Case 1: Single text item → try to parse as Int/Bool/String
        if let FormItem::Text(ref text) = processed[0] {
            // Try to preserve original type by parsing back from string
            // This is needed because quote/unquote converts Int/Bool to string
            if let Ok(n) = text.parse::<i64>() {
                return Ok(CtValue::Int(n));
            }
            if text == "true" {
                return Ok(CtValue::Bool(true));
            }
            if text == "false" {
                return Ok(CtValue::Bool(false));
            }
            return Ok(CtValue::String(text.clone()));
        }
        // Case 2: Single $quote form with one text child → unwrap
        if let FormItem::Form(ref form) = processed[0]
            && form.head == "$quote"
            && let Some(text) = extract_single_text_child(form)
        {
            // Same parsing logic as above
            if let Ok(n) = text.parse::<i64>() {
                return Ok(CtValue::Int(n));
            }
            if text == "true" {
                return Ok(CtValue::Bool(true));
            }
            if text == "false" {
                return Ok(CtValue::Bool(false));
            }
            return Ok(CtValue::String(text));
        }
    }

    Ok(CtValue::Ast(processed))
}

/// Extract the single FormItem::Text child from a form's "children" field.
fn extract_single_text_child(form: &Form) -> Option<String> {
    for field in &form.fields {
        if field.name == "children"
            && let FormValue::Sequence(items) = &field.value
            && items.len() == 1
            && let FormItem::Text(text) = &items[0]
        {
            return Some(text.clone());
        }
    }
    None
}

/// `list_foreach(list, callback_ast)`: Iterates over list and executes callback for each element.
pub(crate) fn builtin_list_foreach(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    builtins: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 2 {
        return Err(ScriptLangError::Message {
            message: "list_foreach() requires exactly 2 arguments: list and callback".to_string(),
        });
    }

    let list = match &args[0] {
        CtValue::List(items) => items,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "list_foreach() first argument must be list, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let callback_items = match &args[1] {
        CtValue::LazyQuote(items) => items.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "list_foreach() second argument (callback) must be a quote (lazy), got {}",
                    other.type_name()
                ),
            });
        }
    };

    for item in list {
        // Bind current item to _item for callback evaluation
        ct_env.set(LOOP_ITEM_NAME.to_string(), item.clone());
        // Evaluate callback (result discarded for list_foreach)
        let _ = evaluate_callback(&callback_items, macro_env, ct_env, expand_env, builtins)?;
    }

    Ok(CtValue::Nil)
}

/// `list_map(list, callback_ast)`: Iterates over list, applies callback to each element,
/// returns list of results.
pub(crate) fn builtin_list_map(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    builtins: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 2 {
        return Err(ScriptLangError::Message {
            message: "list_map() requires exactly 2 arguments: list and callback".to_string(),
        });
    }

    let list = match &args[0] {
        CtValue::List(items) => items,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "list_map() first argument must be list, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let callback_items = match &args[1] {
        CtValue::LazyQuote(items) => items.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "list_map() second argument (callback) must be a quote (lazy), got {}",
                    other.type_name()
                ),
            });
        }
    };

    let mut results = Vec::with_capacity(list.len());
    for item in list {
        // Bind current item to _item for callback evaluation
        ct_env.set(LOOP_ITEM_NAME.to_string(), item.clone());
        // Evaluate callback and collect result
        let result = evaluate_callback(&callback_items, macro_env, ct_env, expand_env, builtins)?;
        results.push(result);
    }

    Ok(CtValue::List(results))
}

/// `list_fold(list, init, callback_ast)`: Accumulates a value over the list by applying
/// callback (starting with init) to each element.
pub(crate) fn builtin_list_fold(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    builtins: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 3 {
        return Err(ScriptLangError::Message {
            message: "list_fold() requires exactly 3 arguments: list, init, and callback"
                .to_string(),
        });
    }

    let list = match &args[0] {
        CtValue::List(items) => items,
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "list_fold() first argument must be list, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let init = args[1].clone();

    let callback_items = match &args[2] {
        CtValue::LazyQuote(items) => items.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "list_fold() third argument (callback) must be a quote (lazy), got {}",
                    other.type_name()
                ),
            });
        }
    };

    let mut acc = init;
    for item in list {
        // Bind current item to _item for callback evaluation
        ct_env.set(LOOP_ITEM_NAME.to_string(), item.clone());
        // Evaluate callback and update accumulator
        let result = evaluate_callback(&callback_items, macro_env, ct_env, expand_env, builtins)?;
        acc = result;
    }

    Ok(acc)
}

const WILDCARD_PATTERN: &str = "_";

/// `match(value, pattern1, result1, pattern2, result2, ...)`: Pattern matching
/// for compile-time values.
///
/// Patterns:
/// - `CtValue::Bool/Int/String/Keyword/List`: exact match
/// - `CtValue::String("_")`: wildcard (matches any value)
///
/// Returns the result of the first matching pattern.
/// Returns error if no pattern matches.
pub(crate) fn builtin_match(
    args: &[CtValue],
    _macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _builtins: &BuiltinRegistry,
) -> BuiltinResult {
    // Requires: value + (pattern, result) pairs = 1 + 2*N arguments (odd number)
    if args.len() < 3 {
        return Err(ScriptLangError::Message {
            message: "match() requires at least 3 arguments: value, pattern, result".to_string(),
        });
    }
    if args.len().is_multiple_of(2) {
        return Err(ScriptLangError::Message {
            message: format!(
                "match() requires odd number of arguments (value + pattern/result pairs), got {}",
                args.len()
            ),
        });
    }

    let value = &args[0];

    // Iterate over pattern-result pairs
    let mut i = 1;
    while i + 1 < args.len() {
        let pattern = &args[i];
        let result = &args[i + 1];

        // Check if pattern matches
        let matches = if let CtValue::String(s) = pattern {
            if s == WILDCARD_PATTERN {
                // Wildcard matches any value
                true
            } else {
                // String literal match
                value == pattern
            }
        } else {
            // Exact match for other types
            value == pattern
        };

        if matches {
            return Ok(result.clone());
        }

        i += 2;
    }

    // No pattern matched
    Err(ScriptLangError::Message {
        message: format!(
            "match(): no pattern matched value {:?}. Consider adding a wildcard '_' pattern as fallback.",
            value
        ),
    })
}
