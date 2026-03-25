//! Remote macro and caller environment builtin functions.

use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::dispatch::ExpandRuleScope;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::expand::macros::expand_macro_invocation_public;
use crate::semantic::location;
use crate::semantic::macro_lang::builtins::{BuiltinRegistry, BuiltinResult};
use crate::semantic::macro_lang::{CtEnv, CtValue};
use sl_core::ScriptLangError;
use sl_core::{Form, FormField, FormMeta, FormValue, SourcePosition};

/// Extract a module reference from the first argument (CtValue::String or CtValue::ModuleRef).
/// Returns Err if args is empty or first arg is not String/ModuleRef.
/// Uses `arg_count_msg` in the error message (e.g. "requires exactly 1 argument").
fn expect_module_ref<'a>(
    args: &'a [CtValue],
    func_name: &str,
    arg_count_msg: &str,
) -> Result<&'a String, ScriptLangError> {
    if args.is_empty() || args.len() > 1 {
        return Err(ScriptLangError::Message {
            message: format!("{func_name}() {arg_count_msg}"),
        });
    }
    match &args[0] {
        CtValue::String(s) => Ok(s),
        CtValue::ModuleRef(s) => Ok(s),
        other => Err(ScriptLangError::Message {
            message: format!(
                "{func_name}() argument must be string or module, got {}",
                other.type_name()
            ),
        }),
    }
}

/// `caller_env()`: Return a CtValue exposing the caller environment.
pub(crate) fn builtin_caller_env(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
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
pub(crate) fn builtin_caller_module(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
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
pub(crate) fn builtin_expand_alias(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    _expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    let module_ref =
        expect_module_ref(args, "expand_alias", "requires exactly 1 argument")?.clone();

    // Check aliases first
    if let Some(full_name) = macro_env.aliases.get(&module_ref) {
        return Ok(CtValue::String(full_name.clone()));
    }

    // Otherwise return as-is (it's a full module name)
    Ok(CtValue::String(module_ref))
}

/// `require_module(module_ref)`: Ensure a module is required (add to requires list).
pub(crate) fn builtin_require_module(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    let module_ref =
        expect_module_ref(args, "require_module", "requires exactly 1 argument")?.clone();

    // Expand alias if needed (e.g. "H" -> "helper")
    let full_name = macro_env
        .aliases
        .get(&module_ref)
        .cloned()
        .unwrap_or_else(|| module_ref.clone());

    // When inside a `use` context, track the resolved provider module name
    // so that check_use_conflict can report the fully qualified provider module path.
    // expand_macro_hook sets use_provider_module to the raw attribute value;
    // here we upgrade it to the resolved name after alias expansion.
    if expand_env.use_caller_module.is_some() {
        expand_env.use_provider_module = Some(full_name.clone());
    }

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
pub(crate) fn builtin_define_import(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    let module_ref =
        expect_module_ref(args, "define_import", "requires exactly 1 argument")?.clone();

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
pub(crate) fn builtin_define_alias(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
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
pub(crate) fn builtin_define_require(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
) -> BuiltinResult {
    let module_ref =
        expect_module_ref(args, "define_require", "requires exactly 1 argument")?.clone();

    // Expand alias if needed
    let full_name = macro_env
        .aliases
        .get(&module_ref)
        .cloned()
        .unwrap_or(module_ref);

    expand_env.add_require(full_name);
    Ok(CtValue::Nil)
}

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
        // LazyQuote: same as Ast (internal construct, shouldn't appear in invoke_macro args)
        CtValue::LazyQuote(_) => "[lazy_quote]".to_string(),
    }
}

/// `invoke_macro(module_ref, macro_name, args)`: Invoke a macro from another module.
pub(crate) fn builtin_invoke_macro(
    args: &[CtValue],
    macro_env: &mut MacroEnv,
    _ct_env: &mut CtEnv,
    expand_env: &mut ExpandEnv,
    _: &BuiltinRegistry,
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

    // Track caller module for error enrichment
    let caller_module = macro_env
        .current_module
        .clone()
        .unwrap_or_else(|| "<unknown>".to_string());

    // Compute caller source location for enriched error messages.
    let caller_location = expand_env
        .caller_invocation_meta
        .as_ref()
        .map(location)
        .unwrap_or_default();

    // Check that the module is registered in the program.
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

    // Resolve the macro STRICTLY from the target module.
    let definition = expand_env
        .program
        .resolve_macro_in(&resolved_module, &macro_name)
        .cloned()
        .ok_or_else(|| {
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

    // Check macro visibility (private macros only visible in defining module)
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
            // Support nested List/Keyword/Ast in invoke_macro args.
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

    // Build synthetic invocation form.
    let caller_meta = expand_env.caller_invocation_meta.clone();
    let inner_definition_meta = definition.meta.clone();
    let synthetic_invocation_meta = caller_meta.unwrap_or_else(|| FormMeta {
        source_name: Some(format!("{} (via {})", resolved_module, caller_module)),
        start: SourcePosition { row: 0, column: 0 },
        start_byte: 0,
        end: SourcePosition { row: 0, column: 0 },
        end_byte: 0,
    });
    let synthetic_invocation = Form {
        head: macro_name.clone(),
        meta: synthetic_invocation_meta,
        fields: invocation_fields,
    };

    // Expand the macro, enriching errors with caller context.
    let trace_before_invoke = expand_env.expansion_trace.len();
    let expanded_items = expand_macro_invocation_public(
        definition,
        &synthetic_invocation,
        expand_env,
        ExpandRuleScope::Statement,
    )
    .map_err(|e| {
        let intermediate_entries: String = if trace_before_invoke < expand_env.expansion_trace.len()
        {
            expand_env.expansion_trace[trace_before_invoke..]
                .iter()
                .map(|en| format!("{}.{} at {}", en.module_name, en.macro_name, en.location))
                .collect::<Vec<_>>()
                .join(" -> ")
        } else {
            format!(
                "{}.{} at {}",
                resolved_module,
                macro_name,
                location(&inner_definition_meta)
            )
        };
        let current_trace = expand_env.format_expansion_trace();
        let complete_trace = crate::semantic::macro_lang::eval::format_full_trace(
            &intermediate_entries,
            &current_trace,
        );
        let trace_suffix = if complete_trace.is_empty() {
            String::new()
        } else {
            format!(" (expansion trace: {})", complete_trace)
        };

        ScriptLangError::Message {
            message: format!(
                "error expanding `{}` from `{}` (called from `{}`): {}{}",
                macro_name, resolved_module, caller_module, e, trace_suffix
            ),
        }
    })?;

    Ok(CtValue::Ast(expanded_items))
}
