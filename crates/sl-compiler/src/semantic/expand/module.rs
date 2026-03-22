use sl_core::{Form, FormField, FormItem, FormValue, ScriptLangError};

use super::declared_types::expand_const_form;
use super::dispatch::{ExpandRuleScope, expand_form_items};
use super::string_attr;
use crate::names::qualified_member_name;
use crate::semantic::env::{CompilePhase, ExpandEnv};
use crate::semantic::{attr, child_forms, error_at, required_attr};

pub(crate) fn expand_module_form(
    form: &Form,
    env: &mut ExpandEnv,
) -> Result<Form, ScriptLangError> {
    expand_module_form_with_parent(form, env, None)
}

fn expand_module_form_with_parent(
    form: &Form,
    env: &mut ExpandEnv,
    parent_module: Option<&str>,
) -> Result<Form, ScriptLangError> {
    let raw_name = required_attr(form, "name")?.to_string();
    let module_name = match parent_module {
        Some(parent) => qualified_member_name(parent, &raw_name),
        None => raw_name,
    };
    let saved_phase = env.phase;
    let saved_source_name = env.source_name.clone();
    let saved_module = env.module.clone();

    let result = (|| {
        env.begin_module(Some(module_name.clone()), form.meta.source_name.clone())
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
                                if child.head == "module" {
                                    let child_raw_name = required_attr(child, "name")?.to_string();
                                    let child_module_name =
                                        qualified_member_name(&module_name, &child_raw_name);
                                    env.add_child_alias(
                                        child_raw_name.clone(),
                                        child_module_name.clone(),
                                    )
                                    .map_err(|message| error_at(child, message))?;
                                    let _ = expand_module_form_with_parent(
                                        child,
                                        env,
                                        Some(&module_name),
                                    )?;
                                    continue;
                                }
                                let child_items = match child.head.as_str() {
                                    "import" => {
                                        if let Some(import_name) = string_attr(child, "name") {
                                            env.add_import(import_name.to_string());
                                            // In Elixir, `import A` automatically also does `require A`
                                            // so that macros from A become available.
                                            env.add_require(import_name.to_string());
                                        }
                                        vec![FormItem::Form(child.clone())]
                                    }
                                    "require" => {
                                        if let Some(require_name) = string_attr(child, "name") {
                                            env.add_require(require_name.to_string());
                                        }
                                        vec![FormItem::Form(child.clone())]
                                    }
                                    "alias" => {
                                        let alias_target = required_attr(child, "name")?;
                                        let alias_name = alias_name(child)?;
                                        env.add_alias(alias_name, alias_target.to_string())
                                            .map_err(|message| error_at(child, message))?;
                                        vec![FormItem::Form(child.clone())]
                                    }
                                    "const" => {
                                        vec![FormItem::Form(expand_const_form(child, env)?)]
                                    }
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
                                    "function" => {
                                        let name = required_attr(child, "name")?.to_string();
                                        let exported = !is_private(child)?;
                                        if !env.declare_function(name.clone(), exported) {
                                            let module_name = env
                                                .module
                                                .module_name
                                                .as_deref()
                                                .unwrap_or("<unknown>");
                                            return Err(error_at(
                                                child,
                                                format!(
                                                    "duplicate function declaration `{module_name}.{name}`"
                                                ),
                                            ));
                                        }
                                        vec![FormItem::Form(child.clone())]
                                    }
                                    _ => {
                                        expand_form_items(child, env, ExpandRuleScope::ModuleChild)?
                                    }
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
        let mut expanded = Form {
            head: form.head.clone(),
            meta: form.meta.clone(),
            fields,
        };
        rewrite_module_name(&mut expanded, &module_name);
        env.set_module_children(
            child_forms(&expanded)?
                .into_iter()
                .cloned()
                .collect::<Vec<_>>(),
        );
        env.finish_module();
        Ok(expanded)
    })();

    env.phase = saved_phase;
    env.source_name = saved_source_name;
    env.module = saved_module;
    result
}

fn rewrite_module_name(form: &mut Form, module_name: &str) {
    for field in &mut form.fields {
        if field.name == "name" {
            field.value = FormValue::String(module_name.to_string());
            break;
        }
    }
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

fn alias_name(form: &Form) -> Result<String, ScriptLangError> {
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

#[cfg(test)]
mod tests {
    use sl_core::{FormMeta, SourcePosition};

    use crate::semantic::env::ModuleState;

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
                child(node("require", vec![("name", "helper")], vec![])),
                child(node(
                    "alias",
                    vec![("name", "main.helper"), ("as", "h")],
                    vec![],
                )),
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
                child(node("function", vec![("name", "pick")], vec![])),
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

        assert_eq!(children.len(), 7);
        assert_eq!(stored.imports, vec!["helper".to_string()]);
        // `import` automatically also does `require`, so helper appears twice:
        // once from the auto-require, once from the explicit require.
        assert_eq!(
            stored.requires,
            vec!["helper".to_string(), "helper".to_string()]
        );
        assert_eq!(
            stored.aliases.get("h").map(String::as_str),
            Some("main.helper")
        );
        assert!(stored.exports.consts.contains_declared("answer"));
        assert!(stored.exports.functions.contains_declared("pick"));
        assert!(stored.exports.vars.contains_declared("value"));
        assert!(stored.exports.scripts.contains_declared("main"));
    }

    #[test]
    fn expand_module_form_flattens_nested_submodules_into_qualified_names() {
        let module = node(
            "module",
            vec![("name", "main")],
            vec![
                child(node(
                    "module",
                    vec![("name", "helper")],
                    vec![
                        child(node(
                            "module",
                            vec![("name", "grand")],
                            vec![child(node(
                                "script",
                                vec![("name", "entry")],
                                vec![child(node("end", vec![], vec![]))],
                            ))],
                        )),
                        child(node(
                            "script",
                            vec![("name", "relay")],
                            vec![child(node(
                                "goto",
                                vec![("script", "@grand.entry")],
                                vec![],
                            ))],
                        )),
                    ],
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
        let parent = env.program.modules.get("main").expect("parent module");
        let child_module = env
            .program
            .modules
            .get("main.helper")
            .expect("nested module");
        let grandchild_module = env
            .program
            .modules
            .get("main.helper.grand")
            .expect("deep nested module");
        let children = child_forms(&expanded).expect("children");

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].head, "script");
        assert!(parent.exports.scripts.contains_declared("main"));
        assert_eq!(
            parent.child_aliases.get("helper").map(String::as_str),
            Some("main.helper")
        );
        assert_eq!(
            child_module.child_aliases.get("grand").map(String::as_str),
            Some("main.helper.grand")
        );
        assert!(child_module.exports.scripts.contains_declared("relay"));
        assert!(grandchild_module.exports.scripts.contains_declared("entry"));
        assert!(
            env.program
                .module_order
                .contains(&"main.helper".to_string())
        );
        assert!(
            env.program
                .module_order
                .contains(&"main.helper.grand".to_string())
        );
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

        let duplicate_function = node(
            "module",
            vec![("name", "main")],
            vec![
                child(node("function", vec![("name", "pick")], vec![])),
                child(node("function", vec![("name", "pick")], vec![])),
            ],
        );
        let mut env = ExpandEnv::default();
        assert!(
            expand_module_form(&duplicate_function, &mut env)
                .expect_err("duplicate function")
                .to_string()
                .contains("duplicate function declaration")
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

        let duplicate_alias = node(
            "module",
            vec![("name", "main")],
            vec![
                child(node(
                    "alias",
                    vec![("name", "main.helper"), ("as", "h")],
                    vec![],
                )),
                child(node(
                    "alias",
                    vec![("name", "other.helper"), ("as", "h")],
                    vec![],
                )),
            ],
        );
        let mut env = ExpandEnv::default();
        assert!(
            expand_module_form(&duplicate_alias, &mut env)
                .expect_err("duplicate alias")
                .to_string()
                .contains("alias `h` already points")
        );
    }

    #[test]
    fn module_helpers_cover_alias_name_and_name_rewrite_paths() {
        let alias = node("alias", vec![("name", "main.helper")], vec![]);
        assert_eq!(alias_name(&alias).expect("default alias"), "helper");

        let bad_alias = node("alias", vec![("name", ""), ("as", "")], vec![]);
        assert!(
            alias_name(&bad_alias)
                .expect_err("empty alias")
                .to_string()
                .contains("cannot be empty")
        );

        let invalid_target = node("alias", vec![("name", ".")], vec![]);
        assert!(
            alias_name(&invalid_target)
                .expect_err("invalid alias target")
                .to_string()
                .contains("requires valid `name`")
        );

        let mut module = node("module", vec![("name", "main")], vec![]);
        rewrite_module_name(&mut module, "main.inner");
        assert_eq!(
            required_attr(&module, "name").expect("rewritten"),
            "main.inner"
        );

        let private_true = node(
            "script",
            vec![("name", "main"), ("private", "true")],
            vec![],
        );
        let private_false = node(
            "script",
            vec![("name", "main"), ("private", "false")],
            vec![],
        );
        assert!(is_private(&private_true).expect("private true"));
        assert!(!is_private(&private_false).expect("private false"));
    }

    #[test]
    fn expand_module_form_without_parent_preserves_text_and_restores_env() {
        let mut env = ExpandEnv {
            phase: Some(CompilePhase::Script),
            module: ModuleState {
                module_name: Some("outer".to_string()),
                ..ModuleState::default()
            },
            ..ExpandEnv::default()
        };

        let rewritten = expand_module_form(
            &node("module", vec![("name", "main")], vec![text_item("  \n")]),
            &mut env,
        )
        .expect("expand");

        assert_eq!(env.phase, Some(CompilePhase::Script));
        assert_eq!(env.module.module_name.as_deref(), Some("outer"));
        let children = match &rewritten.fields[1].value {
            FormValue::Sequence(items) => items,
            _ => panic!("expected children sequence"),
        };
        assert!(matches!(&children[0], FormItem::Text(text) if text.contains('\n')));
    }
}
