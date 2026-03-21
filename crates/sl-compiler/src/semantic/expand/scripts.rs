use std::collections::BTreeSet;

use sl_core::{Form, ScriptLangError, TextTemplate};

use super::{
    ConstCatalog, ConstEnv, ModuleCatalog, ModuleScope, ScopeResolver, parse_declared_type_form,
};
use crate::semantic::expr::{
    parse_text_template, rewrite_expr_with_consts, rewrite_expr_with_vars, rewrite_script_literals,
    rewrite_template_with_consts, rewrite_template_with_vars,
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

pub(super) fn rewrite_var_expr(
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
