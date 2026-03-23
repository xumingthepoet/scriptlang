//! Compile-time evaluator for the macro language.

use super::{BuiltinRegistry, CtBlock, CtEnv, CtExpr, CtStmt, CtValue};
use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::dispatch::ExpandRuleScope;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::expand::macro_values::MacroValue;
use crate::semantic::expand::quote::quote_items;
use sl_core::{Form, FormMeta, ScriptLangError, SourcePosition};

/// Result of evaluation (may return early).
pub enum EvalResult {
    /// Normal completion with a value
    Value(CtValue),
    /// Early return
    Return(CtValue),
}

impl EvalResult {
    pub fn into_value(self) -> Result<CtValue, ScriptLangError> {
        match self {
            EvalResult::Value(v) => Ok(v),
            EvalResult::Return(v) => Ok(v),
        }
    }
}

/// Evaluate a compile-time block.
pub fn eval_block(
    block: &CtBlock,
    macro_env: &MacroEnv,
    ct_env: &mut CtEnv,
    builtins: &BuiltinRegistry,
    expand_env: &mut ExpandEnv,
) -> Result<EvalResult, ScriptLangError> {
    let mut last_value = CtValue::Nil;

    for stmt in &block.stmts {
        let result = eval_stmt(stmt, macro_env, ct_env, builtins, expand_env)?;

        match result {
            EvalResult::Return(_) => return Ok(result),
            EvalResult::Value(v) => last_value = v,
        }
    }

    Ok(EvalResult::Value(last_value))
}

/// Evaluate a compile-time statement.
pub fn eval_stmt(
    stmt: &CtStmt,
    macro_env: &MacroEnv,
    ct_env: &mut CtEnv,
    builtins: &BuiltinRegistry,
    expand_env: &mut ExpandEnv,
) -> Result<EvalResult, ScriptLangError> {
    match stmt {
        CtStmt::Let { name, value, .. } => {
            let val = eval_expr(value, macro_env, ct_env, builtins, expand_env)?;
            ct_env.set(name.clone(), val);
            Ok(EvalResult::Value(CtValue::Nil))
        }

        CtStmt::Set { name, value, .. } => {
            let val = eval_expr(value, macro_env, ct_env, builtins, expand_env)?;
            ct_env
                .update(name, val)
                .map_err(|e| ScriptLangError::Message { message: e })?;
            Ok(EvalResult::Value(CtValue::Nil))
        }

        CtStmt::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            let cond_val = eval_expr(cond, macro_env, ct_env, builtins, expand_env)?;

            if cond_val.is_truthy() {
                eval_block(then_block, macro_env, ct_env, builtins, expand_env)
            } else if let Some(else_block) = else_block {
                eval_block(else_block, macro_env, ct_env, builtins, expand_env)
            } else {
                Ok(EvalResult::Value(CtValue::Nil))
            }
        }

        CtStmt::Return { value, .. } => {
            let val = eval_expr(value, macro_env, ct_env, builtins, expand_env)?;
            Ok(EvalResult::Return(val))
        }

        CtStmt::Expr { expr, .. } => {
            let val = eval_expr(expr, macro_env, ct_env, builtins, expand_env)?;
            Ok(EvalResult::Value(val))
        }
    }
}

