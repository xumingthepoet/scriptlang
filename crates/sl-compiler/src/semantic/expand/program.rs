use std::collections::BTreeSet;

use sl_core::{Form, ScriptLangError};

use super::{
    ConstCatalog, ConstEnv, ConstLookup, ConstValue, ModuleCatalog, ModuleScope, ScopeResolver,
    alias_name, parse_const_value, parse_declared_type_form as parse_declared_type,
    parse_declared_type_name, validate_alias_target, validate_import_target,
    validate_require_target,
};
use crate::semantic::env::{ModuleState, ProgramState};
use crate::semantic::types::{
    DeclaredType, SemanticFunction, SemanticModule, SemanticProgram, SemanticVar,
};
use crate::semantic::{attr, body_expr, error_at, required_attr};

use super::scripts::{analyze_script, rewrite_function_body, rewrite_var_expr};

pub(crate) fn analyze_program(program: &ProgramState) -> Result<SemanticProgram, ScriptLangError> {
    let catalog = ModuleCatalog::build(program)?;
    let mut const_catalog = ConstCatalog::new(&catalog);
    let modules = program
        .module_order
        .iter()
        .map(|module_name| {
            let module = program.modules.get(module_name).ok_or_else(|| {
                ScriptLangError::message(format!(
                    "module `{module_name}` missing expand-time state"
                ))
            })?;
            analyze_module(module, &catalog, &mut const_catalog)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SemanticProgram { modules })
}

fn analyze_module<'a>(
    module: &ModuleState,
    catalog: &'a ModuleCatalog<'a>,
    const_catalog: &mut ConstCatalog<'a>,
) -> Result<SemanticModule, ScriptLangError> {
    let name = module
        .module_name
        .clone()
        .ok_or_else(|| ScriptLangError::message("missing module name in expand state"))?;
    let module_children = module.children.iter().collect::<Vec<_>>();
    let future_const_names = module_children
        .iter()
        .filter(|child| child.head == "const")
        .map(|child| required_attr(child, "name").map(str::to_string))
        .collect::<Result<BTreeSet<_>, _>>()?;

    let mut remaining_const_names = future_const_names;
    let mut const_env = ConstEnv::new();
    let mut scope = ModuleScope::initial(catalog, &name);
    let mut functions = Vec::new();
    let mut vars = Vec::new();
    let mut scripts = Vec::new();

    for child in module_children {
        match child.head.as_str() {
            "import" => {
                let import_name = required_attr(child, "name")?.to_string();
                validate_import_target(catalog, child, &name, &import_name)?;
                scope.add_import(&import_name);
            }
            "require" => {
                let require_name = required_attr(child, "name")?.to_string();
                validate_require_target(catalog, child, &name, &require_name)?;
            }
            "alias" => {
                // Support two syntaxes:
                // 1. <alias name="module" as="alias_name"/> (name=module, as=alias)
                // 2. <alias name="alias_name" target="module"/> (name=alias, target=module)
                let alias_target = if let Some(target) = attr(child, "target") {
                    // Syntax 2: target is the module
                    target.to_string()
                } else {
                    // Syntax 1 or default: name is the module
                    required_attr(child, "name")?.to_string()
                };
                validate_alias_target(catalog, child, &name, &alias_target)?;
                scope.add_alias(&alias_name(child)?, &alias_target);
            }
            "const" => {
                let mut visible = ScopeResolver::new(catalog, const_catalog, &scope);
                let (const_name, value) =
                    analyze_const(child, &const_env, &mut visible, &remaining_const_names)?;
                remaining_const_names.remove(&const_name);
                const_env.insert(const_name, value);
            }
            "function" => {
                let mut visible = ScopeResolver::new(catalog, const_catalog, &scope);
                functions.push(analyze_function(
                    child,
                    &const_env,
                    &mut visible,
                    &remaining_const_names,
                )?);
            }
            "var" => {
                let mut visible = ScopeResolver::new(catalog, const_catalog, &scope);
                vars.push(SemanticVar {
                    name: required_attr(child, "name")?.to_string(),
                    declared_type: parse_declared_type(child)?,
                    expr: rewrite_var_expr(
                        &body_expr(child)?,
                        &const_env,
                        &mut visible,
                        &remaining_const_names,
                        &BTreeSet::new(),
                    )?,
                });
            }
            "script" => scripts.push(analyze_script(
                child,
                catalog,
                const_catalog,
                &scope,
                &const_env,
                &remaining_const_names,
            )?),
            other => {
                return Err(error_at(
                    child,
                    format!("unsupported <module> child <{other}> in MVP"),
                ));
            }
        }
    }

    Ok(SemanticModule {
        name,
        functions,
        vars,
        scripts,
    })
}

