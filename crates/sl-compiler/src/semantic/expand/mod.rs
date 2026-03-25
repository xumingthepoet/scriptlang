mod const_eval;
mod declared_types;
pub(crate) mod dispatch;
mod imports;
pub(crate) mod macro_env;
mod macro_eval;
pub(crate) mod macro_params;
pub(crate) mod macro_values;
pub(crate) mod macros;
pub(crate) mod module;
pub(crate) mod module_reducer;
mod modules;
mod program;
pub(crate) mod quote;
mod scope;
mod scripts;

use sl_core::{Form, FormItem, FormValue, ScriptLangError};

use super::env::ExpandEnv;
use super::types::SemanticProgram;
pub(crate) use const_eval::{ConstEnv, ConstLookup, ConstValue, parse_const_value};
pub(crate) use declared_types::{parse_declared_type_form, parse_declared_type_name};
pub(crate) use dispatch::{ExpandRuleScope, expand_with_rules};
pub(crate) use imports::{validate_alias_target, validate_import_target, validate_require_target};
use macros::collect_program_macros;
use module::expand_module_form;
pub(crate) use modules::ModuleCatalog;
pub(crate) use program::analyze_program;
pub(crate) use scope::{ConstCatalog, ModuleScope, QualifiedConstLookup, ScopeResolver};

/// Expand a list of top-level forms into a `SemanticProgram`.
pub(crate) fn expand_forms(forms: &[Form]) -> Result<SemanticProgram, ScriptLangError> {
    let mut env = ExpandEnv::default().with_phase(super::env::CompilePhase::Module);
    collect_program_macros(forms, &mut env)?;
    let _ = expand_raw_forms(forms, &mut env)?;
    analyze_program(&env.program)
}

/// Expand each form in the list, collecting errors. Updates `env` with module state.
pub(super) fn expand_raw_forms(
    forms: &[Form],
    env: &mut ExpandEnv,
) -> Result<Vec<Form>, ScriptLangError> {
    forms.iter().map(|form| expand_form(form, env)).collect()
}

/// Expand a single form: delegates `<module>` to `expand_module_form`, all others to rule dispatch.
fn expand_form(form: &Form, env: &mut ExpandEnv) -> Result<Form, ScriptLangError> {
    if form.head == "module" {
        return expand_module_form(form, env);
    }
    expand_with_rules(form, env, ExpandRuleScope::ModuleChild)
}

