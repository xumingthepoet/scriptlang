//! Compile-time builtin functions.

use super::eval::macro_value_to_ct_value;
use super::{CtEnv, CtValue};
use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::dispatch::ExpandRuleScope;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::expand::macro_values::MacroValue;
use crate::semantic::expand::macros::expand_macro_invocation_public;
use crate::semantic::location;
use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, ScriptLangError, SourcePosition};

/// Convert a CtValue to a string for serialization in invoke_macro keyword args.
/// The format must be parseable by parse_macro_value_from_string in macro_params.rs.
fn ct_value_to_string(value: &CtValue) -> String {
    match value {
        CtValue::Nil => "nil".to_string(),
        CtValue::Bool(b) => b.to_string(),
        CtValue::Int(i) => i.to_string(),
        CtValue::String(s) => s.clone(),
        CtValue::ModuleRef(m) => format!("@{}", m),
        CtValue::CallerEnv => "<caller_env>".to_string(),
        // List: serialize as comma-separated items (parseable as comma-separated list)
        CtValue::List(items) => items
            .iter()
            .map(ct_value_to_string)
            .collect::<Vec<_>>()
            .join(","),
        // Keyword: serialize as "key:val,key2:val2" (parseable as colon-separated keyword)
        CtValue::Keyword(kv) => kv
            .iter()
            .map(|(k, v)| format!("{}:{}", k, ct_value_to_string(v)))
            .collect::<Vec<_>>()
            .join(","),
        // Ast: represent as opaque string (cannot be losslessly serialized as attribute string)
        CtValue::Ast(_) => "[ast]".to_string(),
    }
}

/// Result of a builtin function call.
pub type BuiltinResult = Result<CtValue, ScriptLangError>;

