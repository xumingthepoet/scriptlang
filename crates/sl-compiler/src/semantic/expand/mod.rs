mod const_values;
mod consts;
mod module;
mod program;
mod rules;
mod scope;

use sl_core::{Form, FormField, FormItem, FormValue, ScriptLangError};

use super::env::{CompilePhase, ExpandEnv, MacroDefinition, MacroScope};
use super::types::SemanticProgram;
use crate::semantic::{attr, child_forms, error_at, required_attr};
pub(crate) use const_values::{ConstEnv, ConstLookup, ConstValue, parse_const_value};
pub(crate) use consts::parse_declared_type_form;
use module::expand_module_form;
pub(crate) use program::analyze_program;
use rules::{ExpandRuleScope, expand_with_rules};
pub(crate) use scope::{
    ConstCatalog, ModuleCatalog, ModuleScope, QualifiedConstLookup, ScopeResolver,
    validate_import_target,
};

pub(crate) fn expand_forms(forms: &[Form]) -> Result<SemanticProgram, ScriptLangError> {
    let mut env = ExpandEnv::default().with_phase(CompilePhase::Module);
    collect_program_macros(forms, &mut env)?;
    let _ = expand_raw_forms(forms, &mut env)?;
    analyze_program(&env.program)
}

pub(super) fn expand_raw_forms(
    forms: &[Form],
    env: &mut ExpandEnv,
) -> Result<Vec<Form>, ScriptLangError> {
    forms.iter().map(|form| expand_form(form, env)).collect()
}

fn collect_program_macros(forms: &[Form], env: &mut ExpandEnv) -> Result<(), ScriptLangError> {
    for form in forms {
        if form.head != "module" {
            return Err(error_at(
                form,
                format!("top-level <{}> is not supported in MVP", form.head),
            ));
        }
        let module_name = required_attr(form, "name")?.to_string();
        for child in child_forms(form)? {
            if child.head != "macro" {
                continue;
            }
            let definition = parse_macro_definition(child, &module_name)?;
            env.program
                .register_macro(definition)
                .map_err(|message| error_at(child, message))?;
        }
    }
    Ok(())
}

fn parse_macro_definition(
    form: &Form,
    module_name: &str,
) -> Result<MacroDefinition, ScriptLangError> {
    let name = required_attr(form, "name")?.to_string();
    let scope = match attr(form, "scope") {
        Some("module") => MacroScope::ModuleChild,
        None | Some("statement") => MacroScope::Statement,
        Some(other) => {
            return Err(error_at(
                form,
                format!("unsupported macro scope `{other}` in MVP"),
            ));
        }
    };
    let body = form
        .fields
        .iter()
        .find_map(|field| match (&field.name[..], &field.value) {
            ("children", FormValue::Sequence(items)) => Some(items.clone()),
            _ => None,
        })
        .ok_or_else(|| error_at(form, "<macro> requires `children` field"))?;
    Ok(MacroDefinition {
        module_name: module_name.to_string(),
        name,
        scope,
        body,
    })
}

fn expand_form(form: &Form, env: &mut ExpandEnv) -> Result<Form, ScriptLangError> {
    if form.head == "module" {
        return expand_module_form(form, env);
    }
    expand_with_rules(form, env, ExpandRuleScope::ModuleChild)
}

pub(super) fn string_attr<'a>(form: &'a Form, name: &str) -> Option<&'a str> {
    form.fields
        .iter()
        .find_map(|field| match (&field.name[..], &field.value) {
            (field_name, FormValue::String(value)) if field_name == name => Some(value.as_str()),
            _ => None,
        })
}

pub(super) fn map_child_forms(
    form: &Form,
    mut rewrite: impl FnMut(&Form) -> Result<Form, ScriptLangError>,
) -> Result<Form, ScriptLangError> {
    let mut fields = Vec::with_capacity(form.fields.len());
    for field in &form.fields {
        let mapped = match (&field.name[..], &field.value) {
            ("children", FormValue::Sequence(items)) => FormField {
                name: field.name.clone(),
                value: FormValue::Sequence(
                    items
                        .iter()
                        .map(|item| match item {
                            FormItem::Text(text) => Ok(FormItem::Text(text.clone())),
                            FormItem::Form(child) => Ok(FormItem::Form(rewrite(child)?)),
                        })
                        .collect::<Result<Vec<_>, ScriptLangError>>()?,
                ),
            },
            _ => field.clone(),
        };
        fields.push(mapped);
    }
    Ok(Form {
        head: form.head.clone(),
        meta: form.meta.clone(),
        fields,
    })
}

