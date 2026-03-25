use std::collections::BTreeSet;

use sl_core::{Form, ScriptLangError, TextTemplate};

use super::{
    ConstCatalog, ConstEnv, ModuleCatalog, ModuleScope, ScopeResolver, parse_declared_type_form,
};
use crate::semantic::expr::{
    normalize_expr_escapes, parse_text_template, rewrite_expr_function_calls,
    rewrite_expr_with_consts, rewrite_expr_with_vars, rewrite_special_literals,
    rewrite_template_special_literals, rewrite_template_with_consts, rewrite_template_with_vars,
};
use crate::semantic::types::{SemanticChoiceOption, SemanticScript, SemanticStmt};
use crate::semantic::{attr, body_expr, body_template, child_forms, error_at, required_attr};

pub(super) fn analyze_script<'a>(
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
            declared_type: parse_declared_type_form(form)?,
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
        "while" => Ok(SemanticStmt::While {
            when: rewrite_var_expr(
                required_attr(form, "when")?,
                const_env,
                resolver,
                remaining_const_names,
                shadowed_names,
            )?,
            skip_loop_control_capture: parse_skip_loop_control_capture_attr(form)?,
            body: analyze_block(
                &child_forms(form)?,
                const_env,
                resolver,
                remaining_const_names,
                shadowed_names,
            )?,
        }),
        "break" => {
            let children = child_forms(form)?;
            if !children.is_empty() {
                return Err(error_at(form, "<break> does not support nested statements"));
            }
            Ok(SemanticStmt::Break)
        }
        "continue" => {
            let children = child_forms(form)?;
            if !children.is_empty() {
                return Err(error_at(
                    form,
                    "<continue> does not support nested statements",
                ));
            }
            Ok(SemanticStmt::Continue)
        }
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

fn parse_skip_loop_control_capture_attr(form: &Form) -> Result<bool, ScriptLangError> {
    match attr(form, "__sl_skip_loop_control_capture") {
        None => Ok(false),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(other) => Err(error_at(
            form,
            format!("invalid boolean value `{other}` for `__sl_skip_loop_control_capture`"),
        )),
    }
}

fn rewrite_expr_pipeline(
    source: &str,
    const_env: &ConstEnv,
    resolver: &mut ScopeResolver<'_, '_>,
    remaining_const_names: &BTreeSet<String>,
    shadowed_names: &BTreeSet<String>,
    with_vars: bool,
) -> Result<String, ScriptLangError> {
    let normalized = normalize_expr_escapes(source)?;
    let rewritten = rewrite_expr_with_consts(
        &normalized,
        const_env,
        resolver,
        remaining_const_names,
        shadowed_names,
    )?;
    let rewritten = rewrite_special_literals(&rewritten, resolver)?;
    let rewritten = rewrite_expr_function_calls(&rewritten, resolver, shadowed_names)?;
    if with_vars {
        rewrite_expr_with_vars(&rewritten, resolver, shadowed_names)
    } else {
        Ok(rewritten)
    }
}

pub(super) fn rewrite_var_expr(
    source: &str,
    const_env: &ConstEnv,
    resolver: &mut ScopeResolver<'_, '_>,
    remaining_const_names: &BTreeSet<String>,
    shadowed_names: &BTreeSet<String>,
) -> Result<String, ScriptLangError> {
    rewrite_expr_pipeline(
        source,
        const_env,
        resolver,
        remaining_const_names,
        shadowed_names,
        true,
    )
}

pub(super) fn rewrite_function_body(
    source: &str,
    const_env: &ConstEnv,
    resolver: &mut ScopeResolver<'_, '_>,
    remaining_const_names: &BTreeSet<String>,
    shadowed_names: &BTreeSet<String>,
) -> Result<String, ScriptLangError> {
    rewrite_expr_pipeline(
        source,
        const_env,
        resolver,
        remaining_const_names,
        shadowed_names,
        false,
    )
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
    let rewritten = rewrite_template_special_literals(rewritten, resolver)?;
    rewrite_template_with_vars(rewritten, resolver, shadowed_names)
}