/// A compile-time builtin function.
/// Builtins receive:
/// - `&MacroEnv`: read-only access to caller context
/// - `&mut CtEnv`: mutable local variable bindings
/// - `&mut ExpandEnv`: mutable module state (for require/import/alias/invoke operations)
pub type BuiltinFn = fn(&[CtValue], &MacroEnv, &mut CtEnv, &mut ExpandEnv) -> BuiltinResult;

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

        // Parsing utilities
        self.register("parse_bool", builtin_parse_bool);
        self.register("parse_int", builtin_parse_int);

        // New compile-time utilities
        self.register("keyword_get", builtin_keyword_get);
        self.register("keyword_has", builtin_keyword_has);
        self.register("list_length", builtin_list_length);
        self.register("to_string", builtin_to_string);
        self.register("keyword_attr", builtin_keyword_attr);

        // Step 4: Remote macro and caller env builtins
        self.register("caller_env", builtin_caller_env);
        self.register("caller_module", builtin_caller_module);
        self.register("expand_alias", builtin_expand_alias);
        self.register("require_module", builtin_require_module);
        self.register("define_import", builtin_define_import);
        self.register("define_alias", builtin_define_alias);
        self.register("define_require", builtin_define_require);
        self.register("invoke_macro", builtin_invoke_macro);

        // Step 3.2: AST builtins (basic read)
        self.register("ast_head", builtin_ast_head);
        self.register("ast_children", builtin_ast_children);
        self.register("ast_attr_get", builtin_ast_attr_get);
        self.register("ast_attr_keys", builtin_ast_attr_keys);

        // Step 3.3: AST builtins (write)
        self.register("ast_attr_set", builtin_ast_attr_set);
        self.register("ast_wrap", builtin_ast_wrap);
        self.register("ast_concat", builtin_ast_concat);
        self.register("ast_filter_head", builtin_ast_filter_head);
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
/// Falls back to checking keyword parameters in macro_env.locals when the
/// attribute is not found in macro_env.attributes (handles keyword:opts protocol).
fn builtin_attr(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_content(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
            let FormItem::Form(form) = item else {
                continue;
            };
            if form.head != *head {
                continue;
            }
            // Extract children from the matched form's "children" field
            if let Some(fields) = form.fields.iter().find_map(|field| {
                if field.name == "children" {
                    if let FormValue::Sequence(items) = &field.value {
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
fn builtin_has_attr(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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

/// `keyword_get(keyword, key)`: Get a value from a keyword list.
fn builtin_keyword_get(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_keyword_attr(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_keyword_has(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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

/// `list_length(list)`: Get the length of a list.
fn builtin_list_length(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_to_string(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_parse_bool(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_parse_int(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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

// ============================================================================
// Step 4: Remote macro and caller env builtins
// ============================================================================

/// `caller_env()`: Return a CtValue exposing the caller environment.
fn builtin_caller_env(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
) -> BuiltinResult {
    if !args.is_empty() {
        return Err(ScriptLangError::Message {
            message: "caller_env() takes no arguments".to_string(),
        });
    }

    // Build a keyword exposing caller context
    let mut items = Vec::new();

    // current_module
    if let Some(ref m) = macro_env.current_module {
        items.push(("current_module".to_string(), CtValue::String(m.clone())));
    }

    // macro_name (the name of the macro being expanded)
    if !macro_env.macro_name.is_empty() {
        items.push((
            "macro_name".to_string(),
            CtValue::String(macro_env.macro_name.clone()),
        ));
    }

    // source_file (file where the macro was invoked)
    if let Some(ref f) = macro_env.source_file {
        items.push(("file".to_string(), CtValue::String(f.clone())));
    }

    // line (1-based row where the macro was invoked)
    if let Some(l) = macro_env.line {
        items.push(("line".to_string(), CtValue::Int(l as i64)));
    }

    // column (1-based column where the macro was invoked)
    if let Some(c) = macro_env.column {
        items.push(("column".to_string(), CtValue::Int(c as i64)));
    }

    // We expose imports, requires, aliases from macro_env
    items.push((
        "imports".to_string(),
        CtValue::List(
            macro_env
                .imports
                .iter()
                .map(|s| CtValue::String(s.clone()))
                .collect(),
        ),
    ));

    items.push((
        "requires".to_string(),
        CtValue::List(
            macro_env
                .requires
                .iter()
                .map(|s| CtValue::String(s.clone()))
                .collect(),
        ),
    ));

    items.push((
        "aliases".to_string(),
        CtValue::List(
            macro_env
                .aliases
                .iter()
                .map(|(k, v)| CtValue::String(format!("{}={}", k, v)))
                .collect(),
        ),
    ));

    Ok(CtValue::Keyword(items))
}

/// `caller_module()`: Return the current module name as a string.
fn builtin_caller_module(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
) -> BuiltinResult {
    if !args.is_empty() {
        return Err(ScriptLangError::Message {
            message: "caller_module() takes no arguments".to_string(),
        });
    }

    Ok(CtValue::String(
        macro_env
            .current_module
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string()),
    ))
}

/// `expand_alias(module_ref)`: Expand a module alias or name to full module name.
fn builtin_expand_alias(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "expand_alias() requires exactly 1 argument".to_string(),
        });
    }

    let module_ref = match &args[0] {
        CtValue::String(s) => s.clone(),
        CtValue::ModuleRef(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "expand_alias() argument must be string or module, got {}",
                    other.type_name()
                ),
            });
        }
    };

    // Check aliases first
    if let Some(full_name) = macro_env.aliases.get(&module_ref) {
        return Ok(CtValue::String(full_name.clone()));
    }

    // Otherwise return as-is (it's a full module name)
    Ok(CtValue::String(module_ref))
}

/// `require_module(module_ref)`: Ensure a module is required (add to requires list).
fn builtin_require_module(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "require_module() requires exactly 1 argument".to_string(),
        });
    }

    let module_ref = match &args[0] {
        CtValue::String(s) => s.clone(),
        CtValue::ModuleRef(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "require_module() argument must be string or module, got {}",
                    other.type_name()
                ),
            });
        }
    };

    // Expand alias if needed (e.g. "H" -> "helper")
    let full_name = macro_env
        .aliases
        .get(&module_ref)
        .cloned()
        .unwrap_or_else(|| module_ref.clone());

    // Also check expand_env.module.requires for the expanded name (in case already added)
    let already_required = macro_env.requires.contains(&module_ref)
        || macro_env.requires.contains(&full_name)
        || expand_env.module.requires.contains(&full_name);

    if !already_required {
        // Add to expand_env requires (this affects subsequent macro resolution)
        expand_env.add_require(full_name.clone());
    }

    // Return the expanded module name so callers (like invoke_macro) use the resolved name
    Ok(CtValue::String(full_name))
}

/// `define_import(module_ref)`: Add an import to the current module.
fn builtin_define_import(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "define_import() requires exactly 1 argument".to_string(),
        });
    }

    let module_ref = match &args[0] {
        CtValue::String(s) => s.clone(),
        CtValue::ModuleRef(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "define_import() argument must be string or module, got {}",
                    other.type_name()
                ),
            });
        }
    };

    // Expand alias if needed
    let full_name = macro_env
        .aliases
        .get(&module_ref)
        .cloned()
        .unwrap_or(module_ref);

    expand_env.add_import(full_name);
    Ok(CtValue::Nil)
}