pub(super) fn raw_body_text(form: &Form) -> Option<String> {
    let mut buffer = String::new();
    let mut saw_text = false;
    for field in &form.fields {
        if let FormValue::Sequence(items) = &field.value {
            if field.name != "children" {
                continue;
            }
            for item in items {
                match item {
                    FormItem::Text(text) => {
                        buffer.push_str(text);
                        saw_text = true;
                    }
                    FormItem::Form(_) => return None,
                }
            }
        }
    }
    if saw_text {
        Some(buffer.trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use crate::semantic::child_forms;
    use crate::semantic::env::{ExpandEnv, MacroScope};
    use crate::semantic::types::DeclaredType;

    use super::{collect_program_macros, expand_forms, expand_raw_forms};

    fn form(head: &str, fields: Vec<FormField>) -> Form {
        Form {
            head: head.to_string(),
            meta: FormMeta {
                source_name: Some("main.xml".to_string()),
                start: SourcePosition { row: 1, column: 1 },
                end: SourcePosition { row: 1, column: 20 },
                start_byte: 0,
                end_byte: 20,
            },
            fields,
        }
    }

    fn attr(name: &str, value: &str) -> FormField {
        FormField {
            name: name.to_string(),
            value: FormValue::String(value.to_string()),
        }
    }

    fn children(items: Vec<FormItem>) -> FormField {
        FormField {
            name: "children".to_string(),
            value: FormValue::Sequence(items),
        }
    }

    fn text(value: &str) -> FormItem {
        FormItem::Text(value.to_string())
    }

    fn child(form: Form) -> FormItem {
        FormItem::Form(form)
    }

    #[test]
    fn expand_forms_routes_forms_into_current_semantic_pipeline() {
        let error = expand_forms(&[Form {
            head: "module".to_string(),
            meta: FormMeta {
                source_name: Some("main.xml".to_string()),
                start: SourcePosition { row: 1, column: 1 },
                end: SourcePosition { row: 1, column: 20 },
                start_byte: 0,
                end_byte: 20,
            },
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(Vec::new()),
            }],
        }])
        .expect_err("missing module name should fail");

        assert!(error.to_string().contains("<module> requires `name`"));
    }

    #[test]
    fn expand_raw_forms_updates_env_with_module_import_const_and_local_state() {
        let forms = vec![form(
            "module",
            vec![
                attr("name", "main"),
                children(vec![
                    child(form(
                        "import",
                        vec![attr("name", "other"), children(vec![])],
                    )),
                    child(form(
                        "const",
                        vec![
                            attr("name", "seed"),
                            attr("type", "int"),
                            children(vec![text("1")]),
                        ],
                    )),
                    child(form(
                        "script",
                        vec![
                            attr("name", "main"),
                            children(vec![child(form(
                                "temp",
                                vec![
                                    attr("name", "counter"),
                                    attr("type", "int"),
                                    children(vec![text("1")]),
                                ],
                            ))]),
                        ],
                    )),
                ]),
            ],
        )];

        let mut env = ExpandEnv::default();
        let expanded = expand_raw_forms(&forms, &mut env).expect("expand");

        assert_eq!(expanded.len(), 1);
        assert_eq!(env.module.module_name.as_deref(), Some("main"));
        assert_eq!(env.module.imports, vec!["other".to_string()]);
        assert!(env.module.locals.contains("counter"));
        assert_eq!(
            env.module
                .const_decls
                .get("seed")
                .expect("const decl should exist")
                .declared_type,
            DeclaredType::Int
        );
        assert_eq!(
            env.module
                .const_decls
                .get("seed")
                .expect("const decl should exist")
                .raw_expr
                .as_deref(),
            Some("1")
        );
        let stored = env
            .program
            .modules
            .get("main")
            .expect("module state should be stored in program state");
        assert_eq!(env.program.module_order, vec!["main".to_string()]);
        assert_eq!(stored.imports, vec!["other".to_string()]);
        assert!(stored.locals.contains("counter"));
        assert_eq!(stored.children.len(), 3);
    }

    #[test]
    fn expand_raw_forms_collects_multiple_module_states() {
        let forms = vec![
            form(
                "module",
                vec![
                    attr("name", "main"),
                    children(vec![child(form(
                        "script",
                        vec![attr("name", "main"), children(vec![])],
                    ))]),
                ],
            ),
            form(
                "module",
                vec![
                    attr("name", "other"),
                    children(vec![child(form(
                        "script",
                        vec![attr("name", "entry"), children(vec![])],
                    ))]),
                ],
            ),
        ];

        let mut env = ExpandEnv::default();
        let expanded = expand_raw_forms(&forms, &mut env).expect("expand");

        assert_eq!(expanded.len(), 2);
        assert_eq!(
            env.program.module_order,
            vec!["main".to_string(), "other".to_string()]
        );
        assert!(env.program.modules.contains_key("main"));
        assert!(env.program.modules.contains_key("other"));
    }

    #[test]
    fn expand_raw_forms_registers_export_visibility_in_module_state() {
        let forms = vec![form(
            "module",
            vec![
                attr("name", "main"),
                children(vec![
                    child(form(
                        "const",
                        vec![
                            attr("name", "public_target"),
                            attr("type", "script"),
                            children(vec![text("@main.loop")]),
                        ],
                    )),
                    child(form(
                        "var",
                        vec![
                            attr("name", "hidden_value"),
                            attr("type", "int"),
                            attr("private", "true"),
                            children(vec![text("1")]),
                        ],
                    )),
                    child(form(
                        "script",
                        vec![
                            attr("name", "loop"),
                            attr("private", "true"),
                            children(vec![]),
                        ],
                    )),
                ]),
            ],
        )];

        let mut env = ExpandEnv::default();
        let _ = expand_raw_forms(&forms, &mut env).expect("expand");
        let module = env.program.modules.get("main").expect("module state");

        assert!(module.exports.consts.contains_exported("public_target"));
        assert!(module.exports.vars.contains_declared("hidden_value"));
        assert!(!module.exports.vars.contains_exported("hidden_value"));
        assert!(module.exports.scripts.contains_declared("loop"));
        assert!(!module.exports.scripts.contains_exported("loop"));
    }

    #[test]
    fn expand_raw_forms_collects_program_macros_and_removes_macro_nodes_from_children() {
        let forms = vec![form(
            "module",
            vec![
                attr("name", "kernel"),
                children(vec![
                    child(form(
                        "macro",
                        vec![
                            attr("name", "say"),
                            attr("scope", "statement"),
                            children(vec![child(form(
                                "text",
                                vec![children(vec![text("{{text}}")])],
                            ))]),
                        ],
                    )),
                    child(form("script", vec![attr("name", "main"), children(vec![])])),
                ]),
            ],
        )];

        let mut env = ExpandEnv::default();
        collect_program_macros(&forms, &mut env).expect("collect macros");
        let expanded = expand_raw_forms(&forms, &mut env).expect("expand");

        assert!(
            env.program
                .resolve_macro(Some("main"), &[], "say", MacroScope::Statement)
                .is_some()
        );
        let children = child_forms(&expanded[0]).expect("module children");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].head, "script");
    }

    #[test]
    fn program_macro_registry_resolves_imported_module_macros() {
        let forms = vec![
            form(
                "module",
                vec![
                    attr("name", "helper"),
                    children(vec![child(form(
                        "macro",
                        vec![
                            attr("name", "mk"),
                            attr("scope", "module"),
                            children(vec![child(form(
                                "script",
                                vec![
                                    attr("name", "{{name}}"),
                                    children(vec![child(form("end", vec![children(vec![])]))]),
                                ],
                            ))]),
                        ],
                    ))]),
                ],
            ),
            form(
                "module",
                vec![
                    attr("name", "main"),
                    children(vec![child(form(
                        "import",
                        vec![attr("name", "helper"), children(vec![])],
                    ))]),
                ],
            ),
        ];

        let mut env = ExpandEnv::default();
        collect_program_macros(&forms, &mut env).expect("collect macros");

        assert!(
            env.program
                .resolve_macro(
                    Some("main"),
                    &["helper".to_string()],
                    "mk",
                    MacroScope::ModuleChild
                )
                .is_some()
        );
    }

    #[test]
    fn program_macro_registry_allows_same_name_for_different_scopes() {
        let forms = vec![form(
            "module",
            vec![
                attr("name", "helper"),
                children(vec![
                    child(form(
                        "macro",
                        vec![
                            attr("name", "dup"),
                            attr("scope", "statement"),
                            children(vec![child(form("end", vec![children(vec![])]))]),
                        ],
                    )),
                    child(form(
                        "macro",
                        vec![
                            attr("name", "dup"),
                            attr("scope", "module"),
                            children(vec![child(form(
                                "script",
                                vec![
                                    attr("name", "{{name}}"),
                                    children(vec![child(form("end", vec![children(vec![])]))]),
                                ],
                            ))]),
                        ],
                    )),
                ]),
            ],
        )];

        let mut env = ExpandEnv::default();
        collect_program_macros(&forms, &mut env).expect("collect macros");

        assert!(
            env.program
                .resolve_macro(Some("helper"), &[], "dup", MacroScope::Statement)
                .is_some()
        );
        assert!(
            env.program
                .resolve_macro(Some("helper"), &[], "dup", MacroScope::ModuleChild)
                .is_some()
        );
    }
}