/// Extract the plain text content from a form's `children` field.
/// Returns `None` if the children contain any nested forms (non-text items).
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
    use crate::semantic::env::ExpandEnv;
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
        let stored = env
            .program
            .modules
            .get("main")
            .expect("module state should be stored in program state");

        assert_eq!(expanded.len(), 1);
        assert_eq!(stored.module_name.as_deref(), Some("main"));
        assert_eq!(stored.imports, vec!["other".to_string()]);
        assert!(stored.locals.contains("counter"));
        assert_eq!(
            stored
                .const_decls
                .get("seed")
                .expect("const decl should exist")
                .declared_type,
            DeclaredType::Int
        );
        assert_eq!(
            stored
                .const_decls
                .get("seed")
                .expect("const decl should exist")
                .raw_expr
                .as_deref(),
            Some("1")
        );
        assert_eq!(env.program.module_order, vec!["main".to_string()]);
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
                            children(vec![
                                child(form(
                                    "let",
                                    vec![
                                        attr("name", "text_value"),
                                        attr("type", "string"),
                                        children(vec![child(form(
                                            "get-attribute",
                                            vec![attr("name", "text"), children(vec![])],
                                        ))]),
                                    ],
                                )),
                                child(form(
                                    "quote",
                                    vec![children(vec![child(form(
                                        "text",
                                        vec![children(vec![text("${text_value}")])],
                                    ))])],
                                )),
                            ]),
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
                .resolve_macro(Some("main"), &[], "say")
                .is_some()
        );
        let children = child_forms(&expanded[0]).expect("module children");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].head, "script");
    }

    #[test]
    fn program_macro_registry_resolves_required_module_macros() {
        let forms = vec![
            form(
                "module",
                vec![
                    attr("name", "helper"),
                    children(vec![child(form(
                        "macro",
                        vec![
                            attr("name", "mk"),
                            children(vec![
                                child(form(
                                    "let",
                                    vec![
                                        attr("name", "script_name"),
                                        attr("type", "string"),
                                        children(vec![child(form(
                                            "get-attribute",
                                            vec![attr("name", "name"), children(vec![])],
                                        ))]),
                                    ],
                                )),
                                child(form(
                                    "quote",
                                    vec![children(vec![child(form(
                                        "script",
                                        vec![
                                            attr("name", "${script_name}"),
                                            children(vec![child(form(
                                                "end",
                                                vec![children(vec![])],
                                            ))]),
                                        ],
                                    ))])],
                                )),
                            ]),
                        ],
                    ))]),
                ],
            ),
            form(
                "module",
                vec![
                    attr("name", "main"),
                    children(vec![child(form(
                        "require",
                        vec![attr("name", "helper"), children(vec![])],
                    ))]),
                ],
            ),
        ];

        let mut env = ExpandEnv::default();
        collect_program_macros(&forms, &mut env).expect("collect macros");

        assert!(
            env.program
                .resolve_macro(Some("main"), &["helper".to_string()], "mk")
                .is_some()
        );
    }

    #[test]
    fn program_macro_registry_does_not_use_imports_for_macro_visibility() {
        let forms = vec![
            form(
                "module",
                vec![
                    attr("name", "helper"),
                    children(vec![child(form(
                        "macro",
                        vec![
                            attr("name", "mk"),
                            children(vec![child(form(
                                "quote",
                                vec![children(vec![child(form(
                                    "script",
                                    vec![
                                        attr("name", "nested"),
                                        children(vec![child(form("end", vec![children(vec![])]))]),
                                    ],
                                ))])],
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

        assert!(env.program.resolve_macro(Some("main"), &[], "mk").is_none());
    }

    #[test]
    fn collect_program_macros_flattens_nested_module_macro_namespaces() {
        let forms = vec![form(
            "module",
            vec![
                attr("name", "main"),
                children(vec![child(form(
                    "module",
                    vec![
                        attr("name", "helper"),
                        children(vec![child(form(
                            "macro",
                            vec![
                                attr("name", "mk"),
                                children(vec![child(form(
                                    "quote",
                                    vec![children(vec![child(form(
                                        "script",
                                        vec![
                                            attr("name", "nested"),
                                            children(vec![child(form(
                                                "end",
                                                vec![children(vec![])],
                                            ))]),
                                        ],
                                    ))])],
                                ))]),
                            ],
                        ))]),
                    ],
                ))]),
            ],
        )];

        let mut env = ExpandEnv::default();
        collect_program_macros(&forms, &mut env).expect("collect macros");

        assert!(
            env.program
                .resolve_macro(Some("main.helper"), &[], "mk")
                .is_some()
        );
    }

    #[test]
    fn program_macro_registry_rejects_same_name_duplicates() {
        let forms = vec![form(
            "module",
            vec![
                attr("name", "helper"),
                children(vec![
                    child(form(
                        "macro",
                        vec![
                            attr("name", "dup"),
                            children(vec![child(form("end", vec![children(vec![])]))]),
                        ],
                    )),
                    child(form(
                        "macro",
                        vec![
                            attr("name", "dup"),
                            children(vec![
                                child(form(
                                    "let",
                                    vec![
                                        attr("name", "script_name"),
                                        attr("type", "string"),
                                        children(vec![child(form(
                                            "get-attribute",
                                            vec![attr("name", "name"), children(vec![])],
                                        ))]),
                                    ],
                                )),
                                child(form(
                                    "quote",
                                    vec![children(vec![child(form(
                                        "script",
                                        vec![
                                            attr("name", "${script_name}"),
                                            children(vec![child(form(
                                                "end",
                                                vec![children(vec![])],
                                            ))]),
                                        ],
                                    ))])],
                                )),
                            ]),
                        ],
                    )),
                ]),
            ],
        )];

        let mut env = ExpandEnv::default();
        let err = collect_program_macros(&forms, &mut env).expect_err("duplicate macro");
        assert!(err.to_string().contains("duplicate macro declaration"));
    }
}
