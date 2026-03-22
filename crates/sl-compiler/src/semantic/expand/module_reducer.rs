//! Definition-time reducer for module expansion.
//!
//! This module implements a reducer pattern that processes module children
//! in order, allowing macro-generated forms to affect the definition-time
//! environment for subsequent siblings.

use sl_core::{Form, FormItem, ScriptLangError};

use super::dispatch::{ExpandRuleScope, expand_form_items};
use crate::names::qualified_member_name;
use crate::semantic::env::ExpandEnv;
use crate::semantic::{attr, error_at, required_attr};

/// Process module children using a reducer pattern.
///
/// Each child is processed in order:
/// - Macro invocations are expanded and the resulting forms are re-queued
/// - Definition-time forms (import/require/alias/const/var/function/script/module)
///   update the module state immediately
/// - Other forms are expanded normally
///
/// This ensures that macro-generated definition-time forms affect the
/// environment for subsequent siblings.
pub(crate) fn reduce_module_children(
    children: &[FormItem],
    env: &mut ExpandEnv,
    parent_module: &str,
) -> Result<Vec<FormItem>, ScriptLangError> {
    let mut queue: Vec<FormItem> = children.to_vec();
    let mut output: Vec<FormItem> = Vec::new();

    while let Some(item) = queue.first() {
        // Process the item
        let processed = process_item(item, env, parent_module)?;

        // Remove the processed item from the queue
        queue.remove(0);

        match processed {
            ProcessedItem::Output(items) => {
                // Add to output
                output.extend(items);
            }
            ProcessedItem::Requeue(items) => {
                // Insert at the front of the queue (to maintain order)
                for (i, item) in items.into_iter().enumerate() {
                    queue.insert(i, item);
                }
            }
            ProcessedItem::Skip => {
                // Item was handled (e.g., nested module), don't add to output
            }
        }
    }

    Ok(output)
}

enum ProcessedItem {
    /// Items to add to the final output
    Output(Vec<FormItem>),
    /// Items to re-insert at the front of the queue (e.g., macro expansion results)
    Requeue(Vec<FormItem>),
    /// Item was handled separately, skip it
    Skip,
}

fn process_item(
    item: &FormItem,
    env: &mut ExpandEnv,
    parent_module: &str,
) -> Result<ProcessedItem, ScriptLangError> {
    match item {
        FormItem::Text(text) => {
            if text.trim().is_empty() {
                // Preserve whitespace-only text (for formatting)
                Ok(ProcessedItem::Output(vec![FormItem::Text(text.clone())]))
            } else {
                Err(ScriptLangError::message(
                    "unexpected top-level text in module body",
                ))
            }
        }
        FormItem::Form(form) => process_form(form, env, parent_module),
    }
}

