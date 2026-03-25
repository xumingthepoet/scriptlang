//! All test functions for the compile-time macro language.
//!
//! The actual test implementations are in sibling files to keep each file under the
//! 400-line guideline. Shared helper functions are provided by the sibling
//! [`super::tests_helpers`] module.

#![allow(unused_imports)]

pub use super::tests_helpers::{empty_expand_env, empty_macro_env};

#[path = "tests_builtins.rs"]
mod tests_builtins;
#[path = "tests_convert.rs"]
mod tests_convert;
#[path = "tests_ct_eval.rs"]
mod tests_ct_eval;