fn analyze_function(
    form: &Form,
    const_env: &ConstEnv,
    resolver: &mut ScopeResolver<'_, '_>,
    remaining_const_names: &BTreeSet<String>,
) -> Result<SemanticFunction, ScriptLangError> {
    let param_names = parse_function_args(form)?;
    let shadowed_names = param_names.iter().cloned().collect::<BTreeSet<_>>();
    let body = body_expr(form)?;
    Ok(SemanticFunction {
        name: required_attr(form, "name")?.to_string(),
        param_names,
        return_type: parse_function_return_type(form)?,
        body: rewrite_function_body(
            &body,
            const_env,
            resolver,
            remaining_const_names,
            &shadowed_names,
        )?,
    })
}

fn analyze_const(
    form: &Form,
    const_env: &ConstEnv,
    resolver: &mut impl ConstLookup,
    remaining_const_names: &BTreeSet<String>,
) -> Result<(String, ConstValue), ScriptLangError> {
    let name = required_attr(form, "name")?.to_string();
    let raw = body_expr(form)?;
    let mut blocked = remaining_const_names.clone();
    blocked.remove(&name);
    let declared_type = parse_declared_type(form)?;
    let value = parse_const_value(&raw, const_env, resolver, &blocked, Some(&declared_type))?;
    Ok((name, value))
}

fn parse_function_return_type(form: &Form) -> Result<DeclaredType, ScriptLangError> {
    parse_function_type(attr(form, "return_type"), form)
}

/// Parse a declared type for function arguments from a raw `Type:name` segment.
fn parse_function_type_from_segment(
    raw: &str,
    form: &Form,
) -> Result<(DeclaredType, String), ScriptLangError> {
    let (declared_type, name) = raw
        .split_once(':')
        .ok_or_else(|| error_at(form, format!("invalid function arg declaration `{raw}`")))?;
    let declared_type = parse_function_type(Some(declared_type.trim()), form)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(error_at(
            form,
            format!("invalid function arg declaration `{raw}`"),
        ));
    }
    Ok((declared_type, name.to_string()))
}

/// Parse a declared type with "function" as the element name for error messages.
fn parse_function_type(
    type_name: Option<&str>,
    form: &Form,
) -> Result<DeclaredType, ScriptLangError> {
    parse_declared_type_name(type_name, "function", |message| error_at(form, message))
}

