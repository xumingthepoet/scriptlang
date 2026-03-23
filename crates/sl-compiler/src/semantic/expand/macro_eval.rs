use sl_core::{Form, FormItem, ScriptLangError};

use super::dispatch::ExpandRuleScope;
use super::macro_env::MacroEnv;
use super::macro_values::MacroValue;
use super::raw_body_text;
use crate::semantic::env::ExpandEnv;
use crate::semantic::error_at;
use crate::semantic::macro_lang::BuiltinRegistry;
use crate::semantic::macro_lang::convert::convert_macro_body;
use crate::semantic::macro_lang::eval::{eval_block, macro_value_to_ct_value};

/// Evaluate macro items using the NEW compile-time evaluator (Step 4).
///
/// This function:
/// 1. Converts the XML macro body to CtBlock using convert_macro_body
/// 2. Evaluates the CtBlock using eval_block (which supports new builtins like invoke_macro)
/// 3. Returns the expanded AST items
pub(crate) fn evaluate_macro_items(
    _body: &[FormItem],
    _invocation: &Form,
    env: &mut ExpandEnv,
    _scope: ExpandRuleScope,
    runtime: MacroEnv,
) -> Result<Vec<FormItem>, ScriptLangError> {
    // Use the new compile-time evaluator
    let block = convert_macro_body(_body)?;
    let builtins = BuiltinRegistry::new();

    // Pre-populate ct_env from macro params stored in macro_env.locals (MacroValue).
    // This makes macro parameters (e.g. `opts` from `keyword:opts`) accessible as
    // CtExpr::Var references in the compile-time evaluator.
    let mut ct_env = crate::semantic::macro_lang::CtEnv::new();
    for (name, mv) in &runtime.locals {
        ct_env.set(name.clone(), macro_value_to_ct_value(mv));
    }

    let result = eval_block(&block, &runtime, &mut ct_env, &builtins, env).map_err(
        |e: ScriptLangError| ScriptLangError::Message {
            message: e.to_string(),
        },
    )?;

    let value = result
        .into_value()
        .map_err(|e: ScriptLangError| ScriptLangError::Message {
            message: e.to_string(),
        })?;

    match value {
        crate::semantic::macro_lang::CtValue::Ast(items) => Ok(items),
        other => Err(ScriptLangError::message(format!(
            "macro body must return AST, got {}",
            other.type_name()
        ))),
    }
}

pub(super) fn eval_unquote(form: &Form, runtime: &MacroEnv) -> Result<MacroValue, ScriptLangError> {
    let name = raw_body_text(form)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| error_at(form, "<unquote> requires local name body"))?;
    runtime.locals.get(&name).cloned().ok_or_else(|| {
        error_at(
            form,
            format!("unknown macro local `{name}` referenced by <unquote>"),
        )
    })
}
