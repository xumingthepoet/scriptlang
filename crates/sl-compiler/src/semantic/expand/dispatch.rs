use sl_core::{Form, FormField, FormItem, FormValue, ScriptLangError};

use super::macros::expand_macro_hook;
use super::string_attr;
use crate::semantic::env::ExpandEnv;

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
    let items = expand_form_items(form, env, scope)?;
    if items.len() != 1 {
        return Err(ScriptLangError::message(format!(
            "expansion of <{}> must produce exactly one root form in this position",
            form.head
        )));
    }
    match items.into_iter().next().expect("single item") {
        FormItem::Form(form) => Ok(form),
        FormItem::Text(_) => Err(ScriptLangError::message(format!(
            "expansion of <{}> cannot produce top-level text in this position",
            form.head
        ))),
    }
}

pub(crate) fn expand_form_items(
    form: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    ExpandRegistry.expand(form, env, scope)
}

impl ExpandRegistry {
    pub(crate) fn expand(
        self,
        form: &Form,
        env: &mut ExpandEnv,
        scope: ExpandRuleScope,
    ) -> Result<Vec<FormItem>, ScriptLangError> {
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
        } else if env.resolve_macro(&form.head).is_some() {
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

fn expand_module_child(form: &Form, env: &mut ExpandEnv) -> Result<Vec<FormItem>, ScriptLangError> {
    match form.head.as_str() {
        "script" => {
            env.begin_script();
            let expanded = rewrite_form_children(form, env, ExpandRuleScope::Statement)?;
            Ok(vec![FormItem::Form(expanded)])
        }
        "var" => Ok(vec![FormItem::Form(form.clone())]),
        "temp" => {
            if let Some(name) = string_attr(form, "name") {
                env.add_local(name.to_string());
            }
            Ok(vec![FormItem::Form(form.clone())])
        }
        _ => Ok(vec![FormItem::Form(form.clone())]),
    }
}

fn expand_statement_child(
    form: &Form,
    env: &mut ExpandEnv,
) -> Result<Vec<FormItem>, ScriptLangError> {
    env.enter_statement();
    match form.head.as_str() {
        "temp" => {
            if let Some(name) = string_attr(form, "name") {
                env.add_local(name.to_string());
            }
            Ok(vec![FormItem::Form(form.clone())])
        }
        "if" | "choice" | "option" => Ok(vec![FormItem::Form(rewrite_form_children(
            form,
            env,
            ExpandRuleScope::Statement,
        )?)]),
        _ => Ok(vec![FormItem::Form(form.clone())]),
    }
}

pub(super) fn expand_generated_items(
    items: &[FormItem],
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    let mut output = Vec::new();
    for item in items {
        match item {
            FormItem::Text(text) => output.push(FormItem::Text(text.clone())),
            FormItem::Form(form) => output.extend(expand_form_items(form, env, scope)?),
        }
    }
    Ok(output)
}

pub(super) fn rewrite_form_children(
    form: &Form,
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Form, ScriptLangError> {
    let mut fields = Vec::with_capacity(form.fields.len());
    for field in &form.fields {
        let mapped = match (&field.name[..], &field.value) {
            ("children", FormValue::Sequence(items)) => FormField {
                name: field.name.clone(),
                value: FormValue::Sequence(expand_sequence_items(items, env, scope)?),
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

fn expand_sequence_items(
    items: &[FormItem],
    env: &mut ExpandEnv,
    scope: ExpandRuleScope,
) -> Result<Vec<FormItem>, ScriptLangError> {
    let mut rewritten = Vec::new();
    for item in items {
        match item {
            FormItem::Text(text) => rewritten.push(FormItem::Text(text.clone())),
            FormItem::Form(child) => rewritten.extend(expand_form_items(child, env, scope)?),
        }
    }
    Ok(rewritten)
}

#[cfg(test)]
mod tests {
    use super::ExpandRuleScope;

    #[test]
    fn expand_rule_scope_variants_remain_distinct() {
        assert_ne!(ExpandRuleScope::ModuleChild, ExpandRuleScope::Statement);
    }
}
