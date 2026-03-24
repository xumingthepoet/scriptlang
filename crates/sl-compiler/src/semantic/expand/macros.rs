use sl_core::{Form, FormItem, FormValue, ScriptLangError};

use super::dispatch::{ExpandRuleScope, expand_generated_items};
use super::macro_eval::evaluate_macro_items;
use crate::names::qualified_member_name;
use crate::semantic::env::{ExpandEnv, MacroDefinition, MacroParam, TraceEntry};
use crate::semantic::{child_forms, error_at, location, required_attr};

pub(super) fn collect_program_macros(
    forms: &[Form],
    env: &mut ExpandEnv,
) -> Result<(), ScriptLangError> {
    for form in forms {
        if form.head != "module" {
            return Err(error_at(
                form,
                format!("top-level <{}> is not supported in MVP", form.head),
            ));
        }
        collect_module_macros(form, None, env)?;
    }
    Ok(())
}

fn collect_module_macros(
    form: &Form,
    parent_module: Option<&str>,
    env: &mut ExpandEnv,
) -> Result<(), ScriptLangError> {
    let raw_name = required_attr(form, "name")?;
    let module_name = match parent_module {
        Some(parent) => qualified_member_name(parent, raw_name),
        None => raw_name.to_string(),
    };
    for child in child_forms(form)? {
        match child.head.as_str() {
            "macro" => {
                let definition = parse_macro_definition(child, &module_name)?;
                env.program
                    .register_macro(definition)
                    .map_err(|message| error_at(child, message))?;
            }
            "module" => collect_module_macros(child, Some(&module_name), env)?,
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn expand_macro_hook(
    form: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    let definition = env.resolve_macro(&form.head).cloned();
    let Some(definition) = definition else {
        // Check if the macro exists in any module but the module is not in scope
        let macro_exists = env
            .program
            .module_macros
            .values()
            .any(|macros| macros.contains_key(&form.head));
        if macro_exists {
            // Macro exists but the required module is not in scope
            return Err(ScriptLangError::message(format!(
                "macro `{}` requires a module that is not in scope (hint: add <require name=\"...\"/> or <import name=\"...\"/>)",
                form.head
            )));
        }
        // Not a macro at all - clone as-is
        return Ok(vec![FormItem::Form(form.clone())]);
    };

    // Step 6: Track use caller context for conflict detection.
    // When expanding the `use` macro from kernel, push the caller module
    // so that injected public members can be checked for conflicts.
    // Also track the provider module so conflict errors can name the correct provider.
    // Note: we do NOT pop here - the reducer handles the pop after processing
    // all requeued items, so that check_use_conflict sees the correct context.
    let is_use_macro = definition.module_name == "kernel" && form.head == "use";
    if is_use_macro {
        env.push_use_caller();
        // NOTE: use_provider_module is NOT set here from the raw attribute.
        // Step 6.4: It is set by builtin_require_module (in builtins.rs) after
        // alias resolution, giving the fully qualified provider module name.
        // This ensures check_use_conflict reports the resolved module path.
    }
    // Step 4.2: Track the invocation source location for caller_env and invoke_macro.
    // This meta is read by builtin_invoke_macro to correctly attribute remote invocations.
    env.caller_invocation_meta = Some(form.meta.clone());

    // Step 4.4: Push expansion trace entry before expanding.
    // The trace is used to provide a call-stack-like view when nested macro expansion fails.
    // Pop happens AFTER expand_macro_invocation returns (in expand_macro_hook caller),
    // so that invoke_macro sees the full trace including this macro when reporting errors.
    let trace_entry = TraceEntry {
        macro_name: definition.name.clone(),
        module_name: definition.module_name.clone(),
        location: location(&form.meta),
    };
    env.push_expansion_trace(trace_entry);
    let result = expand_macro_invocation(definition, form, env, scope);
    env.pop_expansion_trace();
    result
}

fn parse_macro_definition(
    form: &Form,
    module_name: &str,
) -> Result<MacroDefinition, ScriptLangError> {
    use crate::semantic::env::MacroDefinition;

    let name = required_attr(form, "name")?.to_string();
    let body = form
        .fields
        .iter()
        .find_map(|field| match (&field.name[..], &field.value) {
            ("children", FormValue::Sequence(items)) => Some(items.clone()),
            _ => None,
        })
        .ok_or_else(|| error_at(form, "<macro> requires `children` field"))?;

    // Parse new explicit params protocol
    let params = form
        .fields
        .iter()
        .find(|field| field.name == "params")
        .and_then(|field| match &field.value {
            FormValue::String(params_str) => Some(params_str.as_str()),
            _ => None,
        })
        .map(|params_str| parse_params_declaration(params_str, form))
        .transpose()?;

    // Parse private attribute
    let is_private = form
        .fields
        .iter()
        .find(|field| field.name == "private")
        .and_then(|field| match &field.value {
            FormValue::String(value) => Some(value.as_str()),
            _ => None,
        })
        .map(|value| value == "true")
        .unwrap_or(false);

    Ok(MacroDefinition {
        module_name: module_name.to_string(),
        name,
        params,
        body,
        is_private,
        meta: form.meta.clone(),
    })
}

/// Parse params declaration string like "expr:when,ast:body"
fn parse_params_declaration(
    params_str: &str,
    form: &Form,
) -> Result<Vec<MacroParam>, ScriptLangError> {
    use crate::semantic::env::{MacroParam, MacroParamType};

    let mut params = Vec::new();
    for param_decl in params_str.split(',') {
        let param_decl = param_decl.trim();
        if param_decl.is_empty() {
            continue;
        }

        let parts: Vec<&str> = param_decl.split(':').collect();
        if parts.len() != 2 {
            return Err(error_at(
                form,
                format!(
                    "invalid param declaration `{}`: expected format `type:name`",
                    param_decl
                ),
            ));
        }

        let type_str = parts[0].trim();
        let name = parts[1].trim().to_string();

        let param_type = match type_str {
            "expr" => MacroParamType::Expr,
            "ast" => MacroParamType::Ast,
            "string" => MacroParamType::String,
            "bool" => MacroParamType::Bool,
            "int" => MacroParamType::Int,
            "keyword" => MacroParamType::Keyword,
            "module" => MacroParamType::Module,
            _ => {
                return Err(error_at(
                    form,
                    format!(
                        "unknown param type `{}`: expected one of expr, ast, string, bool, int, keyword, module",
                        type_str
                    ),
                ));
            }
        };

        params.push(MacroParam { param_type, name });
    }

    Ok(params)
}

fn expand_macro_invocation(
    definition: MacroDefinition,
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    // Use new parameter binder
    let mut runtime = super::macro_params::bind_macro_params(&definition, invocation, env)?;
    // Set macro_name from definition (bind_explicit_params creates MacroEnv with empty macro_name)
    runtime.macro_name = definition.name.clone();
    let expanded_items = evaluate_macro_items(&definition.body, invocation, env, runtime)?
        .into_iter()
        .filter(|item| match item {
            FormItem::Text(text) => !text.trim().is_empty(),
            FormItem::Form(_) => true,
        })
        .collect::<Vec<_>>();
    // Note: trace entry is popped by expand_macro_hook after this function returns,
    // so that invoke_macro sees the full trace when reporting errors.
    expand_generated_items(&expanded_items, env, scope)
}

/// Public wrapper for expand_macro_invocation, exposed for use by macro_lang builtins.
pub(crate) fn expand_macro_invocation_public(
    definition: MacroDefinition,
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    expand_macro_invocation(definition, invocation, env, scope)
}

#[cfg(test)]
mod tests {
    use sl_core::{FormField, FormMeta, SourcePosition};

    use super::*;
    use crate::semantic::attr;
    use crate::semantic::env::ExpandEnv;
    use crate::semantic::expand::dispatch::{expand_form_items, expand_with_rules};

    fn meta() -> sl_core::FormMeta {
        FormMeta {
            source_name: Some("main.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 20 },
            start_byte: 0,
            end_byte: 20,
        }
    }

    fn form(head: &str, fields: Vec<FormField>) -> Form {
        Form {
            head: head.to_string(),
            meta: meta(),
            fields,
        }
    }

    fn attr_field(name: &str, value: &str) -> FormField {
        FormField {
            name: name.to_string(),
            value: FormValue::String(value.to_string()),
        }
    }

    fn children_field(items: Vec<FormItem>) -> FormField {
        FormField {
            name: "children".to_string(),
            value: FormValue::Sequence(items),
        }
    }

    fn text_item(value: &str) -> FormItem {
        FormItem::Text(value.to_string())
    }

    fn form_item(head: &str, attrs: Vec<(&str, &str)>, items: Vec<FormItem>) -> FormItem {
        let mut fields = attrs
            .into_iter()
            .map(|(name, value)| attr_field(name, value))
            .collect::<Vec<_>>();
        fields.push(children_field(items));
        FormItem::Form(form(head, fields))
    }

    fn register_macro(env: &mut ExpandEnv, module_name: &str, name: &str, body: Vec<FormItem>) {
        env.program
            .register_macro(MacroDefinition {
                module_name: module_name.to_string(),
                name: name.to_string(),
                params: None,
                body,
                meta: Default::default(),
                is_private: false,
            })
            .expect("register macro");
    }

    #[test]
    fn duplicate_macro_names_are_rejected_at_registration_time() {
        let mut env = ExpandEnv::default();
        env.begin_module(Some("main".to_string()), None)
            .expect("module");

        register_macro(
            &mut env,
            "kernel",
            "dup",
            vec![form_item(
                "quote",
                vec![],
                vec![form_item("end", vec![], vec![])],
            )],
        );
        let err = env
            .program
            .register_macro(MacroDefinition {
                module_name: "kernel".to_string(),
                name: "dup".to_string(),
                params: None,
                body: vec![form_item(
                    "quote",
                    vec![],
                    vec![form_item("script", vec![("name", "main")], vec![])],
                )],
                meta: Default::default(),
                is_private: false,
            })
            .expect_err("duplicate macro");
        assert!(err.contains("duplicate macro declaration"));
    }

    #[test]
    fn expand_with_rules_uses_same_macro_name_in_module_and_statement_positions() {
        let mut env = ExpandEnv::default();
        env.begin_module(Some("main".to_string()), None)
            .expect("module");

        register_macro(
            &mut env,
            "kernel",
            "dup",
            vec![
                form_item(
                    "let",
                    vec![("name", "script_name"), ("type", "string")],
                    vec![form_item("get-attribute", vec![("name", "name")], vec![])],
                ),
                form_item(
                    "quote",
                    vec![],
                    vec![form_item(
                        "script",
                        vec![("name", "${script_name}")],
                        vec![],
                    )],
                ),
            ],
        );

        let module_expanded = expand_with_rules(
            &form(
                "dup",
                vec![attr_field("name", "main"), children_field(vec![])],
            ),
            &mut env,
            ExpandRuleScope::ModuleChild,
        )
        .expect("module macro");
        assert_eq!(module_expanded.head, "script");

        let statement_expanded = expand_with_rules(
            &form(
                "dup",
                vec![
                    attr_field("name", "statement_only"),
                    children_field(vec![form_item("end", vec![], vec![])]),
                ],
            ),
            &mut env,
            ExpandRuleScope::Statement,
        )
        .expect("statement expansion");
        assert_eq!(statement_expanded.head, "script");
    }

    #[test]
    fn expand_with_rules_expands_quote_based_attribute_and_content_splice() {
        let mut env = ExpandEnv::default();
        env.begin_module(Some("main".to_string()), None)
            .expect("module");

        register_macro(
            &mut env,
            "kernel",
            "wrap",
            vec![
                form_item(
                    "let",
                    vec![("name", "when_expr"), ("type", "expr")],
                    vec![form_item("get-attribute", vec![("name", "when")], vec![])],
                ),
                form_item(
                    "let",
                    vec![("name", "content_ast"), ("type", "ast")],
                    vec![form_item("get-content", vec![], vec![])],
                ),
                form_item(
                    "quote",
                    vec![],
                    vec![form_item(
                        "while",
                        vec![
                            ("when", "${when_expr}"),
                            ("__sl_skip_loop_control_capture", "true"),
                        ],
                        vec![
                            form_item("code", vec![], vec![text_item("flag = false;")]),
                            form_item("unquote", vec![], vec![text_item("content_ast")]),
                        ],
                    )],
                ),
            ],
        );

        let expanded = expand_with_rules(
            &form(
                "wrap",
                vec![
                    attr_field("when", "flag"),
                    children_field(vec![form_item("end", vec![], vec![])]),
                ],
            ),
            &mut env,
            ExpandRuleScope::Statement,
        )
        .expect("expand");

        assert_eq!(expanded.head, "while");
        assert_eq!(attr(&expanded, "when"), Some("flag"));
        let children = expanded
            .fields
            .iter()
            .find_map(|field| match (&field.name[..], &field.value) {
                ("children", FormValue::Sequence(items)) => Some(items.clone()),
                _ => None,
            })
            .unwrap_or_default();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn expand_with_rules_rejects_invalid_macro_root_shapes() {
        let mut env = ExpandEnv::default();
        env.begin_module(Some("main".to_string()), None)
            .expect("module");

        register_macro(
            &mut env,
            "kernel",
            "many",
            vec![form_item(
                "quote",
                vec![],
                vec![
                    form_item("end", vec![], vec![]),
                    form_item("end", vec![], vec![]),
                ],
            )],
        );
        register_macro(
            &mut env,
            "kernel",
            "texty",
            vec![form_item("quote", vec![], vec![text_item("just text")])],
        );

        let many_items = expand_form_items(
            &form("many", vec![children_field(vec![])]),
            &mut env,
            ExpandRuleScope::Statement,
        )
        .expect("multi root");
        assert_eq!(many_items.len(), 2);

        let text_error = expand_with_rules(
            &form("texty", vec![children_field(vec![])]),
            &mut env,
            ExpandRuleScope::Statement,
        )
        .expect_err("text root");
        assert!(
            text_error
                .to_string()
                .contains("cannot produce top-level text")
        );
    }

    #[test]
    fn parse_macro_definition_requires_children_field() {
        let error = parse_macro_definition(
            &Form {
                head: "macro".to_string(),
                meta: meta(),
                fields: vec![attr_field("name", "m")],
            },
            "main",
        )
        .expect_err("missing children");

        assert!(
            error
                .to_string()
                .contains("<macro> requires `children` field")
        );
    }
}
