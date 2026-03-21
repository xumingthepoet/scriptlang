use std::collections::BTreeSet;

use sl_core::{Form, ScriptLangError, TextTemplate};

use super::{
    ConstCatalog, ConstEnv, ConstLookup, ConstValue, ModuleCatalog, ModuleScope, ScopeResolver,
    parse_const_value, parse_declared_type_form as parse_declared_type, validate_import_target,
};
use crate::semantic::env::{ModuleState, ProgramState};
use crate::semantic::expr::{
    parse_text_template, rewrite_expr_with_consts, rewrite_expr_with_vars, rewrite_script_literals,
    rewrite_template_with_consts, rewrite_template_with_vars,
};
use crate::semantic::types::{
    SemanticChoiceOption, SemanticModule, SemanticProgram, SemanticScript, SemanticStmt,
    SemanticVar,
};
use crate::semantic::{attr, body_expr, body_template, child_forms, error_at, required_attr};

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
    let mut vars = Vec::new();
    let mut scripts = Vec::new();

    for child in module_children {
        match child.head.as_str() {
            "import" => {
                let import_name = required_attr(child, "name")?.to_string();
                validate_import_target(catalog, child, &name, &import_name)?;
                scope.add_import(&import_name);
            }
            "const" => {
                let mut visible = ScopeResolver::new(catalog, const_catalog, &scope);
                let (const_name, value) =
                    analyze_const(child, &const_env, &mut visible, &remaining_const_names)?;
                remaining_const_names.remove(&const_name);
                const_env.insert(const_name, value);
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
        vars,
        scripts,
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

fn analyze_script<'a>(
    form: &Form,
    catalog: &'a ModuleCatalog<'a>,
    const_catalog: &mut ConstCatalog<'a>,
    scope: &ModuleScope,
    const_env: &ConstEnv,
    remaining_const_names: &BTreeSet<String>,
) -> Result<SemanticScript, ScriptLangError> {
    let mut shadowed_names = BTreeSet::new();
    let mut visible = ScopeResolver::new(catalog, const_catalog, scope);
    Ok(SemanticScript {
        name: required_attr(form, "name")?.to_string(),
        body: analyze_block(
            &child_forms(form)?,
            const_env,
            &mut visible,
            remaining_const_names,
            &mut shadowed_names,
        )?,
    })
}

fn analyze_block(
    forms: &[&Form],
    const_env: &ConstEnv,
    resolver: &mut ScopeResolver<'_, '_>,
    remaining_const_names: &BTreeSet<String>,
    shadowed_names: &mut BTreeSet<String>,
) -> Result<Vec<SemanticStmt>, ScriptLangError> {
    let mut body = Vec::with_capacity(forms.len());
    for form in forms {
        let stmt = analyze_stmt(
            form,
            const_env,
            resolver,
            remaining_const_names,
            shadowed_names,
        )?;
        if let SemanticStmt::Temp { name, .. } = &stmt {
            shadowed_names.insert(name.clone());
        }
        body.push(stmt);
    }
    Ok(body)
}

fn analyze_stmt(
    form: &Form,
    const_env: &ConstEnv,
    resolver: &mut ScopeResolver<'_, '_>,
    remaining_const_names: &BTreeSet<String>,
    shadowed_names: &mut BTreeSet<String>,
) -> Result<SemanticStmt, ScriptLangError> {
    match form.head.as_str() {
        "const" => Err(error_at(
            form,
            "<const> is only supported as a direct <module> child in MVP",
        )),
        "import" => Err(error_at(
            form,
            "<import> is only supported as a direct <module> child in MVP",
        )),
        "temp" => Ok(SemanticStmt::Temp {
            name: required_attr(form, "name")?.to_string(),
            declared_type: parse_declared_type(form)?,
            expr: rewrite_var_expr(
                &body_expr(form)?,
                const_env,
                resolver,
                remaining_const_names,
                shadowed_names,
            )?,
        }),
        "code" => Ok(SemanticStmt::Code {
            code: rewrite_var_expr(
                &body_expr(form)?,
                const_env,
                resolver,
                remaining_const_names,
                shadowed_names,
            )?,
        }),
        "text" => Ok(SemanticStmt::Text {
            template: rewrite_var_template(
                parse_text_template(&body_template(form)?)?,
                const_env,
                resolver,
                remaining_const_names,
                shadowed_names,
            )?,
            tag: attr(form, "tag").map(str::to_string),
        }),
        "if" => Ok(SemanticStmt::If {
            when: rewrite_var_expr(
                required_attr(form, "when")?,
                const_env,
                resolver,
                remaining_const_names,
                shadowed_names,
            )?,
            body: analyze_block(
                &child_forms(form)?,
                const_env,
                resolver,
                remaining_const_names,
                shadowed_names,
            )?,
        }),
        "choice" => {
            let mut options = Vec::new();
            for option in child_forms(form)? {
                if option.head != "option" {
                    return Err(error_at(
                        option,
                        format!(
                            "<choice> only supports <option> children in MVP, got <{}>",
                            option.head
                        ),
                    ));
                }
                options.push(SemanticChoiceOption {
                    text: rewrite_var_template(
                        parse_text_template(required_attr(option, "text")?)?,
                        const_env,
                        resolver,
                        remaining_const_names,
                        shadowed_names,
                    )?,
                    body: analyze_block(
                        &child_forms(option)?,
                        const_env,
                        resolver,
                        remaining_const_names,
                        shadowed_names,
                    )?,
                });
            }
            Ok(SemanticStmt::Choice {
                prompt: attr(form, "text")
                    .map(parse_text_template)
                    .map(|template| {
                        rewrite_var_template(
                            template?,
                            const_env,
                            resolver,
                            remaining_const_names,
                            shadowed_names,
                        )
                    })
                    .transpose()?,
                options,
            })
        }
        "goto" => Ok(SemanticStmt::Goto {
            expr: rewrite_var_expr(
                required_attr(form, "script")?,
                const_env,
                resolver,
                remaining_const_names,
                shadowed_names,
            )?,
        }),
        "end" => Ok(SemanticStmt::End),
        other => Err(error_at(
            form,
            format!("unsupported statement <{other}> in MVP"),
        )),
    }
}

fn rewrite_var_expr(
    source: &str,
    const_env: &ConstEnv,
    resolver: &mut ScopeResolver<'_, '_>,
    remaining_const_names: &BTreeSet<String>,
    shadowed_names: &BTreeSet<String>,
) -> Result<String, ScriptLangError> {
    let rewritten = rewrite_expr_with_consts(
        source,
        const_env,
        resolver,
        remaining_const_names,
        shadowed_names,
    )?;
    let rewritten =
        rewrite_script_literals(&rewritten, resolver.current_module(), resolver.modules())?;
    rewrite_expr_with_vars(&rewritten, resolver, shadowed_names)
}

fn rewrite_var_template(
    template: TextTemplate,
    const_env: &ConstEnv,
    resolver: &mut ScopeResolver<'_, '_>,
    remaining_const_names: &BTreeSet<String>,
    shadowed_names: &BTreeSet<String>,
) -> Result<TextTemplate, ScriptLangError> {
    let rewritten = rewrite_template_with_consts(
        template,
        const_env,
        resolver,
        remaining_const_names,
        shadowed_names,
    )?;
    rewrite_template_with_vars(rewritten, resolver, shadowed_names)
}

#[cfg(test)]
mod tests {
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use crate::names::resolved_var_placeholder;
    use crate::semantic::env::ExpandEnv;
    use crate::semantic::expand::expand_raw_forms;
    use crate::semantic::types::DeclaredType;

    use super::{SemanticStmt, analyze_program};

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
}
