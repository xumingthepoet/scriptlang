//! Attribute-related builtin functions.

use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::expand::macro_values::MacroValue;
use crate::semantic::macro_lang::builtins::{BuiltinRegistry, BuiltinResult};
use crate::semantic::macro_lang::{CtEnv, CtValue};
use sl_core::ScriptLangError;

/// `attr(name)`: Get macro invocation attribute value.
/// Falls back to checking keyword parameters in macro_env.locals when the
/// attribute is not found in macro_env.attributes (handles keyword:opts protocol).
pub(crate) fn builtin_attr(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "attr() requires exactly 1 argument".to_string(),
        });
    }

    let attr_name = match &args[0] {
        CtValue::String(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!("attr() argument must be string, got {}", other.type_name()),
            });
        }
    };

    // First check invocation attributes (legacy protocol and explicit attribute params)
    if let Some(s) = macro_env.get_attribute(&attr_name) {
        return Ok(CtValue::String(s.clone()));
    }

    // Fall back: check keyword parameters in macro_env.locals
    // (for keyword:opts protocol, invocation attrs are stored as keyword in locals)
    for value in macro_env.locals.values() {
        if let MacroValue::Keyword(kv) = value {
            for (k, v) in kv {
                if k == &attr_name {
                    let s = match v {
                        MacroValue::String(s) => s.clone(),
                        MacroValue::Expr(s) => s.clone(),
                        MacroValue::Nil => String::new(),
                        _ => format!("{:?}", v),
                    };
                    return Ok(CtValue::String(s));
                }
            }
        }
    }

    Err(ScriptLangError::Message {
        message: format!("Attribute '{}' not found", attr_name),
    })
}

/// `content()` or `content(head="...")`: Get macro invocation content.
pub(crate) fn builtin_content(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    let head_filter = if args.is_empty() {
        None
    } else if args.len() == 1 {
        match &args[0] {
            CtValue::Keyword(kv) => {
                let mut head = None;
                for (key, value) in kv {
                    if key == "head" {
                        head = Some(match value {
                            CtValue::String(s) => s.clone(),
                            other => {
                                return Err(ScriptLangError::Message {
                                    message: format!(
                                        "content(head=...) argument must be string, got {}",
                                        other.type_name()
                                    ),
                                });
                            }
                        });
                        break;
                    }
                }
                head
            }
            other => {
                return Err(ScriptLangError::Message {
                    message: format!(
                        "content() argument must be keyword list, got {}",
                        other.type_name()
                    ),
                });
            }
        }
    } else {
        return Err(ScriptLangError::Message {
            message: "content() takes at most 1 argument".to_string(),
        });
    };

    let children = if let Some(head) = head_filter {
        // Return the CHILDREN of matched slot forms (same behavior as select_invocation_content)
        let mut selected = Vec::new();
        for item in &macro_env.content {
            let sl_core::FormItem::Form(form) = item else {
                continue;
            };
            if form.head != *head {
                continue;
            }
            // Extract children from the matched form's "children" field
            if let Some(fields) = form.fields.iter().find_map(|field| {
                if field.name == "children" {
                    if let sl_core::FormValue::Sequence(items) = &field.value {
                        Some(items.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }) {
                selected.extend(fields);
            }
        }
        selected
    } else {
        macro_env.content.clone()
    };

    Ok(CtValue::Ast(children))
}

/// `has_attr(name)`: Check if macro invocation has an attribute.
/// Also checks keyword parameters in macro_env.locals (for keyword:opts protocol).
pub(crate) fn builtin_has_attr(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "has_attr() requires exactly 1 argument".to_string(),
        });
    }

    let attr_name = match &args[0] {
        CtValue::String(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "has_attr() argument must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    // Check invocation attributes first
    if macro_env.has_attribute(&attr_name) {
        return Ok(CtValue::Bool(true));
    }

    // Also check keyword parameters in macro_env.locals
    for value in macro_env.locals.values() {
        if let MacroValue::Keyword(kv) = value {
            for (k, _v) in kv {
                if k == &attr_name {
                    return Ok(CtValue::Bool(true));
                }
            }
        }
    }

    Ok(CtValue::Bool(false))
}