/// Evaluate a compile-time expression.
pub fn eval_expr(
    expr: &CtExpr,
    macro_env: &MacroEnv,
    ct_env: &mut CtEnv,
    builtins: &BuiltinRegistry,
    expand_env: &mut ExpandEnv,
) -> Result<CtValue, ScriptLangError> {
    match expr {
        CtExpr::Literal(value) => Ok(value.clone()),

        CtExpr::Var { name, .. } => {
            ct_env
                .get(name)
                .cloned()
                .ok_or_else(|| ScriptLangError::Message {
                    message: format!("Undefined variable: {}", name),
                })
        }

        CtExpr::BuiltinCall {
            name: func_name,
            args,
            ..
        } => {
            let builtin = builtins
                .get(func_name)
                .ok_or_else(|| ScriptLangError::Message {
                    message: format!("Unknown builtin function: {}", func_name),
                })?;

            // Evaluate arguments
            let evaluated_args: Result<Vec<_>, _> = args
                .iter()
                .map(|arg| eval_expr(arg, macro_env, ct_env, builtins, expand_env))
                .collect();
            let evaluated_args = evaluated_args?;

            // Call builtin: builtins receive &MacroEnv and &mut ExpandEnv
            // (MacroEnv is read-only, ExpandEnv is for mutations)
            builtin(&evaluated_args, macro_env, ct_env, expand_env)
        }

        CtExpr::Quote { body, .. } => {
            // Evaluate the body - quote is used for compile-time splicing
            let value = eval_expr(body, macro_env, ct_env, builtins, expand_env)?;
            Ok(value)
        }

        CtExpr::QuoteForms { items } => {
            // Process raw form items through quote_items (hygiene + splice)
            // Clone macro_env since quote_items needs &mut MacroEnv for gensym
            let mut runtime = macro_env.clone();
            // Sync CtEnv variables to MacroEnv.locals so eval_unquote can find them
            sync_ct_env_to_macro_env(ct_env, &mut runtime);
            // Create a minimal dummy invocation form for quote processing
            let invocation = Form {
                head: "$quote".to_string(),
                meta: FormMeta {
                    source_name: None,
                    start: SourcePosition { row: 0, column: 0 },
                    end: SourcePosition { row: 0, column: 0 },
                    start_byte: 0,
                    end_byte: 0,
                },
                fields: Vec::new(),
            };
            let processed = quote_items(
                &invocation,
                expand_env,
                ExpandRuleScope::Statement,
                &mut runtime,
                items,
            )?;
            Ok(CtValue::Ast(processed))
        }

        CtExpr::Unquote { expr, .. } => {
            // Unquote should only appear inside quote, but for now we just evaluate
            eval_expr(expr, macro_env, ct_env, builtins, expand_env)
        }
    }
}

// ============================================================================
// CtEnv <-> MacroEnv bridge
// ============================================================================

/// Sync CtEnv variables to MacroEnv.locals so that eval_unquote (which uses
/// MacroEnv.locals) can find variables set by the new compile-time evaluator.
fn sync_ct_env_to_macro_env(ct_env: &CtEnv, macro_env: &mut MacroEnv) {
    for (name, value) in ct_env.all() {
        macro_env
            .locals
            .insert(name.clone(), ct_value_to_macro_value(value));
    }
}

/// Convert a CtValue to a MacroValue for interoperability with eval_unquote.
pub(crate) fn ct_value_to_macro_value(ct: &CtValue) -> MacroValue {
    match ct {
        CtValue::Nil => MacroValue::Nil,
        CtValue::Bool(b) => MacroValue::Bool(*b),
        CtValue::Int(i) => MacroValue::Int(*i),
        CtValue::String(s) => MacroValue::String(s.clone()),
        CtValue::Keyword(kv) => MacroValue::Keyword(
            kv.iter()
                .map(|(k, v)| (k.clone(), ct_value_to_macro_value(v)))
                .collect(),
        ),
        CtValue::List(items) => {
            MacroValue::List(items.iter().map(ct_value_to_macro_value).collect())
        }
        CtValue::ModuleRef(m) => MacroValue::String(m.clone()),
        CtValue::Ast(items) => MacroValue::AstItems(items.clone()),
        CtValue::CallerEnv => MacroValue::String("<caller_env>".to_string()),
    }
}

/// Convert a MacroValue to a CtValue so macro parameters (stored in MacroEnv.locals)
/// are accessible as CtExpr::Var references in the compile-time evaluator.
pub(crate) fn macro_value_to_ct_value(mv: &MacroValue) -> CtValue {
    match mv {
        MacroValue::Nil => CtValue::Nil,
        MacroValue::Bool(b) => CtValue::Bool(*b),
        MacroValue::Int(i) => CtValue::Int(*i),
        MacroValue::String(s) => CtValue::String(s.clone()),
        MacroValue::Expr(s) => CtValue::String(s.clone()),
        MacroValue::AstItems(items) => CtValue::Ast(items.clone()),
        MacroValue::Keyword(kv) => CtValue::Keyword(
            kv.iter()
                .map(|(k, v)| (k.clone(), macro_value_to_ct_value(v)))
                .collect(),
        ),
        MacroValue::List(items) => {
            CtValue::List(items.iter().map(macro_value_to_ct_value).collect())
        }
    }
}