/// `define_alias(module_ref, as)`: Add an alias for a module.
fn builtin_define_alias(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
) -> BuiltinResult {
    if args.len() != 2 {
        return Err(ScriptLangError::Message {
            message: "define_alias() requires exactly 2 arguments".to_string(),
        });
    }

    let module_ref = match &args[0] {
        CtValue::String(s) => s.clone(),
        CtValue::ModuleRef(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "define_alias() first argument must be string or module, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let alias_name = match &args[1] {
        CtValue::String(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "define_alias() second argument must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    // Expand the module ref
    let full_name = macro_env
        .aliases
        .get(&module_ref)
        .cloned()
        .unwrap_or(module_ref);

    expand_env
        .add_alias(alias_name, full_name)
        .map_err(|e| ScriptLangError::Message { message: e })?;

    Ok(CtValue::Nil)
}

/// `define_require(module_ref)`: Add a require to the current module.
fn builtin_define_require(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
) -> BuiltinResult {
    if args.len() != 1 {
        return Err(ScriptLangError::Message {
            message: "define_require() requires exactly 1 argument".to_string(),
        });
    }

    let module_ref = match &args[0] {
        CtValue::String(s) => s.clone(),
        CtValue::ModuleRef(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "define_require() argument must be string or module, got {}",
                    other.type_name()
                ),
            });
        }
    };

    // Expand alias if needed
    let full_name = macro_env
        .aliases
        .get(&module_ref)
        .cloned()
        .unwrap_or(module_ref);

    expand_env.add_require(full_name);
    Ok(CtValue::Nil)
}

