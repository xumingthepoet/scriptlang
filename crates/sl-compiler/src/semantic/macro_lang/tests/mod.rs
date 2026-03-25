//! Unit tests for the compile-time macro language.
//!
//! ## Organization
//!
//! This module has been split into focused submodules to avoid a monolithic 6000+ line
//! test file:
//!
//! - [`ct_lang_tests::tests_ct_eval`](ct_lang_tests::tests_ct_eval) — Basic eval tests and core builtin tests
//! - [`ct_lang_tests::tests_convert`](ct_lang_tests::tests_convert) — Form → CtStmt/CtExpr conversion tests
//! - [`ct_lang_tests::tests_builtins`](ct_lang_tests::tests_builtins) — Advanced builtin tests (AST, module, list, keyword, match)

#[cfg(test)]
mod tests_helpers;

#[cfg(test)]
mod ct_lang_tests;