fn parse_function_args(form: &Form) -> Result<Vec<String>, ScriptLangError> {
    let Some(raw) = attr(form, "args") else {
        return Ok(Vec::new());
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let mut names = BTreeSet::new();
    let mut params = Vec::new();
    for segment in raw.split(',').map(str::trim) {
        let (_, name) = parse_function_type_from_segment(segment, form)?;
        if !names.insert(name.clone()) {
            return Err(error_at(form, format!("duplicate function arg `{name}`")));
        }
        params.push(name);
    }
    Ok(params)
}

#[cfg(test)]
mod tests {
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use crate::names::resolved_var_placeholder;
    use crate::semantic::env::ExpandEnv;
    use crate::semantic::expand::expand_raw_forms;
    use crate::semantic::types::{DeclaredType, SemanticStmt};

    use super::{ProgramState, analyze_program};

    fn meta() -> FormMeta {
        FormMeta {
            source_name: Some("main.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 50 },
            start_byte: 0,
            end_byte: 50,
        }
    }

    fn form(head: &str, fields: Vec<FormField>) -> Form {
        Form {
            head: head.to_string(),
            meta: meta(),
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

    fn node(head: &str, attrs: Vec<(&str, &str)>, items: Vec<FormItem>) -> Form {
        let mut fields = attrs
            .into_iter()
            .map(|(k, v)| attr(k, v))
            .collect::<Vec<_>>();
        fields.push(children(items));
        form(head, fields)
    }

    fn text(text: &str) -> FormItem {
        FormItem::Text(text.to_string())
    }

    fn child(form: Form) -> FormItem {
        FormItem::Form(form)
    }

    fn analyzed(forms: Vec<Form>) -> super::SemanticProgram {
        let mut env = ExpandEnv::default();
        let _ = expand_raw_forms(&forms, &mut env).expect("expand");
        analyze_program(&env.program).expect("analyze")
    }

    #[test]
    fn analyze_forms_tracks_declared_type_and_rewrites_script_literals() {
        let program = analyzed(vec![node(
            "module",
            vec![("name", "main")],
            vec![
                child(node(
                    "var",
                    vec![("name", "next"), ("type", "script")],
                    vec![text("@loop")],
                )),
                child(node(
                    "script",
                    vec![("name", "main")],
                    vec![
                        child(node("goto", vec![("script", "next")], vec![])),
                        child(node("text", vec![], vec![text("${next}")])),
                    ],
                )),
                child(node(
                    "script",
                    vec![("name", "loop")],
                    vec![child(node("end", vec![], vec![]))],
                )),
            ],
        )]);

        let module = &program.modules[0];
        assert_eq!(module.vars[0].declared_type, DeclaredType::Script);
        assert_eq!(module.vars[0].expr, "\"main.loop\"");
        assert!(matches!(
            &module.scripts[0].body[0],
            SemanticStmt::Goto { expr } if expr == &resolved_var_placeholder("main.next")
        ));
    }

    #[test]
    fn analyze_forms_accepts_script_const_literals_and_refs() {
        let program = analyzed(vec![node(
            "module",
            vec![("name", "main")],
            vec![
                child(node(
                    "const",
                    vec![("name", "target"), ("type", "script")],
                    vec![text("@loop")],
                )),
                child(node(
                    "const",
                    vec![("name", "same_target"), ("type", "script")],
                    vec![text("target")],
                )),
                child(node(
                    "script",
                    vec![("name", "main")],
                    vec![child(node("goto", vec![("script", "same_target")], vec![]))],
                )),
                child(node(
                    "script",
                    vec![("name", "loop")],
                    vec![child(node("end", vec![], vec![]))],
                )),
            ],
        )]);

        assert!(matches!(
            &program.modules[0].scripts[0].body[0],
            SemanticStmt::Goto { expr } if expr == "\"main.loop\""
        ));
    }

    #[test]
    fn analyze_forms_supports_alias_for_explicit_refs_and_script_literals() {
        let program = analyzed(vec![
            node(
                "module",
                vec![("name", "helper")],
                vec![
                    child(node(
                        "const",
                        vec![("name", "target"), ("type", "script")],
                        vec![text("@entry")],
                    )),
                    child(node(
                        "var",
                        vec![("name", "value"), ("type", "int")],
                        vec![text("1")],
                    )),
                    child(node(
                        "script",
                        vec![("name", "entry")],
                        vec![child(node("end", vec![], vec![]))],
                    )),
                ],
            ),
            node(
                "module",
                vec![("name", "main")],
                vec![
                    child(node("alias", vec![("name", "helper"), ("as", "h")], vec![])),
                    child(node(
                        "var",
                        vec![("name", "next"), ("type", "script")],
                        vec![text("@h.entry")],
                    )),
                    child(node(
                        "var",
                        vec![("name", "copied"), ("type", "int")],
                        vec![text("h.value")],
                    )),
                    child(node(
                        "script",
                        vec![("name", "main")],
                        vec![
                            child(node("goto", vec![("script", "next")], vec![])),
                            child(node("text", vec![], vec![text("${copied}")])),
                        ],
                    )),
                ],
            ),
        ]);

        let module = program
            .modules
            .iter()
            .find(|module| module.name == "main")
            .expect("main module");
        assert_eq!(module.vars[0].expr, "\"helper.entry\"");
        assert_eq!(
            module.vars[1].expr,
            resolved_var_placeholder("helper.value")
        );
        assert!(matches!(
            &module.scripts[0].body[0],
            SemanticStmt::Goto { expr } if expr == &resolved_var_placeholder("main.next")
        ));
    }

    #[test]
    fn analyze_forms_rejects_missing_or_unknown_type_and_invalid_script_const() {
        let missing_type = [node(
            "module",
            vec![("name", "main")],
            vec![
                child(node("var", vec![("name", "next")], vec![text("1")])),
                child(node(
                    "script",
                    vec![("name", "main")],
                    vec![child(node("end", vec![], vec![]))],
                )),
            ],
        )];
        let mut env = ExpandEnv::default();
        let _ = expand_raw_forms(&missing_type, &mut env).expect("expand");
        let error = analyze_program(&env.program).expect_err("missing type should fail");
        assert!(error.to_string().contains("<var> requires `type`"));

        let unknown_type = [node(
            "module",
            vec![("name", "main")],
            vec![
                child(node(
                    "var",
                    vec![("name", "next"), ("type", "number")],
                    vec![text("1")],
                )),
                child(node(
                    "script",
                    vec![("name", "main")],
                    vec![child(node("end", vec![], vec![]))],
                )),
            ],
        )];
        let mut env = ExpandEnv::default();
        let _ = expand_raw_forms(&unknown_type, &mut env).expect("expand");
        let error = analyze_program(&env.program).expect_err("unknown type should fail");
        assert!(error.to_string().contains("unsupported type `number`"));

        let bad_script_const = [node(
            "module",
            vec![("name", "main")],
            vec![
                child(node(
                    "const",
                    vec![("name", "target"), ("type", "script")],
                    vec![text("\"not-a-script\"")],
                )),
                child(node(
                    "script",
                    vec![("name", "main")],
                    vec![child(node("end", vec![], vec![]))],
                )),
            ],
        )];
        let mut env = ExpandEnv::default();
        let _ = expand_raw_forms(&bad_script_const, &mut env).expect("expand");
        let error = analyze_program(&env.program).expect_err("script const should fail");
        assert!(
            error
                .to_string()
                .contains("const declared as `script` must evaluate to a script literal")
        );
    }

    #[test]
    fn analyze_forms_accepts_function_declarations_and_literals() {
        let program = analyzed(vec![
            node(
                "module",
                vec![("name", "helper")],
                vec![child(node(
                    "function",
                    vec![("name", "pick"), ("return_type", "int")],
                    vec![text("return 1;")],
                ))],
            ),
            node(
                "module",
                vec![("name", "main")],
                vec![
                    child(node("alias", vec![("name", "helper"), ("as", "h")], vec![])),
                    child(node(
                        "const",
                        vec![("name", "picker"), ("type", "function")],
                        vec![text("#h.pick")],
                    )),
                    child(node(
                        "var",
                        vec![("name", "chosen"), ("type", "function")],
                        vec![text("picker")],
                    )),
                    child(node(
                        "script",
                        vec![("name", "main")],
                        vec![child(node("text", vec![], vec![text("${chosen}")]))],
                    )),
                ],
            ),
        ]);

        let helper = program
            .modules
            .iter()
            .find(|module| module.name == "helper")
            .expect("helper module");
        assert_eq!(helper.functions[0].return_type, DeclaredType::Int);
        assert_eq!(helper.functions[0].param_names, Vec::<String>::new());
        assert_eq!(helper.functions[0].body, "return 1;");

        let main = program
            .modules
            .iter()
            .find(|module| module.name == "main")
            .expect("main module");
        assert_eq!(main.vars[0].declared_type, DeclaredType::Function);
        assert_eq!(main.vars[0].expr, "\"helper.pick\"");
    }

    #[test]
    fn analyze_program_reports_missing_state_and_invalid_module_children() {
        let mut env = ExpandEnv::default();
        let _ = expand_raw_forms(
            &[
                node("module", vec![("name", "kernel")], vec![]),
                node(
                    "module",
                    vec![("name", "main")],
                    vec![
                        child(node("require", vec![("name", "kernel")], vec![])),
                        child(node(
                            "function",
                            vec![("name", "pick"), ("return_type", "int")],
                            vec![text("return 1;")],
                        )),
                        child(node(
                            "temp",
                            vec![("name", "scratch"), ("type", "int")],
                            vec![text("1")],
                        )),
                    ],
                ),
            ],
            &mut env,
        )
        .expect("expand");

        let invalid_child = analyze_program(&env.program).expect_err("invalid child");
        assert!(
            invalid_child
                .to_string()
                .contains("unsupported <module> child <temp>")
        );

        let mut broken = env.program.clone();
        broken.module_order.push("missing".to_string());
        let missing = analyze_program(&broken).expect_err("missing module state");
        assert!(missing.to_string().contains("missing expand-time state"));
    }

    #[test]
    fn analyze_program_alias_name_helpers_cover_default_and_error_paths() {
        let alias = node("alias", vec![("name", "main.helper")], vec![]);
        assert_eq!(super::alias_name(&alias).expect("default alias"), "helper");

        let empty = node("alias", vec![("name", "main.helper"), ("as", "")], vec![]);
        assert!(
            super::alias_name(&empty)
                .expect_err("empty alias")
                .to_string()
                .contains("cannot be empty")
        );

        let invalid = node("alias", vec![("name", "")], vec![]);
        assert!(
            super::alias_name(&invalid)
                .expect_err("invalid alias")
                .to_string()
                .contains("requires valid `name`")
        );
    }

    #[test]
    fn analyze_program_covers_import_validation_and_missing_module_name() {
        let mut env = ExpandEnv::default();
        let _ = expand_raw_forms(
            &[node(
                "module",
                vec![("name", "main")],
                vec![
                    child(node("import", vec![("name", "missing")], vec![])),
                    child(node(
                        "script",
                        vec![("name", "main")],
                        vec![child(node("end", vec![], vec![]))],
                    )),
                ],
            )],
            &mut env,
        )
        .expect("expand");
        let import_error = analyze_program(&env.program).expect_err("bad import");
        assert!(
            import_error
                .to_string()
                .contains("imported module `missing` does not exist")
        );

        let mut broken = env.program.clone();
        if let Some(main) = broken.modules.get_mut("main") {
            main.module_name = None;
        }
        let missing_name = analyze_program(&broken).expect_err("missing module name");
        assert!(
            missing_name
                .to_string()
                .contains("missing module name in expand state")
        );
    }

    #[test]
    fn analyze_program_reports_missing_module_state() {
        let mut broken = ProgramState::default();
        broken.module_order.push("main".to_string());

        let error = analyze_program(&broken).expect_err("missing module state");
        assert!(
            error
                .to_string()
                .contains("module `main` missing expand-time state")
        );
    }
}
