use sl_core::{Form, FormField, FormItem, FormValue, ScriptLangError};

use super::consts::expand_const_form;
use super::rules::{ExpandRuleScope, expand_with_rules};
use super::string_attr;
use crate::semantic::env::{CompilePhase, ExpandEnv};
use crate::semantic::{attr, child_forms, error_at, required_attr};

pub(crate) fn expand_module_form(
    form: &Form,
    env: &mut ExpandEnv,
) -> Result<Form, ScriptLangError> {
    let module_name = required_attr(form, "name")?.to_string();
    env.begin_module(Some(module_name), form.meta.source_name.clone())
        .map_err(|message| error_at(form, message))?;
    env.phase = Some(CompilePhase::Module);

    let mut fields = Vec::with_capacity(form.fields.len());
    for field in &form.fields {
        let mapped = match (&field.name[..], &field.value) {
            ("children", FormValue::Sequence(items)) => {
                let mut rewritten = Vec::new();
                for item in items {
                    match item {
                        FormItem::Text(text) => rewritten.push(FormItem::Text(text.clone())),
                        FormItem::Form(child) => {
                            if child.head == "macro" {
                                continue;
                            }
                            let child = match child.head.as_str() {
                                "import" => {
                                    if let Some(import_name) = string_attr(child, "name") {
                                        env.add_import(import_name.to_string());
                                    }
                                    child.clone()
                                }
                                "const" => expand_const_form(child, env)?,
                                "var" => {
                                    let name = required_attr(child, "name")?.to_string();
                                    let exported = !is_private(child)?;
                                    if !env.declare_var(name.clone(), exported) {
                                        let module_name = env
                                            .module
                                            .module_name
                                            .as_deref()
                                            .unwrap_or("<unknown>");
                                        return Err(error_at(
                                            child,
                                            format!(
                                                "duplicate var declaration `{module_name}.{name}`"
                                            ),
                                        ));
                                    }
                                    expand_with_rules(child, env, ExpandRuleScope::ModuleChild)?
                                }
                                "script" => {
                                    let name = required_attr(child, "name")?.to_string();
                                    let exported = !is_private(child)?;
                                    if !env.declare_script(name.clone(), exported) {
                                        let module_name = env
                                            .module
                                            .module_name
                                            .as_deref()
                                            .unwrap_or("<unknown>");
                                        return Err(error_at(
                                            child,
                                            format!(
                                                "duplicate script declaration `{module_name}.{name}`"
                                            ),
                                        ));
                                    }
                                    expand_with_rules(child, env, ExpandRuleScope::ModuleChild)?
                                }
                                _ => expand_with_rules(child, env, ExpandRuleScope::ModuleChild)?,
                            };
                            rewritten.push(FormItem::Form(child));
                        }
                    }
                }
                FormField {
                    name: field.name.clone(),
                    value: FormValue::Sequence(rewritten),
                }
            }
            _ => field.clone(),
        };
        fields.push(mapped);
    }
    let expanded = Form {
        head: form.head.clone(),
        meta: form.meta.clone(),
        fields,
    };
    env.set_module_children(
        child_forms(&expanded)?
            .into_iter()
            .cloned()
            .collect::<Vec<_>>(),
    );
    env.finish_module();
    Ok(expanded)
}

fn is_private(form: &Form) -> Result<bool, ScriptLangError> {
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
