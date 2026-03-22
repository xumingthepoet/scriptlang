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
        } else if is_macro_in_requires(form, env) {
            // Macro might be in a required module - treat as macro hook to get proper error
            ExpandDispatch::MacroHook
        } else {
            ExpandDispatch::Builtin
        }
    }

    fn has_builtin_rule(self, form: &Form, scope: ExpandRuleScope) -> bool {
        match scope {
            ExpandRuleScope::ModuleChild => matches!(form.head.as_str(), "script" | "var" | "temp"),
            ExpandRuleScope::Statement => {
                matches!(form.head.as_str(), "temp" | "while" | "choice" | "option")
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
        _ => {
            // Check if this might be a macro from a required module
            if is_macro_in_requires(form, env) {
                // Try to expand as macro - will fail with proper error
                expand_macro_hook(form, env, ExpandRuleScope::ModuleChild)
            } else {
                Ok(vec![FormItem::Form(form.clone())])
            }
        }
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
        "while" | "choice" | "option" => Ok(vec![FormItem::Form(rewrite_form_children(
            form,
            env,
            ExpandRuleScope::Statement,
        )?)]),
        _ => Ok(vec![FormItem::Form(form.clone())]),
    }
}

/// Check if a form head matches a macro in any module (required or not).
/// This is used to distinguish "not a macro" from "macro from module not in scope".
fn is_macro_in_requires(form: &Form, env: &ExpandEnv) -> bool {
    let name = &form.head;
    // Check all modules that have been loaded (via module_macros)
    // If the macro exists in any module but was NOT found by resolve_macro,
    // it means the module is not in scope
    for macros in env.program.module_macros.values() {
        if macros.contains_key(name) {
            // Macro exists in this module - if it's not found by resolve_macro,
            // the module is not in scope
            return true;
        }
    }
    false
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
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use crate::semantic::env::{ExpandEnv, MacroDefinition};

    use super::{
        ExpandDispatch, ExpandRegistry, ExpandRuleScope, expand_form_items, expand_generated_items,
        expand_with_rules, rewrite_form_children,
    };

    fn meta() -> FormMeta {
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

    fn node(head: &str, attrs: Vec<(&str, &str)>, items: Vec<FormItem>) -> Form {
        let mut fields = attrs
            .into_iter()
            .map(|(k, v)| attr(k, v))
            .collect::<Vec<_>>();
        fields.push(children(items));
        form(head, fields)
    }

    #[test]
    fn expand_rule_scope_variants_remain_distinct() {
        assert_ne!(ExpandRuleScope::ModuleChild, ExpandRuleScope::Statement);
    }

    #[test]
    fn dispatch_covers_builtin_and_macro_hook_paths() {
        let mut env = ExpandEnv::default();
        env.begin_module(Some("main".to_string()), Some("main.xml".to_string()))
            .expect("module");
        env.program
            .register_macro(MacroDefinition {
                module_name: "main".to_string(),
                name: "hello".to_string(),
                params: None,
                legacy_protocol: None,
                body: vec![child(node("quote", vec![], vec![text("hi")]))],
                is_private: false,
            })
            .expect("register macro");

        let registry = ExpandRegistry;
        assert_eq!(
            registry.dispatch(
                &node("script", vec![], vec![]),
                &env,
                ExpandRuleScope::ModuleChild
            ),
            ExpandDispatch::Builtin
        );
        assert_eq!(
            registry.dispatch(
                &node("hello", vec![], vec![]),
                &env,
                ExpandRuleScope::Statement
            ),
            ExpandDispatch::MacroHook
        );
        assert_eq!(
            registry.dispatch(
                &node("unknown", vec![], vec![]),
                &env,
                ExpandRuleScope::Statement
            ),
            ExpandDispatch::Builtin
        );
    }

    #[test]
    fn expand_helpers_cover_root_errors_and_child_rewrite() {
        let mut env = ExpandEnv::default();
        env.begin_module(Some("main".to_string()), Some("main.xml".to_string()))
            .expect("module");
        env.program
            .register_macro(MacroDefinition {
                module_name: "main".to_string(),
                name: "double".to_string(),
                params: None,
                legacy_protocol: None,
                body: vec![child(node(
                    "quote",
                    vec![],
                    vec![
                        child(node("text", vec![], vec![text("a")])),
                        child(node("text", vec![], vec![text("b")])),
                    ],
                ))],
                is_private: false,
            })
            .expect("register multi macro");
        env.program
            .register_macro(MacroDefinition {
                module_name: "main".to_string(),
                name: "stringy".to_string(),
                params: None,
                legacy_protocol: None,
                body: vec![
                    child(node(
                        "let",
                        vec![("name", "label"), ("type", "string")],
                        vec![child(node(
                            "get-attribute",
                            vec![("name", "label")],
                            vec![],
                        ))],
                    )),
                    child(node(
                        "quote",
                        vec![],
                        vec![child(node("unquote", vec![], vec![text("label")]))],
                    )),
                ],
                is_private: false,
            })
            .expect("register string macro");

        let multi_error = expand_with_rules(
            &node("double", vec![], vec![]),
            &mut env,
            ExpandRuleScope::Statement,
        )
        .expect_err("multi root");
        assert!(
            multi_error
                .to_string()
                .contains("must produce exactly one root form")
        );

        let text_error = expand_with_rules(
            &node("stringy", vec![("label", "hello")], vec![]),
            &mut env,
            ExpandRuleScope::Statement,
        )
        .expect_err("top-level text");
        assert!(
            text_error
                .to_string()
                .contains("cannot produce top-level text")
        );

        let script = node(
            "script",
            vec![("name", "main")],
            vec![child(node(
                "temp",
                vec![("name", "x"), ("type", "int")],
                vec![text("1")],
            ))],
        );
        let rewritten = rewrite_form_children(
            &script,
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect("rewrite children");
        assert_eq!(rewritten.head, "script");
        let items = expand_generated_items(
            &[
                text("hi"),
                child(node(
                    "temp",
                    vec![("name", "x"), ("type", "int")],
                    vec![text("1")],
                )),
            ],
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect("generated items");
        assert_eq!(items.len(), 2);
        let plain = expand_form_items(
            &node("noop", vec![], vec![]),
            &mut ExpandEnv::default(),
            ExpandRuleScope::Statement,
        )
        .expect("plain builtin");
        assert_eq!(plain.len(), 1);
    }
}
