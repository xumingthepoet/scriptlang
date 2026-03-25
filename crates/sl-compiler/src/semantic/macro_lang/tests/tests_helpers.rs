//! Shared test helpers for the macro_lang test suite.
//!
//! This module provides `use` imports and utility functions shared by
//! all test submodules in `ct_lang_tests`.

#![allow(unused_imports)]

use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::macro_env::MacroEnv;
// macro_lang::* is intentionally not imported: CtBlock/CtStmt are accessed via test file imports

pub fn empty_macro_env() -> MacroEnv {
    MacroEnv::default()
}

pub fn empty_expand_env() -> ExpandEnv {
    ExpandEnv::default()
}
