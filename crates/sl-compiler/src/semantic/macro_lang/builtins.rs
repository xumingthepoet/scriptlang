//! Compile-time builtin functions.
//!
//! Main facade. Individual builtin implementations live in the `builtins/` subdirectory.

#[path = "builtins/builtins_ast_read.rs"]
mod builtins_ast_read;
#[path = "builtins/builtins_ast_write.rs"]
mod builtins_ast_write;
#[path = "builtins/builtins_attr.rs"]
mod builtins_attr;
#[path = "builtins/builtins_keyword.rs"]
mod builtins_keyword;
#[path = "builtins/builtins_list.rs"]
mod builtins_list;
#[path = "builtins/builtins_module.rs"]
mod builtins_module;
#[path = "builtins/builtins_module_data.rs"]
mod builtins_module_data;
#[path = "builtins/builtins_registry.rs"]
mod builtins_registry;
#[path = "builtins/builtins_scalar.rs"]
mod builtins_scalar;

// Re-export for public API.
pub use builtins_registry::{BuiltinRegistry, BuiltinResult};
