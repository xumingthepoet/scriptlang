use sl_core::{Form, FormField, FormItem, FormValue, ScriptLangError};

use super::declared_types::expand_const_form;
use super::dispatch::{ExpandRuleScope, expand_form_items};
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
                            let child_items = match child.head.as_str() {
                                "import" => {
                                    if let Some(import_name) = string_attr(child, "name") {
                                        env.add_import(import_name.to_string());
                                    }
                                    vec![FormItem::Form(child.clone())]
                                }
                                "const" => vec![FormItem::Form(expand_const_form(child, env)?)],
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
                                    expand_form_items(child, env, ExpandRuleScope::ModuleChild)?
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
                                    expand_form_items(child, env, ExpandRuleScope::ModuleChild)?
                                }
                                _ => expand_form_items(child, env, ExpandRuleScope::ModuleChild)?,
                            };
                            rewritten.extend(child_items);
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

#[cfg(test)]
mod tests {
    use sl_core::{FormMeta, SourcePosition};

    use super::*;

    fn meta() -> FormMeta {
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

    fn node(head: &str, attrs: Vec<(&str, &str)>, items: Vec<FormItem>) -> Form {
        let mut fields = attrs
            .into_iter()
            .map(|(name, value)| attr_field(name, value))
            .collect::<Vec<_>>();
        fields.push(children_field(items));
        form(head, fields)
    }

    fn child(form: Form) -> FormItem {
        FormItem::Form(form)
    }

    #[test]
    fn expand_module_form_tracks_children_and_skips_macro_nodes() {
        let module = node(
            "module",
            vec![("name", "main")],
            vec![
                child(node(
                    "macro",
                    vec![("name", "helper")],
                    vec![child(node("end", vec![], vec![]))],
                )),
                child(node("import", vec![("name", "helper")], vec![])),
                child(node(
                    "const",
                    vec![("name", "answer"), ("type", "int")],
                    vec![text_item("1")],
                )),
                child(node(
                    "var",
                    vec![("name", "value"), ("type", "int")],
                    vec![text_item("1")],
                )),
                child(node(
                    "script",
                    vec![("name", "main")],
                    vec![child(node("end", vec![], vec![]))],
                )),
            ],
        );

        let mut env = ExpandEnv::default();
        let expanded = expand_module_form(&module, &mut env).expect("expand");
        let stored = env.program.modules.get("main").expect("module");
        let children = child_forms(&expanded).expect("children");

        assert_eq!(children.len(), 4);
        assert_eq!(stored.imports, vec!["helper".to_string()]);
        assert!(stored.exports.consts.contains_declared("answer"));
        assert!(stored.exports.vars.contains_declared("value"));
        assert!(stored.exports.scripts.contains_declared("main"));
    }

    #[test]
    fn expand_module_form_rejects_duplicates_and_invalid_private() {
        let duplicate_var = node(
            "module",
            vec![("name", "main")],
            vec![
                child(node(
                    "var",
                    vec![("name", "value"), ("type", "int")],
                    vec![text_item("1")],
                )),
                child(node(
                    "var",
                    vec![("name", "value"), ("type", "int")],
                    vec![text_item("2")],
                )),
            ],
        );
        let mut env = ExpandEnv::default();
        assert!(
            expand_module_form(&duplicate_var, &mut env)
                .expect_err("duplicate var")
                .to_string()
                .contains("duplicate var declaration")
        );

        let duplicate_script = node(
            "module",
            vec![("name", "main")],
            vec![
                child(node("script", vec![("name", "main")], vec![])),
                child(node("script", vec![("name", "main")], vec![])),
            ],
        );
        let mut env = ExpandEnv::default();
        assert!(
            expand_module_form(&duplicate_script, &mut env)
                .expect_err("duplicate script")
                .to_string()
                .contains("duplicate script declaration")
        );

        let invalid_private = node(
            "module",
            vec![("name", "main")],
            vec![child(node(
                "script",
                vec![("name", "main"), ("private", "maybe")],
                vec![],
            ))],
        );
        let mut env = ExpandEnv::default();
        assert!(
            expand_module_form(&invalid_private, &mut env)
                .expect_err("private")
                .to_string()
                .contains("invalid boolean value `maybe`")
        );
    }
}