/// `invoke_macro(module_ref, macro_name, args)`: Invoke a macro from another module.
fn builtin_invoke_macro(
    args: &[CtValue],
    macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
) -> BuiltinResult {
    if args.len() != 3 {
        return Err(ScriptLangError::Message {
            message: "invoke_macro() requires exactly 3 arguments: module, macro_name, args"
                .to_string(),
        });
    }

    let module_ref = match &args[0] {
        CtValue::String(s) => s.clone(),
        CtValue::ModuleRef(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "invoke_macro() first argument (module) must be string or module, got {}",
                    other.type_name()
                ),
            });
        }
    };

    let macro_name = match &args[1] {
        CtValue::String(s) => s.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "invoke_macro() second argument (macro_name) must be string, got {}",
                    other.type_name()
                ),
            });
        }
    };

    // Resolve module name (expand alias if needed)
    let resolved_module = macro_env
        .aliases
        .get(&module_ref)
        .cloned()
        .unwrap_or(module_ref.clone());

    // Track caller module for error enrichment (Step 6: improve error location)
    let caller_module = macro_env
        .current_module
        .clone()
        .unwrap_or_else(|| "<unknown>".to_string());

    // Step 4.3: Compute caller source location for enriched error messages.
    let caller_location = expand_env
        .caller_invocation_meta
        .as_ref()
        .map(location)
        .unwrap_or_default();

    // Check that the module is registered in the program.
    // Note: module registration happens when a module form is processed by the compiler.
    let module_exists = expand_env
        .program
        .module_macros
        .contains_key(&resolved_module);
    if !module_exists {
        let location_suffix = if caller_location.is_empty() {
            String::new()
        } else {
            format!(" at {}", caller_location)
        };
        return Err(ScriptLangError::Message {
            message: format!(
                "cannot invoke macro `{}.{}`: module `{}` is not known \
                (called from `{}`{}). Available modules: {:?}",
                resolved_module,
                macro_name,
                resolved_module,
                caller_module,
                location_suffix,
                expand_env.program.module_macros.keys().collect::<Vec<_>>()
            ),
        });
    }

    // Module exists but must be in scope (required) before we can invoke its macros.
    let is_current_module = macro_env
        .current_module
        .as_ref()
        .map(|m| m == &resolved_module)
        .unwrap_or(false);
    let is_required_in_macro = macro_env.requires.contains(&resolved_module);
    let is_required_in_expand = expand_env.module.requires.contains(&resolved_module);
    let is_required = is_current_module || is_required_in_macro || is_required_in_expand;

    if !is_required {
        let location_suffix = if caller_location.is_empty() {
            String::new()
        } else {
            format!(" at {}", caller_location)
        };
        return Err(ScriptLangError::Message {
            message: format!(
                "cannot invoke macro `{}.{}`: module `{}` is not in scope \
                (called from `{}`{}). Add `<require name=\"{}\"/>` or use `require_module(\"{}\")` first.",
                resolved_module,
                macro_name,
                resolved_module,
                caller_module,
                location_suffix,
                resolved_module,
                resolved_module
            ),
        });
    }

    // Resolve the macro STRICTLY from the target module (Step 1: module-qualified dispatch).
    // Uses resolve_macro_in instead of resolve_macro to avoid fallback to
    // current module / imports / kernel lookup order.
    let definition = expand_env
        .program
        .resolve_macro_in(&resolved_module, &macro_name)
        .cloned()
        .ok_or_else(|| {
            // Module exists but doesn't export this macro name
            let location_suffix = if caller_location.is_empty() {
                String::new()
            } else {
                format!(" at {}", caller_location)
            };
            ScriptLangError::Message {
                message: format!(
                    "macro `{}.{}` is not defined in module `{}` (called from `{}`{})",
                    resolved_module, macro_name, resolved_module, caller_module, location_suffix
                ),
            }
        })?;

    // Step 7: Check macro visibility (private macros only visible in defining module)
    if definition.is_private && definition.module_name != caller_module {
        let location_suffix = if caller_location.is_empty() {
            String::new()
        } else {
            format!(" at {}", caller_location)
        };
        return Err(ScriptLangError::Message {
            message: format!(
                "cannot invoke private macro `{}.{}` from module `{}`{}",
                definition.module_name, macro_name, caller_module, location_suffix
            ),
        });
    }

    // Build synthetic invocation form from args
    let args_kw = match &args[2] {
        CtValue::Keyword(kv) => kv.clone(),
        other => {
            return Err(ScriptLangError::Message {
                message: format!(
                    "invoke_macro() third argument (args) must be keyword, got {}",
                    other.type_name()
                ),
            });
        }
    };

    // Convert keyword args to form attributes
    let mut invocation_fields = Vec::new();
    for (key, value) in &args_kw {
        let form_value = match value {
            CtValue::String(s) => FormValue::String(s.clone()),
            CtValue::Int(i) => FormValue::String(i.to_string()),
            CtValue::Bool(b) => FormValue::String(b.to_string()),
            // Step 2.4: Support nested List/Keyword/Ast in invoke_macro args.
            // These are serialized as delimited strings so they can be passed
            // as XML form attributes and then parsed back in bind_explicit_params.
            CtValue::List(items) => {
                let serialized: String = items
                    .iter()
                    .map(ct_value_to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                FormValue::String(serialized)
            }
            CtValue::Keyword(kv) => {
                let serialized: String = kv
                    .iter()
                    .map(|(k, v)| format!("{}:{}", k, ct_value_to_string(v)))
                    .collect::<Vec<_>>()
                    .join(",");
                FormValue::String(serialized)
            }
            CtValue::Ast(items) => FormValue::Sequence(items.clone()),
            other => {
                return Err(ScriptLangError::Message {
                    message: format!(
                        "invoke_macro() keyword arg value must be string, int, bool, list, keyword, or ast, got {}",
                        other.type_name()
                    ),
                });
            }
        };
        invocation_fields.push(FormField {
            name: key.clone(),
            value: form_value,
        });
    }
    invocation_fields.push(FormField {
        name: "children".to_string(),
        value: FormValue::Sequence(macro_env.content.clone()),
    });

    // Build synthetic invocation form from args.
    // Use the caller invocation meta if available (set by expand_macro_hook),
    // so that caller_env() in the remote macro sees the correct source location.
    // Fall back to dummy meta for nested invoke_macro calls (no caller context).
    let caller_meta = expand_env.caller_invocation_meta.clone();
    let synthetic_invocation_meta = caller_meta.unwrap_or_else(|| FormMeta {
        source_name: Some(format!("{} (via {})", resolved_module, caller_module)),
        start: SourcePosition { row: 0, column: 0 },
        end: SourcePosition { row: 0, column: 0 },
        start_byte: 0,
        end_byte: 0,
    });
    let synthetic_invocation = Form {
        head: macro_name.clone(),
        meta: synthetic_invocation_meta,
        fields: invocation_fields,
    };

    // Expand the macro, enriching errors with caller context.
    // When remote macro expansion fails, error messages will now include
    // both the provider (where the macro is defined) and the caller
    // (where the use/invoke_macro call was made).
    let expanded_items = expand_macro_invocation_public(
        definition,
        &synthetic_invocation,
        expand_env,
        ExpandRuleScope::Statement,
    )
    .map_err(|e| ScriptLangError::Message {
        message: format!(
            "error expanding `{}` from `{}` (called from `{}`): {}",
            macro_name, resolved_module, caller_module, e
        ),
    })?;

    Ok(CtValue::Ast(expanded_items))
}

// ============================================================================
// Step 3.2: AST builtins (basic read)
// ============================================================================

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
fn builtin_ast_head(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_ast_children(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_ast_attr_get(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_ast_attr_keys(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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

// ============================================================================
// Step 3.3: AST builtins (write)
// ============================================================================

/// `ast_attr_set(ast, key, value)`: Return a new ast with the attribute set on the first form.
/// Does NOT modify the original ast (immutable).
fn builtin_ast_attr_set(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_ast_wrap(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_ast_concat(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
fn builtin_ast_filter_head(
    args: &[CtValue],
    _macro_env: &MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
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