#[cfg(test)]
mod tests {
    use sl_core::TextSegment;

    use crate::semantic::env::ExpandEnv;
    use crate::semantic::expand::{analyze_program, expand_raw_forms};
    use crate::semantic::types::{DeclaredType, SemanticStmt};

    use crate::semantic::expand::test_helpers::{analyzed, child, node, text};

    #[test]
    fn analyze_script_covers_statement_variants_and_special_literals() {
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
                        vec![("name", "answer"), ("type", "int")],
                        vec![text("41")],
                    )),
                    child(node(
                        "var",
                        vec![("name", "next"), ("type", "script")],
                        vec![text("@loop")],
                    )),
                    child(node(
                        "script",
                        vec![("name", "main")],
                        vec![
                            child(node(
                                "temp",
                                vec![("name", "counter"), ("type", "int")],
                                vec![text("answer")],
                            )),
                            child(node("code", vec![], vec![text("#h.pick")])),
                            child(node(
                                "text",
                                vec![("tag", "line")],
                                vec![text("${counter}:${#h.pick}:${@loop}")],
                            )),
                            child(node(
                                "while",
                                vec![
                                    ("when", "counter == 41"),
                                    ("__sl_skip_loop_control_capture", "true"),
                                ],
                                vec![child(node("text", vec![], vec![text("ok")]))],
                            )),
                            child(node(
                                "while",
                                vec![("when", "counter < 45")],
                                vec![
                                    child(node("continue", vec![], vec![])),
                                    child(node("break", vec![], vec![])),
                                ],
                            )),
                            child(node(
                                "choice",
                                vec![("text", "pick ${answer}")],
                                vec![
                                    child(node(
                                        "option",
                                        vec![("text", "A ${#h.pick}")],
                                        vec![child(node("text", vec![], vec![text("a")]))],
                                    )),
                                    child(node(
                                        "option",
                                        vec![("text", "B")],
                                        vec![child(node("end", vec![], vec![]))],
                                    )),
                                ],
                            )),
                            child(node("goto", vec![("script", "@loop")], vec![])),
                            child(node("end", vec![], vec![])),
                        ],
                    )),
                    child(node(
                        "script",
                        vec![("name", "loop")],
                        vec![child(node("end", vec![], vec![]))],
                    )),
                ],
            ),
        ]);

        let main = program
            .modules
            .iter()
            .find(|module| module.name == "main")
            .expect("main module");
        let body = &main.scripts[0].body;

        assert!(matches!(
            &body[0],
            SemanticStmt::Temp { name, declared_type, expr }
                if name == "counter"
                    && *declared_type == DeclaredType::Int
                    && expr == "41"
        ));
        assert!(matches!(
            &body[1],
            SemanticStmt::Code { code } if code == "\"helper.pick\""
        ));
        assert!(matches!(
            &body[2],
            SemanticStmt::Text { template, tag }
                if tag.as_deref() == Some("line")
                    && matches!(&template.segments[0], TextSegment::Expr(expr) if expr == "counter")
                    && matches!(&template.segments[2], TextSegment::Expr(expr) if expr == "\"helper.pick\"")
                    && matches!(&template.segments[4], TextSegment::Expr(expr) if expr == "\"main.loop\"")
        ));
        assert!(matches!(
            &body[3],
            SemanticStmt::While {
                when,
                body,
                skip_loop_control_capture: true
            }
                if when == "counter == 41"
                    && matches!(&body[0], SemanticStmt::Text { .. })
        ));
        assert!(matches!(
            &body[4],
            SemanticStmt::While {
                when,
                body,
                skip_loop_control_capture: false
            }
                if when == "counter < 45"
                    && matches!(&body[0], SemanticStmt::Continue)
                    && matches!(&body[1], SemanticStmt::Break)
        ));
        assert!(matches!(
            &body[5],
            SemanticStmt::Choice { prompt: Some(prompt), options }
                if matches!(&prompt.segments[1], TextSegment::Expr(expr) if expr == "41")
                    && options.len() == 2
                    && matches!(&options[0].text.segments[1], TextSegment::Expr(expr) if expr == "\"helper.pick\"")
        ));
        assert!(matches!(
            &body[6],
            SemanticStmt::Goto { expr } if expr == "\"main.loop\""
        ));
        assert!(matches!(&body[7], SemanticStmt::End));
    }

    #[test]
    fn analyze_script_rejects_invalid_statement_forms() {
        let const_error = {
            let mut env = ExpandEnv::default();
            let forms = vec![node(
                "module",
                vec![("name", "main")],
                vec![child(node(
                    "script",
                    vec![("name", "main")],
                    vec![child(node(
                        "const",
                        vec![("name", "x"), ("type", "int")],
                        vec![text("1")],
                    ))],
                ))],
            )];
            let _ = expand_raw_forms(&forms, &mut env).expect("expand");
            analyze_program(&env.program).expect_err("const in script")
        };
        assert!(const_error.to_string().contains("direct <module> child"));

        let import_error = {
            let mut env = ExpandEnv::default();
            let forms = vec![node(
                "module",
                vec![("name", "main")],
                vec![child(node(
                    "script",
                    vec![("name", "main")],
                    vec![child(node("import", vec![("name", "helper")], vec![]))],
                ))],
            )];
            let _ = expand_raw_forms(&forms, &mut env).expect("expand");
            analyze_program(&env.program).expect_err("import in script")
        };
        assert!(import_error.to_string().contains("direct <module> child"));

        let invalid_choice_error = {
            let mut env = ExpandEnv::default();
            let forms = vec![node(
                "module",
                vec![("name", "main")],
                vec![child(node(
                    "script",
                    vec![("name", "main")],
                    vec![child(node(
                        "choice",
                        vec![],
                        vec![child(node("text", vec![], vec![text("bad")]))],
                    ))],
                ))],
            )];
            let _ = expand_raw_forms(&forms, &mut env).expect("expand");
            analyze_program(&env.program).expect_err("invalid choice")
        };
        assert!(
            invalid_choice_error
                .to_string()
                .contains("only supports <option> children")
        );

        let unsupported_stmt_error = {
            let mut env = ExpandEnv::default();
            let forms = vec![node(
                "module",
                vec![("name", "main")],
                vec![child(node(
                    "script",
                    vec![("name", "main")],
                    vec![child(node("unknown", vec![], vec![]))],
                ))],
            )];
            let _ = expand_raw_forms(&forms, &mut env).expect("expand");
            analyze_program(&env.program).expect_err("unsupported stmt")
        };
        assert!(
            unsupported_stmt_error
                .to_string()
                .contains("unsupported statement <unknown>")
        );

        let bad_break_error = {
            let mut env = ExpandEnv::default();
            let forms = vec![node(
                "module",
                vec![("name", "main")],
                vec![child(node(
                    "script",
                    vec![("name", "main")],
                    vec![child(node(
                        "break",
                        vec![],
                        vec![child(node("end", vec![], vec![]))],
                    ))],
                ))],
            )];
            let _ = expand_raw_forms(&forms, &mut env).expect("expand");
            analyze_program(&env.program).expect_err("break with children")
        };
        assert!(
            bad_break_error
                .to_string()
                .contains("<break> does not support nested statements")
        );

        let bad_loop_attr_error = {
            let mut env = ExpandEnv::default();
            let forms = vec![node(
                "module",
                vec![("name", "main")],
                vec![child(node(
                    "script",
                    vec![("name", "main")],
                    vec![child(node(
                        "while",
                        vec![
                            ("when", "true"),
                            ("__sl_skip_loop_control_capture", "maybe"),
                        ],
                        vec![child(node("end", vec![], vec![]))],
                    ))],
                ))],
            )];
            let _ = expand_raw_forms(&forms, &mut env).expect("expand");
            analyze_program(&env.program).expect_err("bad loop attr")
        };
        assert!(
            bad_loop_attr_error
                .to_string()
                .contains("invalid boolean value `maybe`")
        );
    }
}