fn process_form(
    form: &Form,
    env: &mut ExpandEnv,
    parent_module: &str,
) -> Result<ProcessedItem, ScriptLangError> {
    match form.head.as_str() {
        "macro" => {
            // Macro definitions are already registered, skip them in output
            Ok(ProcessedItem::Output(Vec::new()))
        }
        "module" => {
            // Nested modules: register alias and expand recursively
            let child_raw_name = required_attr(form, "name")?.to_string();
            let child_module_name = qualified_member_name(parent_module, &child_raw_name);
            env.add_child_alias(child_raw_name.clone(), child_module_name.clone())
                .map_err(|message| error_at(form, message))?;

            // Import the module expansion function lazily to avoid circular dependency
            // This expands the nested module with its own environment
            expand_nested_module(form, env, parent_module)?;

            Ok(ProcessedItem::Skip)
        }
        "import" => {
            if let Some(import_name) = super::string_attr(form, "name") {
                env.add_import(import_name.to_string());
                // In Elixir, `import A` automatically also does `require A`
                // so that macros from A become available.
                env.add_require(import_name.to_string());
            }
            Ok(ProcessedItem::Output(vec![FormItem::Form(form.clone())]))
        }
        "require" => {
            if let Some(require_name) = super::string_attr(form, "name") {
                env.add_require(require_name.to_string());
            }
            Ok(ProcessedItem::Output(vec![FormItem::Form(form.clone())]))
        }
        "alias" => {
            let alias_target = required_attr(form, "name")?;
            let alias_name = alias_name(form)?;
            env.add_alias(alias_name, alias_target.to_string())
                .map_err(|message| error_at(form, message))?;
            Ok(ProcessedItem::Output(vec![FormItem::Form(form.clone())]))
        }
        "const" => {
            // Const forms need special handling - use const_eval
            let expanded = super::declared_types::expand_const_form(form, env)?;
            Ok(ProcessedItem::Output(vec![FormItem::Form(expanded)]))
        }
        "var" => {
            let name = required_attr(form, "name")?.to_string();
            let exported = !is_private(form)?;
            if !env.declare_var(name.clone(), exported) {
                let module_name = env.module.module_name.as_deref().unwrap_or("<unknown>");
                return Err(error_at(
                    form,
                    format!("duplicate var declaration `{module_name}.{name}`"),
                ));
            }
            let expanded = expand_form_items(form, env, ExpandRuleScope::ModuleChild)?;
            Ok(ProcessedItem::Output(expanded))
        }
        "script" => {
            let name = required_attr(form, "name")?.to_string();
            let exported = !is_private(form)?;
            if !env.declare_script(name.clone(), exported) {
                let module_name = env.module.module_name.as_deref().unwrap_or("<unknown>");
                return Err(error_at(
                    form,
                    format!("duplicate script declaration `{module_name}.{name}`"),
                ));
            }
            let expanded = expand_form_items(form, env, ExpandRuleScope::ModuleChild)?;
            Ok(ProcessedItem::Output(expanded))
        }
        "function" => {
            let name = required_attr(form, "name")?.to_string();
            let exported = !is_private(form)?;
            if !env.declare_function(name.clone(), exported) {
                let module_name = env.module.module_name.as_deref().unwrap_or("<unknown>");
                return Err(error_at(
                    form,
                    format!("duplicate function declaration `{module_name}.{name}`"),
                ));
            }
            Ok(ProcessedItem::Output(vec![FormItem::Form(form.clone())]))
        }
        _ => {
            // Check if this is a macro invocation
            if env.resolve_macro(&form.head).is_some() {
                // Expand the macro
                let expanded = expand_form_items(form, env, ExpandRuleScope::ModuleChild)?;
                // Requeue the expanded items so they go through definition-time processing
                Ok(ProcessedItem::Requeue(expanded))
            } else {
                // Regular form, expand normally
                let expanded = expand_form_items(form, env, ExpandRuleScope::ModuleChild)?;
                Ok(ProcessedItem::Output(expanded))
            }
        }
    }
}

fn expand_nested_module(
    form: &Form,
    env: &mut ExpandEnv,
    parent_module: &str,
) -> Result<(), ScriptLangError> {
    // Use the module expansion function from the parent module
    // We use a lazy import to avoid circular dependency
    let _ = super::module::expand_nested_module_form(form, env, Some(parent_module))?;
    Ok(())
}

pub(crate) fn is_private(form: &Form) -> Result<bool, ScriptLangError> {
    match attr(form, "private") {
        None => Ok(false),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(other) => Err(error_at(
            form,
            format!("invalid boolean value `{other}` for `private`"),
        )),
    }
}

pub(crate) fn alias_name(form: &Form) -> Result<String, ScriptLangError> {
    if let Some(alias_name) = attr(form, "as") {
        if alias_name.is_empty() {
            return Err(error_at(form, "<alias> `as` cannot be empty"));
        }
        return Ok(alias_name.to_string());
    }
    let module_name = required_attr(form, "name")?;
    module_name
        .rsplit('.')
        .next()
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .ok_or_else(|| error_at(form, "<alias> requires valid `name`"))
}
