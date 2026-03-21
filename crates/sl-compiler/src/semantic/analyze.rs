use std::collections::BTreeSet;

use sl_core::{ScriptLangError, TextSegment, TextTemplate};

use super::const_eval::{ConstEnv, ConstValue, parse_const_value};
use super::expr_rewrite::{
    rewrite_expr_with_consts, rewrite_expr_with_vars, rewrite_script_literals,
    rewrite_template_with_consts, rewrite_template_with_vars,
};
use super::resolve::{
    ConstCatalog, ModuleCatalog, ModuleScope, ScopeResolver, validate_import_target,
};
use super::types::{
    DeclaredType, SemanticChoiceOption, SemanticModule, SemanticProgram, SemanticScript,
    SemanticStmt, SemanticVar,
};
use super::{ClassifiedForm, attr, body_expr, body_template, child_forms, error_at, required_attr};
pub(crate) fn analyze_forms(forms: &[ClassifiedForm]) -> Result<SemanticProgram, ScriptLangError> {
    let catalog = ModuleCatalog::build(forms)?;
    let mut const_catalog = ConstCatalog::new(&catalog);
    let modules = forms
        .iter()
        .map(|form| analyze_module(form, &catalog, &mut const_catalog))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SemanticProgram { modules })
}

fn analyze_module<'a>(
    form: &ClassifiedForm,
    catalog: &'a ModuleCatalog<'a>,
    const_catalog: &mut ConstCatalog<'a>,
) -> Result<SemanticModule, ScriptLangError> {
    if form.head != "module" {
        return Err(error_at(
            form,
            format!("top-level <{}> is not supported in MVP", form.head),
        ));
    }

    let name = required_attr(form, "name")?.to_string();
    let module_children = child_forms(form)?;
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
                        body_expr(child)?,
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
    form: &ClassifiedForm,
    const_env: &ConstEnv,
    resolver: &mut impl super::const_eval::ConstLookup,
    remaining_const_names: &BTreeSet<String>,
) -> Result<(String, ConstValue), ScriptLangError> {
    let name = required_attr(form, "name")?.to_string();
    let raw = body_expr(form)?;
    let mut blocked = remaining_const_names.clone();
    blocked.remove(&name);
    let declared_type = parse_declared_type(form)?;
    let value = parse_const_value(raw, const_env, resolver, &blocked, Some(&declared_type))?;
    Ok((name, value))
}

fn analyze_script<'a>(
    form: &ClassifiedForm,
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
    forms: &[&ClassifiedForm],
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
    form: &ClassifiedForm,
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
                body_expr(form)?,
                const_env,
                resolver,
                remaining_const_names,
                shadowed_names,
            )?,
        }),
        "code" => Ok(SemanticStmt::Code {
            code: rewrite_var_expr(
                body_expr(form)?,
                const_env,
                resolver,
                remaining_const_names,
                shadowed_names,
            )?,
        }),
        "text" => Ok(SemanticStmt::Text {
            template: rewrite_var_template(
                parse_text_template(body_template(form)?),
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
                        parse_text_template(required_attr(option, "text")?),
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
                            template,
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

fn parse_text_template(source: &str) -> TextTemplate {
    let mut segments = Vec::new();
    let mut cursor = 0usize;

    while let Some(start_offset) = source[cursor..].find("${") {
        let start = cursor + start_offset;
        if start > cursor {
            segments.push(TextSegment::Literal(source[cursor..start].to_string()));
        }

        let expr_start = start + 2;
        let Some(end_offset) = source[expr_start..].find('}') else {
            if let Some(TextSegment::Literal(prefix)) = segments.last_mut() {
                prefix.push_str(&source[start..]);
            } else {
                segments.push(TextSegment::Literal(source[start..].to_string()));
            }
            cursor = source.len();
            break;
        };
        let expr_end = expr_start + end_offset;
        segments.push(TextSegment::Expr(
            source[expr_start..expr_end].trim().to_string(),
        ));
        cursor = expr_end + 1;
    }

    if cursor < source.len() {
        segments.push(TextSegment::Literal(source[cursor..].to_string()));
    }
    if segments.is_empty() {
        segments.push(TextSegment::Literal(source.to_string()));
    }

    TextTemplate { segments }
}

fn parse_declared_type(form: &ClassifiedForm) -> Result<DeclaredType, ScriptLangError> {
    match attr(form, "type") {
        None => Err(error_at(form, format!("<{}> requires `type`", form.head))),
        Some("array") => Ok(DeclaredType::Array),
        Some("bool") => Ok(DeclaredType::Bool),
        Some("int") => Ok(DeclaredType::Int),
        Some("object") => Ok(DeclaredType::Object),
        Some("script") => Ok(DeclaredType::Script),
        Some("string") => Ok(DeclaredType::String),
        Some(other) => Err(error_at(form, format!("unsupported type `{other}` in MVP"))),
    }
}

#[cfg(test)]
mod tests {
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use crate::names::resolved_var_placeholder;
    use crate::semantic::types::DeclaredType;

    use super::{SemanticStmt, analyze_forms, parse_text_template};
    use crate::semantic::classify_forms;

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
        let classified = classify_forms(&forms).expect("classify");
        analyze_forms(&classified).expect("analyze")
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
        let missing_type = classify_forms(&[node(
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
        )])
        .expect("classify");
        let error = analyze_forms(&missing_type).expect_err("missing type should fail");
        assert!(error.to_string().contains("<var> requires `type`"));

        let unknown_type = classify_forms(&[node(
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
        )])
        .expect("classify");
        let error = analyze_forms(&unknown_type).expect_err("unknown type should fail");
        assert!(error.to_string().contains("unsupported type `number`"));

        let bad_script_const = classify_forms(&[node(
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
        )])
        .expect("classify");
        let error = analyze_forms(&bad_script_const).expect_err("script const should fail");
        assert!(
            error
                .to_string()
                .contains("const declared as `script` must evaluate to a script literal")
        );
    }

    #[test]
    fn parse_text_template_covers_literal_and_expression_shapes() {
        let mixed = parse_text_template("a ${left} b");
        assert_eq!(mixed.segments.len(), 3);
    }
}
