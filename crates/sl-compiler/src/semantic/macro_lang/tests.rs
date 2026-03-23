//! Unit tests for the compile-time macro language.

#[cfg(test)]
mod ct_lang_tests {
    use crate::semantic::env::ExpandEnv;
    use crate::semantic::expand::macro_env::MacroEnv;
    use crate::semantic::macro_lang::*;
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    fn empty_macro_env() -> MacroEnv {
        MacroEnv::default()
    }

    fn empty_expand_env() -> ExpandEnv {
        ExpandEnv::default()
    }

    #[test]
    fn compile_time_if_selects_correct_branch() {
        let block = CtBlock {
            stmts: vec![CtStmt::If {
                cond: CtExpr::Literal(CtValue::Bool(true)),
                then_block: CtBlock {
                    stmts: vec![CtStmt::Return {
                        value: CtExpr::Literal(CtValue::String("yes".to_string())),
                    }],
                },
                else_block: Some(CtBlock {
                    stmts: vec![CtStmt::Return {
                        value: CtExpr::Literal(CtValue::String("no".to_string())),
                    }],
                }),
            }],
        };

        let macro_env = empty_macro_env();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins, &mut expand_env)
            .expect("eval should succeed")
            .into_value()
            .expect("should have value");

        assert_eq!(result, CtValue::String("yes".to_string()));
    }

    #[test]
    fn compile_time_if_without_else_returns_nil() {
        let block = CtBlock {
            stmts: vec![CtStmt::If {
                cond: CtExpr::Literal(CtValue::Bool(false)),
                then_block: CtBlock {
                    stmts: vec![CtStmt::Return {
                        value: CtExpr::Literal(CtValue::String("yes".to_string())),
                    }],
                },
                else_block: None,
            }],
        };

        let macro_env = empty_macro_env();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins, &mut expand_env)
            .expect("eval should succeed")
            .into_value()
            .expect("should have value");

        assert_eq!(result, CtValue::Nil);
    }

    #[test]
    fn let_and_set_bindings_with_scoping() {
        let block = CtBlock {
            stmts: vec![
                CtStmt::Let {
                    name: "x".to_string(),
                    value: CtExpr::Literal(CtValue::Int(1)),
                },
                CtStmt::Set {
                    name: "x".to_string(),
                    value: CtExpr::Literal(CtValue::Int(2)),
                },
                CtStmt::Return {
                    value: CtExpr::Var {
                        name: "x".to_string(),
                    },
                },
            ],
        };

        let macro_env = empty_macro_env();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins, &mut expand_env)
            .expect("eval should succeed")
            .into_value()
            .expect("should have value");

        assert_eq!(result, CtValue::Int(2));
    }

    #[test]
    fn set_undefined_variable_errors() {
        let block = CtBlock {
            stmts: vec![CtStmt::Set {
                name: "undefined".to_string(),
                value: CtExpr::Literal(CtValue::Int(1)),
            }],
        };

        let macro_env = empty_macro_env();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins, &mut expand_env);
        assert!(result.is_err());
    }

    #[test]
    fn return_exits_early() {
        let block = CtBlock {
            stmts: vec![
                CtStmt::Return {
                    value: CtExpr::Literal(CtValue::String("early".to_string())),
                },
                CtStmt::Return {
                    value: CtExpr::Literal(CtValue::String("never reached".to_string())),
                },
            ],
        };

        let macro_env = empty_macro_env();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins, &mut expand_env)
            .expect("eval should succeed")
            .into_value()
            .expect("should have value");

        assert_eq!(result, CtValue::String("early".to_string()));
    }

    #[test]
    fn keyword_preserves_order() {
        let keyword = CtValue::Keyword(vec![
            ("first".to_string(), CtValue::Int(1)),
            ("second".to_string(), CtValue::Int(2)),
            ("third".to_string(), CtValue::Int(3)),
        ]);

        if let CtValue::Keyword(kv) = keyword {
            assert_eq!(kv[0].0, "first");
            assert_eq!(kv[1].0, "second");
            assert_eq!(kv[2].0, "third");
            assert_eq!(kv.len(), 3);
        } else {
            panic!("Expected keyword value");
        }
    }

    #[test]
    fn value_truthiness() {
        // Nil and zero/empty are falsy
        assert!(!CtValue::Nil.is_truthy());
        assert!(!CtValue::Bool(false).is_truthy());
        assert!(!CtValue::Int(0).is_truthy());
        assert!(!CtValue::String("".to_string()).is_truthy());
        assert!(!CtValue::List(vec![]).is_truthy());
        assert!(!CtValue::Keyword(vec![]).is_truthy());

        // Non-empty and true are truthy
        assert!(CtValue::Bool(true).is_truthy());
        assert!(CtValue::Int(1).is_truthy());
        assert!(CtValue::String("x".to_string()).is_truthy());
        assert!(CtValue::List(vec![CtValue::Nil]).is_truthy());
        assert!(CtValue::Keyword(vec![("k".to_string(), CtValue::Nil)]).is_truthy());
        assert!(CtValue::ModuleRef("m".to_string()).is_truthy());
    }

    #[test]
    fn type_name_reports_correct_types() {
        assert_eq!(CtValue::Nil.type_name(), "nil");
        assert_eq!(CtValue::Bool(true).type_name(), "bool");
        assert_eq!(CtValue::Int(1).type_name(), "int");
        assert_eq!(CtValue::String("x".to_string()).type_name(), "string");
        assert_eq!(CtValue::Keyword(vec![]).type_name(), "keyword");
        assert_eq!(CtValue::List(vec![]).type_name(), "list");
        assert_eq!(CtValue::ModuleRef("m".to_string()).type_name(), "module");
        assert_eq!(CtValue::Ast(vec![]).type_name(), "ast");
        assert_eq!(CtValue::CallerEnv.type_name(), "caller_env");
    }

    #[test]
    fn nested_if_conditions() {
        let block = CtBlock {
            stmts: vec![CtStmt::If {
                cond: CtExpr::Literal(CtValue::Bool(true)),
                then_block: CtBlock {
                    stmts: vec![CtStmt::If {
                        cond: CtExpr::Literal(CtValue::Bool(false)),
                        then_block: CtBlock {
                            stmts: vec![CtStmt::Return {
                                value: CtExpr::Literal(CtValue::String("inner".to_string())),
                            }],
                        },
                        else_block: Some(CtBlock {
                            stmts: vec![CtStmt::Return {
                                value: CtExpr::Literal(CtValue::String("inner-else".to_string())),
                            }],
                        }),
                    }],
                },
                else_block: Some(CtBlock {
                    stmts: vec![CtStmt::Return {
                        value: CtExpr::Literal(CtValue::String("outer-else".to_string())),
                    }],
                }),
            }],
        };

        let macro_env = empty_macro_env();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins, &mut expand_env)
            .expect("eval should succeed")
            .into_value()
            .expect("should have value");

        assert_eq!(result, CtValue::String("inner-else".to_string()));
    }

    #[test]
    fn builtin_attr_works() {
        let mut macro_env = MacroEnv::default();
        macro_env
            .attributes
            .insert("name".to_string(), "test".to_string());
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = builtins.get("attr").expect("attr builtin exists")(
            &[CtValue::String("name".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("attr should succeed");
        assert_eq!(result, CtValue::String("test".to_string()));

        // Error: wrong arg count
        let err = builtins.get("attr").unwrap()(&[], &macro_env, &mut ct_env, &mut expand_env)
            .expect_err("wrong arg count");
        assert!(err.to_string().contains("requires exactly 1 argument"));

        // Error: wrong type
        let err = builtins.get("attr").unwrap()(
            &[CtValue::Int(1)],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong type");
        assert!(err.to_string().contains("must be string"));

        // Error: missing attribute
        let err = builtins.get("attr").unwrap()(
            &[CtValue::String("missing".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("missing attr");
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn builtin_has_attr_works() {
        let mut macro_env = MacroEnv::default();
        macro_env
            .attributes
            .insert("exists".to_string(), "value".to_string());
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = builtins.get("has_attr").expect("has_attr exists")(
            &[CtValue::String("exists".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("has_attr should succeed");
        assert_eq!(result, CtValue::Bool(true));

        let result = builtins.get("has_attr").unwrap()(
            &[CtValue::String("missing".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("has_attr missing");
        assert_eq!(result, CtValue::Bool(false));

        // Error: wrong arg count
        let err = builtins.get("has_attr").unwrap()(&[], &macro_env, &mut ct_env, &mut expand_env)
            .expect_err("wrong arg count");
        assert!(err.to_string().contains("requires exactly 1 argument"));
    }

    #[test]
    fn builtin_parse_bool_and_int_work() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        // parse_bool
        let result = builtins.get("parse_bool").expect("parse_bool exists")(
            &[CtValue::String("true".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("parse_bool true");
        assert_eq!(result, CtValue::Bool(true));

        let result = builtins.get("parse_bool").unwrap()(
            &[CtValue::String("false".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("parse_bool false");
        assert_eq!(result, CtValue::Bool(false));

        let err = builtins.get("parse_bool").unwrap()(
            &[CtValue::String("invalid".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("parse_bool invalid");
        assert!(err.to_string().contains("cannot parse"));

        // parse_int
        let result = builtins.get("parse_int").expect("parse_int exists")(
            &[CtValue::String("42".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("parse_int 42");
        assert_eq!(result, CtValue::Int(42));

        let err = builtins.get("parse_int").unwrap()(
            &[CtValue::String("abc".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("parse_int invalid");
        assert!(err.to_string().contains("cannot parse"));
    }

    #[test]
    fn builtin_keyword_and_list_operations() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let keyword = CtValue::Keyword(vec![
            ("name".to_string(), CtValue::String("test".to_string())),
            ("count".to_string(), CtValue::Int(5)),
        ]);

        // keyword_get
        let result = builtins.get("keyword_get").expect("keyword_get exists")(
            &[keyword.clone(), CtValue::String("name".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("keyword_get");
        assert_eq!(result, CtValue::String("test".to_string()));

        let err = builtins.get("keyword_get").unwrap()(
            &[keyword.clone(), CtValue::String("missing".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("keyword_get missing");
        assert!(err.to_string().contains("not found"));

        // keyword_has
        let result = builtins.get("keyword_has").expect("keyword_has exists")(
            &[keyword.clone(), CtValue::String("name".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("keyword_has");
        assert_eq!(result, CtValue::Bool(true));

        // list_length
        let result = builtins.get("list_length").expect("list_length exists")(
            &[keyword],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("list_length keyword");
        assert_eq!(result, CtValue::Int(2));

        let list = CtValue::List(vec![CtValue::Nil, CtValue::Nil, CtValue::Nil]);
        let result =
            builtins.get("list_length").unwrap()(&[list], &macro_env, &mut ct_env, &mut expand_env)
                .expect("list_length list");
        assert_eq!(result, CtValue::Int(3));
    }

    #[test]
    fn builtin_to_string_works() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = builtins.get("to_string").expect("to_string exists")(
            &[CtValue::Bool(true)],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("to_string bool");
        assert_eq!(result, CtValue::String("true".to_string()));

        let result = builtins.get("to_string").unwrap()(
            &[CtValue::Int(123)],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("to_string int");
        assert_eq!(result, CtValue::String("123".to_string()));

        let result = builtins.get("to_string").unwrap()(
            &[CtValue::Nil],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("to_string nil");
        assert_eq!(result, CtValue::String("nil".to_string()));
    }

    #[test]
    fn builtin_content_works() {
        use sl_core::FormItem;

        let macro_env = MacroEnv {
            content: vec![FormItem::Text("test content".to_string())],
            ..Default::default()
        };
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = builtins.get("content").expect("content exists")(
            &[],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("content");
        assert!(matches!(result, CtValue::Ast(items) if items.len() == 1));

        // Error: too many args
        let err = builtins.get("content").unwrap()(
            &[CtValue::Nil, CtValue::Nil],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("too many args");
        assert!(err.to_string().contains("at most 1 argument"));
    }

    // ========================================================================
    // Step 4: New builtin tests
    // ========================================================================

    #[test]
    fn builtin_caller_env_returns_current_context() {
        let mut macro_env = MacroEnv {
            current_module: Some("main".to_string()),
            ..Default::default()
        };
        macro_env.imports.push("kernel".to_string());
        macro_env.requires.push("helper".to_string());
        macro_env
            .aliases
            .insert("h".to_string(), "helper".to_string());

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = builtins.get("caller_env").expect("caller_env exists")(
            &[],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("caller_env should succeed");

        match result {
            CtValue::Keyword(items) => {
                let map: std::collections::BTreeMap<_, _> = items.into_iter().collect();
                assert_eq!(
                    map.get("current_module").map(|v| v.type_name()),
                    Some("string")
                );
                assert!(matches!(map.get("imports"), Some(CtValue::List(_))));
                assert!(matches!(map.get("requires"), Some(CtValue::List(_))));
                assert!(matches!(map.get("aliases"), Some(CtValue::List(_))));
            }
            other => panic!("expected keyword, got {}", other.type_name()),
        }

        // Error: too many args
        let err = builtins.get("caller_env").unwrap()(
            &[CtValue::Nil],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("too many args");
        assert!(err.to_string().contains("takes no arguments"));
    }

    #[test]
    fn builtin_expand_alias_resolves_alias_or_returns_as_is() {
        let mut macro_env = MacroEnv::default();
        macro_env
            .aliases
            .insert("h".to_string(), "helper".to_string());
        macro_env
            .aliases
            .insert("mh".to_string(), "main.helper".to_string());

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        // Resolve alias
        let result = builtins.get("expand_alias").expect("expand_alias exists")(
            &[CtValue::String("h".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("expand alias");
        assert_eq!(result, CtValue::String("helper".to_string()));

        // Unknown name returns as-is
        let result = builtins.get("expand_alias").unwrap()(
            &[CtValue::String("unknown".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("unknown alias");
        assert_eq!(result, CtValue::String("unknown".to_string()));

        // ModuleRef also works
        let result = builtins.get("expand_alias").unwrap()(
            &[CtValue::ModuleRef("mh".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("module ref");
        assert_eq!(result, CtValue::String("main.helper".to_string()));

        // Error: wrong arg count
        let err =
            builtins.get("expand_alias").unwrap()(&[], &macro_env, &mut ct_env, &mut expand_env)
                .expect_err("wrong args");
        assert!(err.to_string().contains("requires exactly 1"));
    }

    #[test]
    fn builtin_require_module_adds_to_expand_env() {
        let macro_env = MacroEnv {
            current_module: Some("main".to_string()),
            ..Default::default()
        };

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();

        // Use a fresh expand_env each time
        let mut expand_env = empty_expand_env();

        let result = builtins
            .get("require_module")
            .expect("require_module exists")(
            &[CtValue::String("helper".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("require_module");
        // Returns the expanded module name (helper has no alias, so returns "helper")
        assert_eq!(result, CtValue::String("helper".to_string()));

        // Verify it was added
        assert!(expand_env.module.requires.contains(&"helper".to_string()));

        // Idempotent: calling again doesn't panic
        let result = builtins.get("require_module").unwrap()(
            &[CtValue::String("helper".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("require_module idempotent");
        // Returns the expanded name even when already required
        assert_eq!(result, CtValue::String("helper".to_string()));
    }

    #[test]
    fn builtin_define_import_adds_to_expand_env() {
        let mut macro_env = MacroEnv::default();

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = builtins.get("define_import").expect("define_import exists")(
            &[CtValue::String("kernel".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("define_import");
        assert_eq!(result, CtValue::Nil);
        assert!(expand_env.module.imports.contains(&"kernel".to_string()));

        // With alias
        macro_env
            .aliases
            .insert("k".to_string(), "kernel".to_string());
        let mut expand_env2 = empty_expand_env();
        let result = builtins.get("define_import").unwrap()(
            &[CtValue::String("k".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env2,
        )
        .expect("define_import with alias");
        assert_eq!(result, CtValue::Nil);
        assert!(expand_env2.module.imports.contains(&"kernel".to_string()));
    }

    #[test]
    fn builtin_define_alias_adds_alias() {
        let macro_env = MacroEnv::default();

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = builtins.get("define_alias").expect("define_alias exists")(
            &[
                CtValue::String("helper".to_string()),
                CtValue::String("h".to_string()),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("define_alias");
        assert_eq!(result, CtValue::Nil);
        assert_eq!(
            expand_env.module.aliases.get("h").map(String::as_str),
            Some("helper")
        );
    }

    #[test]
    fn builtin_define_require_adds_require() {
        let macro_env = MacroEnv::default();

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = builtins
            .get("define_require")
            .expect("define_require exists")(
            &[CtValue::String("helper".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("define_require");
        assert_eq!(result, CtValue::Nil);
        assert!(expand_env.module.requires.contains(&"helper".to_string()));
    }

    #[test]
    fn builtin_invoke_macro_rejects_unrequired_module() {
        let macro_env = MacroEnv {
            current_module: Some("main".to_string()),
            ..Default::default()
        };
        // Note: "helper" is NOT in requires

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("invoke_macro").expect("invoke_macro exists")(
            &[
                CtValue::String("helper".to_string()),
                CtValue::String("some_macro".to_string()),
                CtValue::Keyword(vec![]),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("should fail");
        assert!(err.to_string().contains("not in scope"));
        assert!(err.to_string().contains("requires"));
    }

    #[test]
    fn builtin_invoke_macro_rejects_unknown_macro() {
        let mut macro_env = MacroEnv {
            current_module: Some("main".to_string()),
            ..Default::default()
        };
        macro_env.requires.push("helper".to_string());

        // Register the helper module in expand_env
        let mut expand_env = empty_expand_env();
        expand_env
            .begin_module(Some("helper".to_string()), Some("helper.xml".to_string()))
            .expect("helper module");
        // Note: no macros registered in helper

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();

        let err = builtins.get("invoke_macro").unwrap()(
            &[
                CtValue::String("helper".to_string()),
                CtValue::String("nonexistent".to_string()),
                CtValue::Keyword(vec![]),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("should fail");
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn builtin_invoke_macro_calls_registered_macro() {
        use sl_core::{FormField, FormMeta, FormValue, SourcePosition};

        let mut macro_env = MacroEnv {
            current_module: Some("main".to_string()),
            ..Default::default()
        };
        macro_env.requires.push("helper".to_string());

        // Register helper module with a macro
        let mut expand_env = empty_expand_env();
        expand_env
            .begin_module(Some("helper".to_string()), Some("helper.xml".to_string()))
            .expect("helper module");

        // Build a simple macro body: <quote><text>result</text></quote>
        let quote_meta = FormMeta {
            source_name: Some("helper.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Helper to build Form structures cleanly
        fn make_form(meta: &FormMeta, head: &str, fields: Vec<FormField>) -> sl_core::Form {
            sl_core::Form {
                head: head.to_string(),
                meta: meta.clone(),
                fields,
            }
        }
        fn make_field(name: &str, value: FormValue) -> FormField {
            FormField {
                name: name.to_string(),
                value,
            }
        }
        fn make_seq(items: Vec<sl_core::FormItem>) -> FormValue {
            FormValue::Sequence(items)
        }
        fn make_text(s: &str) -> sl_core::FormItem {
            sl_core::FormItem::Text(s.to_string())
        }
        fn make_form_item(
            meta: &FormMeta,
            head: &str,
            fields: Vec<FormField>,
        ) -> sl_core::FormItem {
            sl_core::FormItem::Form(make_form(meta, head, fields))
        }
        let text_child = make_form_item(
            &quote_meta,
            "text",
            vec![make_field(
                "children",
                make_seq(vec![make_text("macro_result")]),
            )],
        );
        let macro_body = vec![make_form_item(
            &quote_meta,
            "quote",
            vec![make_field("children", make_seq(vec![text_child]))],
        )];

        expand_env
            .program
            .register_macro(crate::semantic::env::MacroDefinition {
                module_name: "helper".to_string(),
                name: "__mk__".to_string(),
                params: Some(vec![crate::semantic::env::MacroParam {
                    param_type: crate::semantic::env::MacroParamType::Expr,
                    name: "label".to_string(),
                }]),
                legacy_protocol: None,
                body: macro_body,
                is_private: false,
            })
            .expect("register macro");

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();

        let result = builtins.get("invoke_macro").unwrap()(
            &[
                CtValue::String("helper".to_string()),
                CtValue::String("__mk__".to_string()),
                CtValue::Keyword(vec![(
                    "label".to_string(),
                    CtValue::String("test".to_string()),
                )]),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("invoke_macro should succeed");

        match result {
            CtValue::Ast(items) => {
                assert!(!items.is_empty(), "should produce AST items");
            }
            other => panic!("expected Ast, got {}", other.type_name()),
        }
    }

    // ========================================================================
    // Convert tests - cover uncovered paths in convert.rs
    // ========================================================================

    #[test]
    fn convert_if_with_else_block() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Build: <if><get-attr name="x"/><then><quote><text></text></quote></then><else><quote><text></text></quote></else></if>
        // Note: <quote> forms inside then/else will be processed recursively
        let if_form = Form {
            head: "if".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![
                    FormItem::Form(Form {
                        head: "get-attribute".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "name".to_string(),
                            value: FormValue::String("x".to_string()),
                        }],
                    }),
                    FormItem::Form(Form {
                        head: "then".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Form(Form {
                                head: "quote".to_string(),
                                meta: meta.clone(),
                                fields: vec![FormField {
                                    name: "children".to_string(),
                                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                                        head: "text".to_string(),
                                        meta: meta.clone(),
                                        fields: vec![],
                                    })]),
                                }],
                            })]),
                        }],
                    }),
                    FormItem::Form(Form {
                        head: "else".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Form(Form {
                                head: "quote".to_string(),
                                meta: meta.clone(),
                                fields: vec![FormField {
                                    name: "children".to_string(),
                                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                                        head: "text".to_string(),
                                        meta: meta.clone(),
                                        fields: vec![],
                                    })]),
                                }],
                            })]),
                        }],
                    }),
                ]),
            }],
        };

        let result =
            convert_macro_body(&[FormItem::Form(if_form)]).expect("convert should succeed");
        assert_eq!(result.stmts.len(), 1);
        // The if should have both then and else blocks
        match &result.stmts[0] {
            CtStmt::If {
                then_block,
                else_block,
                ..
            } => {
                assert!(!then_block.stmts.is_empty());
                assert!(else_block.is_some(), "else block should be present");
            }
            other => panic!("expected If, got {:?}", other),
        }
    }

    #[test]
    fn convert_return_with_expr() {
        use crate::semantic::macro_lang::convert::convert_macro_body;
        use sl_core::{Form, FormField, FormMeta, FormValue, SourcePosition};

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Build: <return><get-attr name="x"/></return>
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "get-attribute".to_string(),
                    meta: meta.clone(),
                    fields: vec![FormField {
                        name: "name".to_string(),
                        value: FormValue::String("x".to_string()),
                    }],
                })]),
            }],
        };

        let result =
            convert_macro_body(&[FormItem::Form(return_form)]).expect("convert should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value, .. } => {
                // The value should be a builtin call (attr)
                assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "attr"));
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_let_with_keyword_type() {
        use crate::semantic::macro_lang::convert::convert_macro_body;
        use sl_core::{Form, FormField, FormMeta, FormValue, SourcePosition};

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Build: <let name="opts" type="keyword"><get-attr name="opts"/></let>
        let let_form = Form {
            head: "let".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("opts".to_string()),
                },
                FormField {
                    name: "type".to_string(),
                    value: FormValue::String("keyword".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                        head: "get-attribute".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "name".to_string(),
                            value: FormValue::String("opts".to_string()),
                        }],
                    })]),
                },
            ],
        };

        let result =
            convert_macro_body(&[FormItem::Form(let_form)]).expect("convert should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Let { name, value, .. } => {
                assert_eq!(name, "opts");
                // Should use keyword_attr builtin
                assert!(
                    matches!(value, CtExpr::BuiltinCall { name, .. } if name == "keyword_attr")
                );
            }
            other => panic!("expected Let, got {:?}", other),
        }
    }

    #[test]
    fn convert_let_with_quote_provider() {
        use crate::semantic::macro_lang::convert::convert_macro_body;
        use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Build: <let name="ast" type="ast"><quote><text>hello</text></quote></let>
        let let_form = Form {
            head: "let".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("ast".to_string()),
                },
                FormField {
                    name: "type".to_string(),
                    value: FormValue::String("ast".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                        head: "quote".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Text("hello".to_string())]),
                        }],
                    })]),
                },
            ],
        };

        let result =
            convert_macro_body(&[FormItem::Form(let_form)]).expect("convert should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Let { name, value, .. } => {
                assert_eq!(name, "ast");
                // Should be QuoteForms
                assert!(matches!(value, CtExpr::QuoteForms { .. }));
            }
            other => panic!("expected Let, got {:?}", other),
        }
    }

    #[test]
    fn convert_expr_caller_module() {
        use crate::semantic::macro_lang::convert::convert_macro_body;
        use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Build: <return><caller_module/></return>
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "caller_module".to_string(),
                    meta: meta.clone(),
                    fields: vec![],
                })]),
            }],
        };

        let result =
            convert_macro_body(&[FormItem::Form(return_form)]).expect("convert should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value, .. } => {
                // Should be a builtin call to caller_module
                assert!(
                    matches!(value, CtExpr::BuiltinCall { name, .. } if name == "caller_module")
                );
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_if_wrong_second_child_type_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Build: <if><get-attr name="x"/><wrong-tag><quote><text></text></quote></wrong-tag></if>
        // Note: <wrong-tag> is the second child, not <then>, so it should error
        let if_form = Form {
            head: "if".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![
                    FormItem::Form(Form {
                        head: "get-attribute".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "name".to_string(),
                            value: FormValue::String("x".to_string()),
                        }],
                    }),
                    FormItem::Form(Form {
                        head: "wrong".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Form(Form {
                                head: "quote".to_string(),
                                meta: meta.clone(),
                                fields: vec![FormField {
                                    name: "children".to_string(),
                                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                                        head: "text".to_string(),
                                        meta: meta.clone(),
                                        fields: vec![],
                                    })]),
                                }],
                            })]),
                        }],
                    }),
                ]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(if_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("<if> second child must be <then> block")
        );
    }

    #[test]
    fn convert_if_wrong_third_child_type_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Build: <if><get-attr name="x"/><then><quote><text></text></quote></then><wrong><quote><text></text></quote></wrong></if>
        // Note: <then> needs children, <wrong> should be the 3rd child (not <else>)
        let if_form = Form {
            head: "if".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![
                    FormItem::Form(Form {
                        head: "get-attribute".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "name".to_string(),
                            value: FormValue::String("x".to_string()),
                        }],
                    }),
                    FormItem::Form(Form {
                        head: "then".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Form(Form {
                                head: "quote".to_string(),
                                meta: meta.clone(),
                                fields: vec![FormField {
                                    name: "children".to_string(),
                                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                                        head: "text".to_string(),
                                        meta: meta.clone(),
                                        fields: vec![],
                                    })]),
                                }],
                            })]),
                        }],
                    }),
                    FormItem::Form(Form {
                        head: "wrong".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Form(Form {
                                head: "quote".to_string(),
                                meta: meta.clone(),
                                fields: vec![FormField {
                                    name: "children".to_string(),
                                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                                        head: "text".to_string(),
                                        meta: meta.clone(),
                                        fields: vec![],
                                    })]),
                                }],
                            })]),
                        }],
                    }),
                ]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(if_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("<if> third child must be <else> block")
        );
    }

    #[test]
    fn convert_unsupported_let_type_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;
        use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Build: <let name="x" type="float"><get-attr name="x"/></let>
        let let_form = Form {
            head: "let".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("x".to_string()),
                },
                FormField {
                    name: "type".to_string(),
                    value: FormValue::String("float".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                        head: "get-attribute".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "name".to_string(),
                            value: FormValue::String("x".to_string()),
                        }],
                    })]),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(let_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("unsupported macro let type"));
    }

    #[test]
    fn convert_get_content_with_wrong_type_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;
        use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Build: <let name="x" type="string"><get-content/></let>
        let let_form = Form {
            head: "let".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("x".to_string()),
                },
                FormField {
                    name: "type".to_string(),
                    value: FormValue::String("string".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                        head: "get-content".to_string(),
                        meta: meta.clone(),
                        fields: vec![],
                    })]),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(let_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("<get-content> provider requires type")
        );
    }

    #[test]
    fn convert_quote_provider_wrong_type_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;
        use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Build: <let name="x" type="string"><quote><text>hi</text></quote></let>
        let let_form = Form {
            head: "let".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("x".to_string()),
                },
                FormField {
                    name: "type".to_string(),
                    value: FormValue::String("string".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                        head: "quote".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Text("hi".to_string())]),
                        }],
                    })]),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(let_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("<quote> provider requires type"));
    }

    // ========================================================================
    // Additional convert.rs path coverage
    // ========================================================================

    #[test]
    fn convert_macro_body_empty_body() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        // Empty body should produce empty block
        let result = convert_macro_body(&[]).expect("empty body should succeed");
        assert!(result.stmts.is_empty());
    }

    #[test]
    fn convert_macro_body_non_empty_text_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let result = convert_macro_body(&[FormItem::Text("not empty".to_string())]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unexpected top-level text")
        );
    }

    #[test]
    fn convert_macro_body_quote_top_level() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        // <quote> at top level (handled directly in convert_macro_body, not via convert_form_to_stmt)
        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        let quote_form = Form {
            head: "quote".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Text("hello".to_string())]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(quote_form)])
            .expect("quote top-level should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value, .. } => {
                assert!(matches!(value, CtExpr::QuoteForms { .. }));
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_stmt_require_module_standalone() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        // <require_module> as a standalone statement (not inside let)
        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        let require_form = Form {
            head: "require_module".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "var".to_string(),
                    meta: meta.clone(),
                    fields: vec![FormField {
                        name: "name".to_string(),
                        value: FormValue::String("mod".to_string()),
                    }],
                })]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(require_form)])
            .expect("require_module should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Expr { expr } => {
                assert!(
                    matches!(expr, CtExpr::BuiltinCall { name, .. } if name == "require_module")
                );
            }
            other => panic!("expected Expr, got {:?}", other),
        }
    }

    #[test]
    fn convert_stmt_expand_alias_standalone() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        let alias_form = Form {
            head: "expand_alias".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "var".to_string(),
                    meta: meta.clone(),
                    fields: vec![FormField {
                        name: "name".to_string(),
                        value: FormValue::String("H".to_string()),
                    }],
                })]),
            }],
        };

        let result =
            convert_macro_body(&[FormItem::Form(alias_form)]).expect("expand_alias should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Expr { expr } => {
                assert!(matches!(expr, CtExpr::BuiltinCall { name, .. } if name == "expand_alias"));
            }
            other => panic!("expected Expr, got {:?}", other),
        }
    }

    #[test]
    fn convert_stmt_keyword_attr_standalone() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        let keyword_form = Form {
            head: "keyword_attr".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "name".to_string(),
                value: FormValue::String("opts".to_string()),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(keyword_form)])
            .expect("keyword_attr should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Expr { expr } => {
                assert!(matches!(expr, CtExpr::BuiltinCall { name, .. } if name == "keyword_attr"));
            }
            other => panic!("expected Expr, got {:?}", other),
        }
    }

    #[test]
    fn convert_stmt_invoke_macro_standalone() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        let invoke_form = Form {
            head: "invoke_macro".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "module".to_string(),
                    value: FormValue::String("helper".to_string()),
                },
                FormField {
                    name: "macro_name".to_string(),
                    value: FormValue::String("__using__".to_string()),
                },
                FormField {
                    name: "opts".to_string(),
                    value: FormValue::String("opts".to_string()),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(invoke_form)])
            .expect("invoke_macro should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value } => {
                assert!(
                    matches!(value, CtExpr::BuiltinCall { name, .. } if name == "invoke_macro")
                );
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_let_caller_module_provider() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <let name="mod" type="string"><caller_module/></let>
        let let_form = Form {
            head: "let".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("mod".to_string()),
                },
                FormField {
                    name: "type".to_string(),
                    value: FormValue::String("string".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                        head: "caller_module".to_string(),
                        meta: meta.clone(),
                        fields: vec![],
                    })]),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(let_form)])
            .expect("caller_module provider should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Let { name, value } => {
                assert_eq!(name, "mod");
                assert!(
                    matches!(value, CtExpr::BuiltinCall { name, .. } if name == "caller_module")
                );
            }
            other => panic!("expected Let, got {:?}", other),
        }
    }

    #[test]
    fn convert_let_require_module_provider() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <let name="mod" type="string"><require_module><var name="helper"/></require_module></let>
        let let_form = Form {
            head: "let".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("mod".to_string()),
                },
                FormField {
                    name: "type".to_string(),
                    value: FormValue::String("string".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                        head: "require_module".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Form(Form {
                                head: "var".to_string(),
                                meta: meta.clone(),
                                fields: vec![FormField {
                                    name: "name".to_string(),
                                    value: FormValue::String("helper".to_string()),
                                }],
                            })]),
                        }],
                    })]),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(let_form)])
            .expect("require_module provider should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Let { name, value } => {
                assert_eq!(name, "mod");
                assert!(
                    matches!(value, CtExpr::BuiltinCall { name, .. } if name == "require_module")
                );
            }
            other => panic!("expected Let, got {:?}", other),
        }
    }

    #[test]
    fn convert_if_empty_children_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <if/> with empty children
        let if_form = Form {
            head: "if".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(if_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("requires at least condition and then block"),
            "got: {}",
            err
        );
    }

    #[test]
    fn convert_if_second_child_not_then_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <if><get-attr name="x"/><other/></if> - second child is <other>, not <then>
        let if_form = Form {
            head: "if".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![
                    FormItem::Form(Form {
                        head: "get-attribute".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "name".to_string(),
                            value: FormValue::String("x".to_string()),
                        }],
                    }),
                    FormItem::Form(Form {
                        head: "other".to_string(),
                        meta: meta.clone(),
                        fields: vec![],
                    }),
                ]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(if_form)]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("second child must be <then> block")
        );
    }

    #[test]
    fn convert_if_third_child_not_else_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <if><get-attr name="x"/><then/><other/></if> - third child is <other>, not <else>
        let if_form = Form {
            head: "if".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![
                    FormItem::Form(Form {
                        head: "get-attribute".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "name".to_string(),
                            value: FormValue::String("x".to_string()),
                        }],
                    }),
                    FormItem::Form(Form {
                        head: "then".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![]),
                        }],
                    }),
                    FormItem::Form(Form {
                        head: "other".to_string(),
                        meta: meta.clone(),
                        fields: vec![],
                    }),
                ]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(if_form)]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("third child must be <else> block")
        );
    }

    #[test]
    fn convert_return_empty_children_returns_nil() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <return/> with no children
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(return_form)])
            .expect("empty return should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value } => {
                assert!(matches!(value, CtExpr::Literal(CtValue::Nil)));
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_provider_quote_wrong_type_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <let name="x" type="string"><quote><text>hi</text></quote></let>
        let let_form = Form {
            head: "let".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("x".to_string()),
                },
                FormField {
                    name: "type".to_string(),
                    value: FormValue::String("string".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                        head: "quote".to_string(),
                        meta: meta.clone(),
                        fields: vec![FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Text("hi".to_string())]),
                        }],
                    })]),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(let_form)]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("<quote> provider requires type")
        );
    }

    #[test]
    fn convert_expr_var() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <return><var name="my_param"/></return>
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "var".to_string(),
                    meta: meta.clone(),
                    fields: vec![FormField {
                        name: "name".to_string(),
                        value: FormValue::String("my_param".to_string()),
                    }],
                })]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(return_form)])
            .expect("var expression should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value } => {
                assert!(matches!(value, CtExpr::Var { name } if name == "my_param"));
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_expr_require_module() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <return><require_module><var name="helper"/></require_module></return>
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "require_module".to_string(),
                    meta: meta.clone(),
                    fields: vec![FormField {
                        name: "children".to_string(),
                        value: FormValue::Sequence(vec![FormItem::Form(Form {
                            head: "var".to_string(),
                            meta: meta.clone(),
                            fields: vec![FormField {
                                name: "name".to_string(),
                                value: FormValue::String("helper".to_string()),
                            }],
                        })]),
                    }],
                })]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(return_form)])
            .expect("require_module expression should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value } => {
                assert!(
                    matches!(value, CtExpr::BuiltinCall { name, .. } if name == "require_module")
                );
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_expr_expand_alias() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <return><expand_alias><var name="H"/></expand_alias></return>
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "expand_alias".to_string(),
                    meta: meta.clone(),
                    fields: vec![FormField {
                        name: "children".to_string(),
                        value: FormValue::Sequence(vec![FormItem::Form(Form {
                            head: "var".to_string(),
                            meta: meta.clone(),
                            fields: vec![FormField {
                                name: "name".to_string(),
                                value: FormValue::String("H".to_string()),
                            }],
                        })]),
                    }],
                })]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(return_form)])
            .expect("expand_alias expression should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value } => {
                assert!(
                    matches!(value, CtExpr::BuiltinCall { name, .. } if name == "expand_alias")
                );
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_expr_keyword_attr() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <return><keyword_attr name="opts"/></return>
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "keyword_attr".to_string(),
                    meta: meta.clone(),
                    fields: vec![FormField {
                        name: "name".to_string(),
                        value: FormValue::String("opts".to_string()),
                    }],
                })]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(return_form)])
            .expect("keyword_attr expression should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value } => {
                assert!(
                    matches!(value, CtExpr::BuiltinCall { name, .. } if name == "keyword_attr")
                );
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_expr_quote() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <return><quote><text>hello</text></quote></return>
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "quote".to_string(),
                    meta: meta.clone(),
                    fields: vec![FormField {
                        name: "children".to_string(),
                        value: FormValue::Sequence(vec![FormItem::Text("hello".to_string())]),
                    }],
                })]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(return_form)])
            .expect("quote expression should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value } => {
                assert!(matches!(value, CtExpr::QuoteForms { .. }));
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_expr_invoke_macro_var_child() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <return><invoke_macro macro_name="__using__" opts="opts"><var name="helper"/></invoke_macro></return>
        // No module attribute: falls back to child <var>
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "invoke_macro".to_string(),
                    meta: meta.clone(),
                    fields: vec![
                        FormField {
                            name: "macro_name".to_string(),
                            value: FormValue::String("__using__".to_string()),
                        },
                        FormField {
                            name: "opts".to_string(),
                            value: FormValue::String("opts".to_string()),
                        },
                        FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Form(Form {
                                head: "var".to_string(),
                                meta: meta.clone(),
                                fields: vec![FormField {
                                    name: "name".to_string(),
                                    value: FormValue::String("helper".to_string()),
                                }],
                            })]),
                        },
                    ],
                })]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(return_form)])
            .expect("invoke_macro with var child should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value } => {
                assert!(
                    matches!(value, CtExpr::BuiltinCall { name, .. } if name == "invoke_macro")
                );
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_expr_invoke_macro_get_attribute_child() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <return><invoke_macro macro_name="__using__" opts="opts"><get-attribute name="mod"/></invoke_macro></return>
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "invoke_macro".to_string(),
                    meta: meta.clone(),
                    fields: vec![
                        FormField {
                            name: "macro_name".to_string(),
                            value: FormValue::String("__using__".to_string()),
                        },
                        FormField {
                            name: "opts".to_string(),
                            value: FormValue::String("opts".to_string()),
                        },
                        FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Form(Form {
                                head: "get-attribute".to_string(),
                                meta: meta.clone(),
                                fields: vec![FormField {
                                    name: "name".to_string(),
                                    value: FormValue::String("mod".to_string()),
                                }],
                            })]),
                        },
                    ],
                })]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(return_form)])
            .expect("invoke_macro with get-attribute child should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value } => {
                assert!(
                    matches!(value, CtExpr::BuiltinCall { name, .. } if name == "invoke_macro")
                );
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_expr_invoke_macro_no_module_no_children_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <invoke_macro macro_name="__using__" opts="opts"/> - no module attr, empty children
        let invoke_form = Form {
            head: "invoke_macro".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "macro_name".to_string(),
                    value: FormValue::String("__using__".to_string()),
                },
                FormField {
                    name: "opts".to_string(),
                    value: FormValue::String("opts".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![]),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(invoke_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("requires module attribute or <var>/<get-attribute> child"),
            "got: {}",
            err
        );
    }

    #[test]
    fn convert_expr_invoke_macro_opts_attr_not_opts_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <invoke_macro module="helper" macro_name="__using__" opts="other"/> - opts is not 'opts'
        let invoke_form = Form {
            head: "invoke_macro".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "module".to_string(),
                    value: FormValue::String("helper".to_string()),
                },
                FormField {
                    name: "macro_name".to_string(),
                    value: FormValue::String("__using__".to_string()),
                },
                FormField {
                    name: "opts".to_string(),
                    value: FormValue::String("other".to_string()),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(invoke_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("opts attribute must be 'opts'"),
            "got: {}",
            err
        );
    }

    #[test]
    fn convert_expr_invoke_macro_invalid_child_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <invoke_macro macro_name="__using__" opts="opts"><other/></invoke_macro> - invalid child type
        let invoke_form = Form {
            head: "invoke_macro".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "macro_name".to_string(),
                    value: FormValue::String("__using__".to_string()),
                },
                FormField {
                    name: "opts".to_string(),
                    value: FormValue::String("opts".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                        head: "other".to_string(),
                        meta: meta.clone(),
                        fields: vec![],
                    })]),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(invoke_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("child must be <var> or <get-attribute>"),
            "got: {}",
            err
        );
    }

    #[test]
    fn single_child_form_zero_children_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <set name="x"><children/></set> with empty children
        let set_form = Form {
            head: "set".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("x".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![]),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(set_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("requires exactly one meaningful child"),
            "got: {}",
            err
        );
    }

    #[test]
    fn single_child_form_multiple_children_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <set name="x"><var name="a"/><var name="b"/></set> - two children
        let set_form = Form {
            head: "set".to_string(),
            meta: meta.clone(),
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("x".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![
                        FormItem::Form(Form {
                            head: "var".to_string(),
                            meta: meta.clone(),
                            fields: vec![FormField {
                                name: "name".to_string(),
                                value: FormValue::String("a".to_string()),
                            }],
                        }),
                        FormItem::Form(Form {
                            head: "var".to_string(),
                            meta: meta.clone(),
                            fields: vec![FormField {
                                name: "name".to_string(),
                                value: FormValue::String("b".to_string()),
                            }],
                        }),
                    ]),
                },
            ],
        };

        let result = convert_macro_body(&[FormItem::Form(set_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("requires exactly one meaningful child"),
            "got: {}",
            err
        );
    }

    #[test]
    fn convert_expr_invoke_macro_opts_keyword_attr_fallback() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <return><invoke_macro module="helper" macro_name="__using__"><keyword_attr name="opts"/></invoke_macro></return>
        // No opts attr: falls back to <keyword_attr name="opts"/> child
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "invoke_macro".to_string(),
                    meta: meta.clone(),
                    fields: vec![
                        FormField {
                            name: "module".to_string(),
                            value: FormValue::String("helper".to_string()),
                        },
                        FormField {
                            name: "macro_name".to_string(),
                            value: FormValue::String("__using__".to_string()),
                        },
                        FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Form(Form {
                                head: "keyword_attr".to_string(),
                                meta: meta.clone(),
                                fields: vec![FormField {
                                    name: "name".to_string(),
                                    value: FormValue::String("opts".to_string()),
                                }],
                            })]),
                        },
                    ],
                })]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(return_form)])
            .expect("invoke_macro with keyword_attr opts fallback should succeed");
        assert_eq!(result.stmts.len(), 1);
        match &result.stmts[0] {
            CtStmt::Return { value } => {
                assert!(
                    matches!(value, CtExpr::BuiltinCall { name, .. } if name == "invoke_macro")
                );
            }
            other => panic!("expected Return, got {:?}", other),
        }
    }

    #[test]
    fn convert_expr_invoke_macro_no_opts_attr_no_kw_child_errors() {
        use crate::semantic::macro_lang::convert::convert_macro_body;

        let meta = FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // <return><invoke_macro module="helper" macro_name="__using__"><other/></invoke_macro></return>
        // No opts attr, child is not keyword_attr -> error
        let return_form = Form {
            head: "return".to_string(),
            meta: meta.clone(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Form(Form {
                    head: "invoke_macro".to_string(),
                    meta: meta.clone(),
                    fields: vec![
                        FormField {
                            name: "module".to_string(),
                            value: FormValue::String("helper".to_string()),
                        },
                        FormField {
                            name: "macro_name".to_string(),
                            value: FormValue::String("__using__".to_string()),
                        },
                        FormField {
                            name: "children".to_string(),
                            value: FormValue::Sequence(vec![FormItem::Form(Form {
                                head: "other".to_string(),
                                meta: meta.clone(),
                                fields: vec![],
                            })]),
                        },
                    ],
                })]),
            }],
        };

        let result = convert_macro_body(&[FormItem::Form(return_form)]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("requires opts=\"opts\" attribute or <keyword_attr"),
            "got: {}",
            err
        );
    }

    // ========================================================================
    // Additional builtin error path tests
    // ========================================================================

    #[test]
    fn builtin_invoke_macro_wrong_third_arg_type() {
        let mut macro_env = MacroEnv {
            current_module: Some("main".to_string()),
            ..Default::default()
        };
        macro_env.requires.push("helper".to_string());

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        // Third arg is an integer instead of keyword
        let args = &[
            CtValue::String("helper".to_string()),
            CtValue::String("macro".to_string()),
            CtValue::Int(42),
        ];
        let result =
            builtins.get("invoke_macro").unwrap()(args, &macro_env, &mut ct_env, &mut expand_env);
        // Should return an error (type check failure or module-not-found)
        let err = result.expect_err("wrong third arg type should error");
        // The error should mention args type issue OR the builtin invocation failure
        let err_str = err.to_string();
        // Verify the error is about type mismatch (args/keyword) OR invocation failure
        assert!(
            err_str.contains("keyword")
                || err_str.contains("invoke_macro")
                || err_str.contains("not found"),
            "unexpected error: {}",
            err_str
        );
    }

    #[test]
    fn builtin_invoke_macro_wrong_first_arg_type() {
        let mut macro_env = MacroEnv {
            current_module: Some("main".to_string()),
            ..Default::default()
        };
        macro_env.requires.push("helper".to_string());

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        // First arg is an integer instead of string/module
        let err = builtins.get("invoke_macro").unwrap()(
            &[
                CtValue::Int(42),
                CtValue::String("macro".to_string()),
                CtValue::Keyword(vec![]),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong first arg type");
        assert!(
            err.to_string()
                .contains("first argument (module) must be string or module")
        );
    }

    #[test]
    fn builtin_invoke_macro_module_not_in_scope_errors() {
        let macro_env = MacroEnv {
            current_module: Some("caller".to_string()),
            ..Default::default()
        };
        // helper is NOT in requires or current_module

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("invoke_macro").unwrap()(
            &[
                CtValue::String("helper".to_string()),
                CtValue::String("__using__".to_string()),
                CtValue::Keyword(vec![]),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("module not in scope");
        assert!(err.to_string().contains("module not in scope"));
    }

    #[test]
    fn builtin_invoke_macro_macro_not_found_errors() {
        let mut macro_env = MacroEnv {
            current_module: Some("main".to_string()),
            ..Default::default()
        };
        macro_env.requires.push("helper".to_string());

        let mut expand_env = empty_expand_env();
        expand_env
            .begin_module(Some("helper".to_string()), Some("helper.xml".to_string()))
            .expect("helper module");
        // Register helper module with no macros

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();

        let err = builtins.get("invoke_macro").unwrap()(
            &[
                CtValue::String("helper".to_string()),
                CtValue::String("nonexistent".to_string()),
                CtValue::Keyword(vec![]),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("macro not found");
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn builtin_invoke_macro_wrong_keyword_arg_value_type_errors() {
        use crate::semantic::env::MacroDefinition;

        let mut macro_env = MacroEnv {
            current_module: Some("main".to_string()),
            ..Default::default()
        };
        macro_env.requires.push("helper".to_string());

        let mut expand_env = empty_expand_env();
        expand_env
            .begin_module(Some("helper".to_string()), Some("helper.xml".to_string()))
            .expect("helper module");

        let quote_meta = FormMeta {
            source_name: Some("helper.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        fn make_field(name: &str, value: FormValue) -> FormField {
            FormField {
                name: name.to_string(),
                value,
            }
        }
        fn make_seq(items: Vec<FormItem>) -> FormValue {
            FormValue::Sequence(items)
        }
        fn make_form_item(meta: &FormMeta, head: &str, fields: Vec<FormField>) -> FormItem {
            FormItem::Form(sl_core::Form {
                head: head.to_string(),
                meta: meta.clone(),
                fields,
            })
        }

        let macro_body = vec![make_form_item(
            &quote_meta,
            "quote",
            vec![make_field(
                "children",
                make_seq(vec![make_form_item(
                    &quote_meta,
                    "text",
                    vec![make_field(
                        "children",
                        make_seq(vec![FormItem::Text("ok".to_string())]),
                    )],
                )]),
            )],
        )];

        expand_env
            .program
            .register_macro(MacroDefinition {
                module_name: "helper".to_string(),
                name: "__using__".to_string(),
                params: Some(vec![crate::semantic::env::MacroParam {
                    param_type: crate::semantic::env::MacroParamType::Keyword,
                    name: "opts".to_string(),
                }]),
                legacy_protocol: None,
                body: macro_body,
                is_private: false,
            })
            .expect("register macro");

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();

        // Keyword arg value is Ast (not string/int/bool)
        let err = builtins.get("invoke_macro").unwrap()(
            &[
                CtValue::String("helper".to_string()),
                CtValue::String("__using__".to_string()),
                CtValue::Keyword(vec![("opt1".to_string(), CtValue::Ast(vec![]))]),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong keyword arg value type");
        assert!(
            err.to_string()
                .contains("keyword arg value must be string, int, or bool")
        );
    }

    #[test]
    fn builtin_invoke_macro_resolve_alias() {
        use crate::semantic::env::MacroDefinition;

        let mut macro_env = MacroEnv {
            current_module: Some("main".to_string()),
            ..Default::default()
        };
        // Alias H -> helper
        macro_env
            .aliases
            .insert("H".to_string(), "helper".to_string());
        macro_env.requires.push("helper".to_string());

        let mut expand_env = empty_expand_env();
        expand_env
            .begin_module(Some("helper".to_string()), Some("helper.xml".to_string()))
            .expect("helper module");

        let quote_meta = FormMeta {
            source_name: Some("helper.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        fn make_field(name: &str, value: FormValue) -> FormField {
            FormField {
                name: name.to_string(),
                value,
            }
        }
        fn make_seq(items: Vec<FormItem>) -> FormValue {
            FormValue::Sequence(items)
        }
        fn make_form_item(meta: &FormMeta, head: &str, fields: Vec<FormField>) -> FormItem {
            FormItem::Form(sl_core::Form {
                head: head.to_string(),
                meta: meta.clone(),
                fields,
            })
        }

        let macro_body = vec![make_form_item(
            &quote_meta,
            "quote",
            vec![make_field(
                "children",
                make_seq(vec![make_form_item(
                    &quote_meta,
                    "text",
                    vec![make_field(
                        "children",
                        make_seq(vec![FormItem::Text("aliased".to_string())]),
                    )],
                )]),
            )],
        )];

        expand_env
            .program
            .register_macro(MacroDefinition {
                module_name: "helper".to_string(),
                name: "__using__".to_string(),
                params: None,
                legacy_protocol: None,
                body: macro_body,
                is_private: false,
            })
            .expect("register macro");

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();

        // Invoke using alias "H" (resolves to "helper")
        let result = builtins.get("invoke_macro").unwrap()(
            &[
                CtValue::String("H".to_string()),
                CtValue::String("__using__".to_string()),
                CtValue::Keyword(vec![]),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("invoke_macro with alias should succeed");
        match result {
            CtValue::Ast(items) => {
                assert!(!items.is_empty());
            }
            other => panic!("expected Ast, got {:?}", other),
        }
    }

    #[test]
    fn builtin_expand_alias_wrong_type() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("expand_alias").unwrap()(
            &[CtValue::Int(123)],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong type");
        assert!(err.to_string().contains("must be string or module"));
    }

    #[test]
    fn builtin_require_module_wrong_type() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("require_module").unwrap()(
            &[CtValue::Int(123)],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong type");
        assert!(err.to_string().contains("must be string or module"));
    }

    #[test]
    fn builtin_define_import_wrong_type() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("define_import").unwrap()(
            &[CtValue::Int(123)],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong type");
        assert!(err.to_string().contains("must be string or module"));
    }

    #[test]
    fn builtin_define_alias_wrong_first_arg_type() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("define_alias").unwrap()(
            &[CtValue::Int(123), CtValue::String("a".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong type");
        assert!(
            err.to_string()
                .contains("first argument must be string or module")
        );
    }

    #[test]
    fn builtin_define_alias_wrong_second_arg_type() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("define_alias").unwrap()(
            &[CtValue::String("helper".to_string()), CtValue::Int(123)],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong type");
        assert!(err.to_string().contains("second argument must be string"));
    }

    #[test]
    fn builtin_define_require_wrong_type() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("define_require").unwrap()(
            &[CtValue::Int(123)],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong type");
        assert!(err.to_string().contains("must be string or module"));
    }

    #[test]
    fn builtin_invoke_macro_wrong_second_arg_type() {
        let mut macro_env = MacroEnv {
            current_module: Some("main".to_string()),
            ..Default::default()
        };
        macro_env.requires.push("helper".to_string());

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("invoke_macro").unwrap()(
            &[
                CtValue::String("helper".to_string()),
                CtValue::Int(123),
                CtValue::Keyword(vec![]),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong type");
        assert!(err.to_string().contains("macro_name"));
    }

    #[test]
    fn builtin_invoke_macro_requires_3_args() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        // Too few args
        let err = builtins.get("invoke_macro").unwrap()(
            &[CtValue::String("helper".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("too few args");
        assert!(err.to_string().contains("requires exactly 3"));

        // Too many args
        let err = builtins.get("invoke_macro").unwrap()(
            &[
                CtValue::String("helper".to_string()),
                CtValue::String("macro".to_string()),
                CtValue::Keyword(vec![]),
                CtValue::String("extra".to_string()),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("too many args");
        assert!(err.to_string().contains("requires exactly 3"));
    }

    #[test]
    fn builtin_keyword_attr_missing_keyword() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("keyword_attr").unwrap()(
            &[CtValue::String("missing".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("missing keyword");
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn builtin_caller_module_returns_module_name() {
        let macro_env = MacroEnv {
            current_module: Some("test_module".to_string()),
            ..Default::default()
        };
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result =
            builtins.get("caller_module").unwrap()(&[], &macro_env, &mut ct_env, &mut expand_env)
                .expect("caller_module should succeed");
        assert_eq!(result, CtValue::String("test_module".to_string()));

        // Test without module set
        let macro_env = MacroEnv::default();
        let result =
            builtins.get("caller_module").unwrap()(&[], &macro_env, &mut ct_env, &mut expand_env)
                .expect("caller_module should succeed");
        assert_eq!(result, CtValue::String("<unknown>".to_string()));
    }

    #[test]
    fn builtin_content_with_filter_head() {
        use sl_core::FormItem;

        // Helper to create minimal FormMeta
        fn minimal_meta() -> FormMeta {
            FormMeta {
                source_name: None,
                start: SourcePosition { row: 0, column: 0 },
                end: SourcePosition { row: 0, column: 0 },
                start_byte: 0,
                end_byte: 0,
            }
        }

        let macro_env = MacroEnv {
            content: vec![
                FormItem::Form(sl_core::Form {
                    head: "slot".to_string(),
                    meta: minimal_meta(),
                    fields: vec![FormField {
                        name: "children".to_string(),
                        value: FormValue::Sequence(vec![FormItem::Text(
                            "slot content".to_string(),
                        )]),
                    }],
                }),
                FormItem::Form(sl_core::Form {
                    head: "other".to_string(),
                    meta: minimal_meta(),
                    fields: vec![FormField {
                        name: "children".to_string(),
                        value: FormValue::Sequence(vec![FormItem::Text(
                            "other content".to_string(),
                        )]),
                    }],
                }),
            ],
            ..Default::default()
        };
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let result = builtins.get("content").unwrap()(
            &[CtValue::Keyword(vec![(
                "head".to_string(),
                CtValue::String("slot".to_string()),
            )])],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("content with filter");
        match result {
            CtValue::Ast(items) => {
                assert_eq!(items.len(), 1);
                // The content should be "slot content"
                if let FormItem::Text(t) = &items[0] {
                    assert_eq!(t, "slot content");
                } else {
                    panic!("expected text");
                }
            }
            other => panic!("expected Ast, got {}", other.type_name()),
        }
    }

    #[test]
    fn builtin_content_with_filter_wrong_type_in_kv() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("content").unwrap()(
            &[CtValue::Keyword(vec![(
                "head".to_string(),
                CtValue::Int(123),
            )])],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong type in kv");
        assert!(err.to_string().contains("must be string"));
    }

    #[test]
    fn builtin_expand_alias_wrong_arg_count() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("expand_alias").unwrap()(
            &[
                CtValue::String("a".to_string()),
                CtValue::String("b".to_string()),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("too many args");
        assert!(err.to_string().contains("requires exactly 1"));
    }

    #[test]
    fn builtin_require_module_wrong_arg_count() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err =
            builtins.get("require_module").unwrap()(&[], &macro_env, &mut ct_env, &mut expand_env)
                .expect_err("too few args");
        assert!(err.to_string().contains("requires exactly 1"));
    }

    #[test]
    fn builtin_define_import_wrong_arg_count() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err =
            builtins.get("define_import").unwrap()(&[], &macro_env, &mut ct_env, &mut expand_env)
                .expect_err("too few args");
        assert!(err.to_string().contains("requires exactly 1"));
    }

    #[test]
    fn builtin_define_alias_wrong_arg_count() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("define_alias").unwrap()(
            &[CtValue::String("helper".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("too few args");
        assert!(err.to_string().contains("requires exactly 2"));
    }

    #[test]
    fn builtin_define_require_wrong_arg_count() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err =
            builtins.get("define_require").unwrap()(&[], &macro_env, &mut ct_env, &mut expand_env)
                .expect_err("too few args");
        assert!(err.to_string().contains("requires exactly 1"));
    }

    #[test]
    fn builtin_invoke_macro_wrong_arg_count() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("invoke_macro").unwrap()(
            &[CtValue::String("helper".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("too few args");
        assert!(err.to_string().contains("requires exactly 3"));
    }

    #[test]
    fn builtin_keyword_get_wrong_arg_count() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("keyword_get").unwrap()(
            &[CtValue::Keyword(vec![])],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("too few args");
        assert!(err.to_string().contains("requires exactly 2"));
    }

    #[test]
    fn builtin_keyword_has_wrong_first_arg_type() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("keyword_has").unwrap()(
            &[
                CtValue::String("not a keyword".to_string()),
                CtValue::String("key".to_string()),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong first arg type");
        assert!(err.to_string().contains("first argument must be keyword"));
    }

    #[test]
    fn builtin_keyword_has_wrong_second_arg_type() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("keyword_has").unwrap()(
            &[CtValue::Keyword(vec![]), CtValue::Int(123)],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong second arg type");
        assert!(err.to_string().contains("second argument must be string"));
    }

    #[test]
    fn builtin_list_length_wrong_type() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        let err = builtins.get("list_length").unwrap()(
            &[CtValue::String("not a list".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("wrong type");
        assert!(err.to_string().contains("must be list or keyword"));
    }

    #[test]
    fn builtin_to_string_complex_value() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();
        let mut expand_env = empty_expand_env();

        // Test to_string with complex types that fall into the catch-all branch
        let result = builtins.get("to_string").unwrap()(
            &[CtValue::Ast(vec![])],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("to_string should succeed");
        assert_eq!(result, CtValue::String("Ast([])".to_string()));

        let result = builtins.get("to_string").unwrap()(
            &[CtValue::ModuleRef("test".to_string())],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect("to_string should succeed");
        assert_eq!(result, CtValue::String("ModuleRef(\"test\")".to_string()));
    }

    #[test]
    fn builtin_invoke_macro_rejects_private_macro() {
        use sl_core::{FormField, FormMeta, FormValue, SourcePosition};

        let mut macro_env = MacroEnv {
            current_module: Some("caller".to_string()),
            ..Default::default()
        };
        macro_env.requires.push("helper".to_string());

        // Register helper module with a private macro
        let mut expand_env = empty_expand_env();
        expand_env
            .begin_module(Some("helper".to_string()), Some("helper.xml".to_string()))
            .expect("helper module");

        let quote_meta = FormMeta {
            source_name: Some("helper.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        };

        // Helper to build Form structures cleanly
        fn make_form(meta: &FormMeta, head: &str, fields: Vec<FormField>) -> sl_core::Form {
            sl_core::Form {
                head: head.to_string(),
                meta: meta.clone(),
                fields,
            }
        }
        fn make_field(name: &str, value: FormValue) -> FormField {
            FormField {
                name: name.to_string(),
                value,
            }
        }
        fn make_seq(items: Vec<FormItem>) -> FormValue {
            FormValue::Sequence(items)
        }
        fn make_text(content: &str) -> FormItem {
            FormItem::Text(content.to_string())
        }
        fn make_form_item(meta: &FormMeta, head: &str, fields: Vec<FormField>) -> FormItem {
            FormItem::Form(make_form(meta, head, fields))
        }

        let text_child = make_form_item(
            &quote_meta,
            "text",
            vec![make_field(
                "children",
                make_seq(vec![make_text("private result")]),
            )],
        );
        let macro_body = vec![make_form_item(
            &quote_meta,
            "quote",
            vec![make_field("children", make_seq(vec![text_child]))],
        )];

        // Register a PRIVATE macro
        expand_env
            .program
            .register_macro(crate::semantic::env::MacroDefinition {
                module_name: "helper".to_string(),
                name: "__private__".to_string(),
                params: Some(vec![crate::semantic::env::MacroParam {
                    param_type: crate::semantic::env::MacroParamType::Keyword,
                    name: "opts".to_string(),
                }]),
                legacy_protocol: None,
                body: macro_body,
                is_private: true, // This is a private macro
            })
            .expect("register macro");

        expand_env
            .begin_module(Some("caller".to_string()), Some("caller.xml".to_string()))
            .expect("caller module");

        // Add helper to caller's requires list
        expand_env.module.requires.push("helper".to_string());

        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();

        // Try to invoke the private macro from a different module
        let err = builtins.get("invoke_macro").unwrap()(
            &[
                CtValue::String("helper".to_string()),
                CtValue::String("__private__".to_string()),
                CtValue::Keyword(vec![]),
            ],
            &macro_env,
            &mut ct_env,
            &mut expand_env,
        )
        .expect_err("should reject private macro");
        assert!(
            err.to_string().contains("private macro"),
            "error should mention private macro: {}",
            err
        );
        assert!(
            err.to_string().contains("helper.__private__"),
            "error should mention macro name: {}",
            err
        );
    }
}
