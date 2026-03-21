use sl_core::{Form, FormField, FormItem, FormValue, ScriptLangError};

use super::{map_child_forms, string_attr};
use crate::semantic::env::{ExpandEnv, MacroDefinition, MacroScope};
use crate::semantic::form::attr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExpandRuleScope {
    ModuleChild,
    Statement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExpandDispatch {
    Builtin,
    MacroHook,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ExpandRegistry;

pub(crate) fn expand_with_rules(
    form: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Form, ScriptLangError> {
    ExpandRegistry.expand(form, env, scope)
}

impl ExpandRegistry {
    pub(crate) fn expand(
        self,
        form: &Form,
        env: &mut ExpandEnv,
        scope: ExpandRuleScope,
    ) -> Result<Form, ScriptLangError> {
        match self.dispatch(form, env, scope) {
            ExpandDispatch::Builtin => match scope {
                ExpandRuleScope::ModuleChild => expand_module_child(form, env),
                ExpandRuleScope::Statement => expand_statement_child(form, env),
            },
            ExpandDispatch::MacroHook => expand_macro_hook(form, env, scope),
        }
    }

    fn dispatch(self, form: &Form, env: &ExpandEnv, scope: ExpandRuleScope) -> ExpandDispatch {
        if self.has_builtin_rule(form, scope) {
            ExpandDispatch::Builtin
        } else if env
            .resolve_macro(&form.head, envless_scope(scope))
            .is_some()
        {
            ExpandDispatch::MacroHook
        } else {
            ExpandDispatch::Builtin
        }
    }

    fn has_builtin_rule(self, form: &Form, scope: ExpandRuleScope) -> bool {
        match scope {
            ExpandRuleScope::ModuleChild => matches!(form.head.as_str(), "script" | "var" | "temp"),
            ExpandRuleScope::Statement => {
                matches!(form.head.as_str(), "temp" | "if" | "choice" | "option")
            }
        }
    }
}

fn expand_module_child(form: &Form, env: &mut ExpandEnv) -> Result<Form, ScriptLangError> {
    match form.head.as_str() {
        "script" => {
            env.begin_script();
            map_child_forms(form, |child| {
                expand_with_rules(child, env, ExpandRuleScope::Statement)
            })
        }
        "var" => Ok(form.clone()),
        "temp" => {
            if let Some(name) = string_attr(form, "name") {
                env.add_local(name.to_string());
            }
            Ok(form.clone())
        }
        _ => Ok(form.clone()),
    }
}

fn expand_statement_child(form: &Form, env: &mut ExpandEnv) -> Result<Form, ScriptLangError> {
    env.enter_statement();
    match form.head.as_str() {
        "temp" => {
            if let Some(name) = string_attr(form, "name") {
                env.add_local(name.to_string());
            }
            Ok(form.clone())
        }
        "if" | "choice" | "option" => map_child_forms(form, |child| {
            expand_with_rules(child, env, ExpandRuleScope::Statement)
        }),
        _ => Ok(form.clone()),
    }
}

fn expand_macro_hook(
    form: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Form, ScriptLangError> {
    let macro_scope = envless_scope(scope);
    let definition = env.resolve_macro(&form.head, macro_scope).cloned();
    match definition {
        Some(definition) => expand_macro_invocation(&definition, form, env, scope),
        None => Ok(form.clone()),
    }
}

fn envless_scope(scope: ExpandRuleScope) -> MacroScope {
    match scope {
        ExpandRuleScope::ModuleChild => MacroScope::ModuleChild,
        ExpandRuleScope::Statement => MacroScope::Statement,
    }
}

fn expand_macro_invocation(
    definition: &MacroDefinition,
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Form, ScriptLangError> {
    let expanded_items = expand_macro_items(&definition.body, invocation, env, scope)?
        .into_iter()
        .filter(|item| match item {
            FormItem::Text(text) => !text.trim().is_empty(),
            FormItem::Form(_) => true,
        })
        .collect::<Vec<_>>();
    if expanded_items.len() != 1 {
        return Err(ScriptLangError::message(format!(
            "macro `{}` must expand to exactly one root form in MVP",
            definition.name
        )));
    }
    match expanded_items.into_iter().next().expect("single item") {
        FormItem::Form(form) => Ok(form),
        FormItem::Text(_) => Err(ScriptLangError::message(format!(
            "macro `{}` cannot expand to top-level text item",
            definition.name
        ))),
    }
}

fn expand_macro_items(
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
                expanded.push(FormItem::Form(expand_macro_form(
                    node, invocation, env, scope,
                )?));
            }
        }
    }
    Ok(expanded)
}

fn expand_macro_form(
    form: &Form,
    invocation: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Form, ScriptLangError> {
    let mut fields = Vec::with_capacity(form.fields.len());
    for field in &form.fields {
        let value = match &field.value {
            FormValue::String(text) => FormValue::String(substitute_text(text, invocation)),
            FormValue::Sequence(items) => {
                FormValue::Sequence(expand_macro_items(items, invocation, env, scope)?)
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
    expand_with_rules(&expanded, env, scope)
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
