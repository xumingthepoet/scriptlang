use sl_core::{Form, FormField, FormItem, FormValue, ScriptLangError};

use super::dispatch::{ExpandRuleScope, expand_generated_items, macro_scope};
use super::macro_eval::{evaluate_macro_items, uses_macro_evaluator};
use crate::semantic::env::{ExpandEnv, MacroDefinition};
use crate::semantic::{attr, child_forms, error_at, required_attr};

pub(super) fn collect_program_macros(
    forms: &[Form],
    env: &mut ExpandEnv,
) -> Result<(), ScriptLangError> {
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

pub(super) fn expand_macro_hook(
    form: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    let macro_scope = macro_scope(scope);
    let definition = env.resolve_macro(&form.head, macro_scope).cloned();
    let Some(definition) = definition else {
        return Ok(vec![FormItem::Form(form.clone())]);
    };
    expand_macro_invocation(definition, form, env, scope)
}

fn parse_macro_definition(
    form: &Form,
    module_name: &str,
) -> Result<MacroDefinition, ScriptLangError> {
    let name = required_attr(form, "name")?.to_string();
    let scope = match attr(form, "scope") {
        Some("module") => crate::semantic::env::MacroScope::ModuleChild,
        None | Some("statement") => crate::semantic::env::MacroScope::Statement,
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

fn expand_macro_invocation(
    definition: MacroDefinition,
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    if uses_macro_evaluator(&definition.body) {
        let items = evaluate_macro_items(&definition.body, invocation, env, scope)?;
        return expand_generated_items(&items, env, scope);
    }

    let expanded_items = expand_template_macro_items(&definition.body, invocation, env, scope)?
        .into_iter()
        .filter(|item| match item {
            FormItem::Text(text) => !text.trim().is_empty(),
            FormItem::Form(_) => true,
        })
        .collect::<Vec<_>>();
    expand_generated_items(&expanded_items, env, scope)
}

fn expand_template_macro_items(
    items: &[FormItem],
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    let invocation_children = invocation_children(invocation);
    let mut expanded = Vec::new();
    for item in items {
        match item {
            FormItem::Text(text) => {
                expanded.push(FormItem::Text(substitute_text(text, invocation)))
            }
            FormItem::Form(node) if node.head == "yield" => {
                expanded.extend(invocation_children.clone());
            }
            FormItem::Form(node) => {
                expanded.push(FormItem::Form(expand_template_macro_form(
                    node, invocation, env, scope,
                )?));
            }
        }
    }
    Ok(expanded)
}

fn expand_template_macro_form(
    form: &Form,
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Form, ScriptLangError> {
    let items = expand_template_macro_form_items(form, invocation, env, scope)?;
    if items.len() != 1 {
        return Err(ScriptLangError::message(format!(
            "macro expansion of <{}> must produce exactly one root form in nested position",
            form.head
        )));
    }
    match items.into_iter().next().expect("single item") {
        FormItem::Form(form) => Ok(form),
        FormItem::Text(_) => Err(ScriptLangError::message(format!(
            "macro expansion of <{}> cannot produce text in nested form position",
            form.head
        ))),
    }
}

fn expand_template_macro_form_items(
    form: &Form,
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    let mut fields = Vec::with_capacity(form.fields.len());
    for field in &form.fields {
        let value = match &field.value {
            FormValue::String(text) => FormValue::String(substitute_text(text, invocation)),
            FormValue::Sequence(items) => {
                FormValue::Sequence(expand_template_macro_items(items, invocation, env, scope)?)
            }
        };
        fields.push(FormField {
            name: field.name.clone(),
            value,
        });
    }
    let expanded = Form {
        head: form.head.clone(),
        meta: invocation.meta.clone(),
        fields,
    };
    expand_generated_items(&[FormItem::Form(expanded)], env, scope)
}

fn substitute_text(source: &str, invocation: &Form) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(start) = source[cursor..].find("{{") {
        let start = cursor + start;
        output.push_str(&source[cursor..start]);
        let expr_start = start + 2;
        let Some(end) = source[expr_start..].find("}}") else {
            output.push_str(&source[start..]);
            return output;
        };
        let end = expr_start + end;
        let key = source[expr_start..end].trim();
        output.push_str(attr(invocation, key).unwrap_or_default());
        cursor = end + 2;
    }
    output.push_str(&source[cursor..]);
    output
}

fn invocation_children(invocation: &Form) -> Vec<FormItem> {
    invocation
        .fields
        .iter()
        .find_map(|field| match (&field.name[..], &field.value) {
            ("children", FormValue::Sequence(items)) => Some(items.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use sl_core::{FormMeta, SourcePosition};

    use super::*;
    use crate::semantic::env::{ExpandEnv, MacroScope};
    use crate::semantic::expand::dispatch::{expand_form_items, expand_with_rules};

    fn meta() -> sl_core::FormMeta {
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

    fn form_item(head: &str, attrs: Vec<(&str, &str)>, items: Vec<FormItem>) -> FormItem {
        let mut fields = attrs
            .into_iter()
            .map(|(name, value)| attr_field(name, value))
            .collect::<Vec<_>>();
        fields.push(children_field(items));
        FormItem::Form(form(head, fields))
    }

    fn register_macro(
        env: &mut ExpandEnv,
        module_name: &str,
        name: &str,
        scope: MacroScope,
        body: Vec<FormItem>,
    ) {
        env.program
            .register_macro(MacroDefinition {
                module_name: module_name.to_string(),
                name: name.to_string(),
                scope,
                body,
            })
            .expect("register macro");
    }

    #[test]
    fn expand_with_rules_dispatches_same_name_macros_by_scope() {
        let mut env = ExpandEnv::default();
        env.begin_module(Some("main".to_string()), None)
            .expect("module");

        register_macro(
            &mut env,
            "kernel",
            "dup",
            MacroScope::Statement,
            vec![form_item("end", vec![], vec![])],
        );
        register_macro(
            &mut env,
            "kernel",
            "dup",
            MacroScope::ModuleChild,
            vec![form_item("script", vec![("name", "{{name}}")], vec![])],
        );

        let module_expanded = expand_with_rules(
            &form(
                "dup",
                vec![attr_field("name", "main"), children_field(vec![])],
            ),
            &mut env,
            ExpandRuleScope::ModuleChild,
        )
        .expect("module macro");
        assert_eq!(module_expanded.head, "script");

        let statement_expanded = expand_with_rules(
            &form("dup", vec![children_field(vec![])]),
            &mut env,
            ExpandRuleScope::Statement,
        )
        .expect("statement macro");
        assert_eq!(statement_expanded.head, "end");
    }

    #[test]
    fn expand_with_rules_expands_yield_and_attribute_substitution() {
        let mut env = ExpandEnv::default();
        env.begin_module(Some("main".to_string()), None)
            .expect("module");

        register_macro(
            &mut env,
            "kernel",
            "wrap",
            MacroScope::Statement,
            vec![form_item(
                "if",
                vec![("when", "{{when}}")],
                vec![form_item("yield", vec![], vec![])],
            )],
        );

        let expanded = expand_with_rules(
            &form(
                "wrap",
                vec![
                    attr_field("when", "flag"),
                    children_field(vec![form_item("end", vec![], vec![])]),
                ],
            ),
            &mut env,
            ExpandRuleScope::Statement,
        )
        .expect("expand");

        assert_eq!(expanded.head, "if");
        assert_eq!(attr(&expanded, "when"), Some("flag"));
        let children = invocation_children(&expanded);
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn expand_with_rules_rejects_invalid_macro_root_shapes() {
        let mut env = ExpandEnv::default();
        env.begin_module(Some("main".to_string()), None)
            .expect("module");

        register_macro(
            &mut env,
            "kernel",
            "many",
            MacroScope::Statement,
            vec![
                form_item("end", vec![], vec![]),
                form_item("end", vec![], vec![]),
            ],
        );
        register_macro(
            &mut env,
            "kernel",
            "texty",
            MacroScope::Statement,
            vec![text_item("just text")],
        );

        let many_items = expand_form_items(
            &form("many", vec![children_field(vec![])]),
            &mut env,
            ExpandRuleScope::Statement,
        )
        .expect("multi root");
        assert_eq!(many_items.len(), 2);

        let text_error = expand_with_rules(
            &form("texty", vec![children_field(vec![])]),
            &mut env,
            ExpandRuleScope::Statement,
        )
        .expect_err("text root");
        assert!(
            text_error
                .to_string()
                .contains("cannot produce top-level text")
        );
    }
}
