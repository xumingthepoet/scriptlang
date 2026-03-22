//! Compile-time macro language infrastructure.
//!
//! This module implements a real compile-time language for macro bodies,
//! replacing the old template-based approach.

mod ast;
mod builtins;
pub(crate) mod convert;
mod env;
pub(crate) mod eval;
mod values;

#[cfg(test)]
mod tests;

pub use ast::*;
pub use builtins::BuiltinRegistry;
#[allow(unused_imports)]
pub use convert::convert_macro_body;
pub use env::CtEnv;
#[allow(unused_imports)]
pub use eval::{EvalResult, eval_block};
