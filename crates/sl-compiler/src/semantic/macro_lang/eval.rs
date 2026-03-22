//! Compile-time evaluator for the macro language.

use super::{CtBlock, CtStmt, CtExpr, CtValue, CtEnv, BuiltinRegistry};
use crate::semantic::expand::macro_env::MacroEnv;
use sl_core::ScriptLangError;

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
) -> Result<EvalResult, ScriptLangError> {
    let mut last_value = CtValue::Nil;

    for stmt in &block.stmts {
        let result = eval_stmt(stmt, macro_env, ct_env, builtins)?;

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
) -> Result<EvalResult, ScriptLangError> {
    match stmt {
        CtStmt::Let { name, value, .. } => {
            let val = eval_expr(value, macro_env, ct_env, builtins)?;
            ct_env.set(name.clone(), val);
            Ok(EvalResult::Value(CtValue::Nil))
        }

        CtStmt::Set { name, value, .. } => {
            let val = eval_expr(value, macro_env, ct_env, builtins)?;
            ct_env
                .update(name, val)
                .map_err(|e| ScriptLangError::Message {
                    message: e,
                })?;
            Ok(EvalResult::Value(CtValue::Nil))
        }

        CtStmt::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            let cond_val = eval_expr(cond, macro_env, ct_env, builtins)?;

            if cond_val.is_truthy() {
                eval_block(then_block, macro_env, ct_env, builtins)
            } else if let Some(else_block) = else_block {
                eval_block(else_block, macro_env, ct_env, builtins)
            } else {
                Ok(EvalResult::Value(CtValue::Nil))
            }
        }

        CtStmt::Return { value, .. } => {
            let val = eval_expr(value, macro_env, ct_env, builtins)?;
            Ok(EvalResult::Return(val))
        }

        CtStmt::Expr { expr, .. } => {
            let val = eval_expr(expr, macro_env, ct_env, builtins)?;
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
) -> Result<CtValue, ScriptLangError> {
    match expr {
        CtExpr::Literal(value) => Ok(value.clone()),

        CtExpr::Var { name, .. } => ct_env
            .get(name)
            .cloned()
            .ok_or_else(|| {
                ScriptLangError::Message {
                    message: format!("Undefined variable: {}", name),
                }
            }),

        CtExpr::BuiltinCall {
            name: func_name,
            args,
            ..
        } => {
            let builtin = builtins.get(func_name).ok_or_else(|| {
                ScriptLangError::Message {
                    message: format!("Unknown builtin function: {}", func_name),
                }
            })?;

            // Evaluate arguments
            let evaluated_args: Result<Vec<_>, _> = args
                .iter()
                .map(|arg| eval_expr(arg, macro_env, ct_env, builtins))
                .collect();
            let evaluated_args = evaluated_args?;

            // Call builtin
            builtin(&evaluated_args, macro_env, ct_env)
        }

        CtExpr::Quote { body, .. } => {
            // For now, quote just evaluates the body and expects an Ast value
            // Later this will do proper quasi-quoting
            let value = eval_expr(body, macro_env, ct_env, builtins)?;
            Ok(value)
        }

        CtExpr::Unquote { expr, .. } => {
            // Unquote should only appear inside quote, but for now we just evaluate
            eval_expr(expr, macro_env, ct_env, builtins)
        }
    }
}
