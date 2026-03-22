//! Compile-time builtin functions.

use super::{CtValue, CtEnv};
use crate::semantic::expand::macro_env::MacroEnv;
use sl_core::ScriptLangError;

/// Result of a builtin function call.
pub type BuiltinResult = Result<CtValue, ScriptLangError>;

/// A compile-time builtin function.
pub type BuiltinFn = fn(&[CtValue], &MacroEnv, &mut CtEnv) -> BuiltinResult;

/// Registry of compile-time builtin functions.
pub struct BuiltinRegistry {
    builtins: std::collections::HashMap<String, BuiltinFn>,
}

impl BuiltinRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            builtins: std::collections::HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    /// Register default builtins.
    fn register_defaults(&mut self) {
        // Old template providers adapted as builtins
        self.register("attr", builtin_attr);
        self.register("content", builtin_content);
        self.register("has_attr", builtin_has_attr);

        // New compile-time utilities
        self.register("keyword_get", builtin_keyword_get);
        self.register("keyword_has", builtin_keyword_has);
        self.register("list_length", builtin_list_length);
        self.register("to_string", builtin_to_string);
    }

    /// Register a builtin function.
    pub fn register(&mut self, name: &str, func: BuiltinFn) {
        self.builtins.insert(name.to_string(), func);
    }

    /// Get a builtin function by name.
    pub fn get(&self, name: &str) -> Option<BuiltinFn> {
        self.builtins.get(name).copied()
    }
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Builtin implementations
// ============================================================================

/// `attr(name)`: Get macro invocation attribute value.
fn builtin_attr(args: &[CtValue], macro_env: &MacroEnv, _ct_env: &mut CtEnv) -> BuiltinResult {
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

    macro_env
        .get_attribute(&attr_name)
        .map(|s| CtValue::String(s.clone()))
        .ok_or_else(|| {
            ScriptLangError::Message {
                message: format!("Attribute '{}' not found", attr_name),
            }
        })
}

/// `content()` or `content(head="...")`: Get macro invocation content.
fn builtin_content(args: &[CtValue], macro_env: &MacroEnv, _ct_env: &mut CtEnv) -> BuiltinResult {
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
        macro_env.get_content_with_head(&head)
    } else {
        macro_env.get_content()
    };

    Ok(CtValue::Ast(children))
}

/// `has_attr(name)`: Check if macro invocation has an attribute.
fn builtin_has_attr(args: &[CtValue], macro_env: &MacroEnv, _ct_env: &mut CtEnv) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "has_attr() requires exactly 1 argument".to_string(),
        });
    }

    let attr_name = match &args[0] {
        CtValue::String(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!("has_attr() argument must be string, got {}", other.type_name()),
            });
        }
    };

    Ok(CtValue::Bool(macro_env.has_attribute(&attr_name)))
}

/// `keyword_get(keyword, key)`: Get a value from a keyword list.
fn builtin_keyword_get(args: &[CtValue], _macro_env: &MacroEnv, _ct_env: &mut CtEnv) -> BuiltinResult {
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
        .ok_or_else(|| {
            ScriptLangError::Message {
                message: format!("Key '{}' not found in keyword list", key),
            }
        })
}

/// `keyword_has(keyword, key)`: Check if a keyword list has a key.
fn builtin_keyword_has(args: &[CtValue], _macro_env: &MacroEnv, _ct_env: &mut CtEnv) -> BuiltinResult {
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

/// `list_length(list)`: Get the length of a list.
fn builtin_list_length(args: &[CtValue], _macro_env: &MacroEnv, _ct_env: &mut CtEnv) -> BuiltinResult {
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
fn builtin_to_string(args: &[CtValue], _macro_env: &MacroEnv, _ct_env: &mut CtEnv) -> BuiltinResult {
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
