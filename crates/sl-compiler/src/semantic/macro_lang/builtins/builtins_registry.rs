//! Compile-time builtin function registry and common types.

use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::macro_lang::CtEnv;
use crate::semantic::macro_lang::CtValue;
use sl_core::ScriptLangError;
use std::collections::HashMap;

/// Result of a builtin function call.
pub type BuiltinResult = Result<CtValue, ScriptLangError>;

/// A compile-time builtin function.
/// Builtins receive:
/// - `&mut MacroEnv`: mutable access to caller context (needed for list_map/list_foreach/list_fold callbacks)
/// - `&mut CtEnv`: mutable local variable bindings
/// - `&mut ExpandEnv`: mutable module state (for require/import/alias/invoke operations)
/// - `&BuiltinRegistry`: access to other builtins (needed for list_map/list_foreach/list_fold callbacks)
pub type BuiltinFn =
    fn(&[CtValue], &mut MacroEnv, &mut CtEnv, &mut ExpandEnv, &BuiltinRegistry) -> BuiltinResult;

/// Registry of compile-time builtin functions.
pub struct BuiltinRegistry {
    builtins: HashMap<String, BuiltinFn>,
}

impl BuiltinRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            builtins: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    /// Register default builtins.
    fn register_defaults(&mut self) {
        // Attr builtins
        self.register("attr", super::builtins_attr::builtin_attr);
        self.register("content", super::builtins_attr::builtin_content);
        self.register("has_attr", super::builtins_attr::builtin_has_attr);

        // Scalar builtins
        self.register("list_length", super::builtins_scalar::builtin_list_length);
        self.register("to_string", super::builtins_scalar::builtin_to_string);
        self.register("parse_bool", super::builtins_scalar::builtin_parse_bool);
        self.register("parse_int", super::builtins_scalar::builtin_parse_int);

        // Keyword builtins
        self.register("keyword_get", super::builtins_keyword::builtin_keyword_get);
        self.register(
            "keyword_attr",
            super::builtins_keyword::builtin_keyword_attr,
        );
        self.register("keyword_has", super::builtins_keyword::builtin_keyword_has);
        self.register(
            "keyword_keys",
            super::builtins_keyword::builtin_keyword_keys,
        );
        self.register(
            "keyword_values",
            super::builtins_keyword::builtin_keyword_values,
        );
        self.register(
            "keyword_pairs",
            super::builtins_keyword::builtin_keyword_pairs,
        );

        // Module operation builtins
        self.register("caller_env", super::builtins_module::builtin_caller_env);
        self.register(
            "caller_module",
            super::builtins_module::builtin_caller_module,
        );
        self.register("expand_alias", super::builtins_module::builtin_expand_alias);
        self.register(
            "require_module",
            super::builtins_module::builtin_require_module,
        );
        self.register(
            "define_import",
            super::builtins_module::builtin_define_import,
        );
        self.register("define_alias", super::builtins_module::builtin_define_alias);
        self.register(
            "define_require",
            super::builtins_module::builtin_define_require,
        );
        self.register("invoke_macro", super::builtins_module::builtin_invoke_macro);

        // AST read builtins
        self.register("ast_head", super::builtins_ast_read::builtin_ast_head);
        self.register(
            "ast_children",
            super::builtins_ast_read::builtin_ast_children,
        );
        self.register(
            "ast_attr_get",
            super::builtins_ast_read::builtin_ast_attr_get,
        );
        self.register(
            "ast_attr_keys",
            super::builtins_ast_read::builtin_ast_attr_keys,
        );

        // AST write builtins
        self.register(
            "ast_attr_set",
            super::builtins_ast_write::builtin_ast_attr_set,
        );
        self.register("ast_wrap", super::builtins_ast_write::builtin_ast_wrap);
        self.register("ast_concat", super::builtins_ast_write::builtin_ast_concat);
        self.register(
            "ast_filter_head",
            super::builtins_ast_write::builtin_ast_filter_head,
        );

        // Module data builtins
        self.register(
            "module_get",
            super::builtins_module_data::builtin_module_get,
        );
        self.register(
            "module_put",
            super::builtins_module_data::builtin_module_put,
        );
        self.register(
            "module_update",
            super::builtins_module_data::builtin_module_update,
        );

        // List builtins
        self.register("list", super::builtins_list::builtin_list);
        self.register("list_concat", super::builtins_list::builtin_list_concat);
        self.register("list_foreach", super::builtins_list::builtin_list_foreach);
        self.register("list_map", super::builtins_list::builtin_list_map);
        self.register("list_fold", super::builtins_list::builtin_list_fold);
        self.register("match", super::builtins_list::builtin_match);
    }

    /// Register a builtin function.
    pub fn register(&mut self, name: &str, func: BuiltinFn) {
        self.builtins.insert(name.to_string(), func);
    }

    /// Get a builtin function by name.
    pub fn get(&self, name: &str) -> Option<BuiltinFn> {
        self.builtins.get(name).copied()
    }
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}
