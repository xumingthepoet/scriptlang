//! Unit tests for the compile-time macro language.

#[cfg(test)]
mod ct_lang_tests {
    use crate::semantic::expand::macro_env::MacroEnv;
    use crate::semantic::macro_lang::*;

    fn empty_macro_env() -> MacroEnv {
        MacroEnv::default()
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

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins)
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

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins)
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

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins)
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

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins);
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

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins)
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

        let result = eval_block(&block, &macro_env, &mut ct_env, &builtins)
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

        let result = builtins.get("attr").expect("attr builtin exists")(
            &[CtValue::String("name".to_string())],
            &macro_env,
            &mut ct_env,
        )
        .expect("attr should succeed");
        assert_eq!(result, CtValue::String("test".to_string()));

        // Error: wrong arg count
        let err = builtins.get("attr").unwrap()(&[], &macro_env, &mut ct_env)
            .expect_err("wrong arg count");
        assert!(err.to_string().contains("requires exactly 1 argument"));

        // Error: wrong type
        let err = builtins.get("attr").unwrap()(&[CtValue::Int(1)], &macro_env, &mut ct_env)
            .expect_err("wrong type");
        assert!(err.to_string().contains("must be string"));

        // Error: missing attribute
        let err = builtins.get("attr").unwrap()(
            &[CtValue::String("missing".to_string())],
            &macro_env,
            &mut ct_env,
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

        let result = builtins.get("has_attr").expect("has_attr exists")(
            &[CtValue::String("exists".to_string())],
            &macro_env,
            &mut ct_env,
        )
        .expect("has_attr should succeed");
        assert_eq!(result, CtValue::Bool(true));

        let result = builtins.get("has_attr").unwrap()(
            &[CtValue::String("missing".to_string())],
            &macro_env,
            &mut ct_env,
        )
        .expect("has_attr missing");
        assert_eq!(result, CtValue::Bool(false));

        // Error: wrong arg count
        let err = builtins.get("has_attr").unwrap()(&[], &macro_env, &mut ct_env)
            .expect_err("wrong arg count");
        assert!(err.to_string().contains("requires exactly 1 argument"));
    }

    #[test]
    fn builtin_parse_bool_and_int_work() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();

        // parse_bool
        let result = builtins.get("parse_bool").expect("parse_bool exists")(
            &[CtValue::String("true".to_string())],
            &macro_env,
            &mut ct_env,
        )
        .expect("parse_bool true");
        assert_eq!(result, CtValue::Bool(true));

        let result = builtins.get("parse_bool").unwrap()(
            &[CtValue::String("false".to_string())],
            &macro_env,
            &mut ct_env,
        )
        .expect("parse_bool false");
        assert_eq!(result, CtValue::Bool(false));

        let err = builtins.get("parse_bool").unwrap()(
            &[CtValue::String("invalid".to_string())],
            &macro_env,
            &mut ct_env,
        )
        .expect_err("parse_bool invalid");
        assert!(err.to_string().contains("cannot parse"));

        // parse_int
        let result = builtins.get("parse_int").expect("parse_int exists")(
            &[CtValue::String("42".to_string())],
            &macro_env,
            &mut ct_env,
        )
        .expect("parse_int 42");
        assert_eq!(result, CtValue::Int(42));

        let err = builtins.get("parse_int").unwrap()(
            &[CtValue::String("abc".to_string())],
            &macro_env,
            &mut ct_env,
        )
        .expect_err("parse_int invalid");
        assert!(err.to_string().contains("cannot parse"));
    }

    #[test]
    fn builtin_keyword_and_list_operations() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();

        let keyword = CtValue::Keyword(vec![
            ("name".to_string(), CtValue::String("test".to_string())),
            ("count".to_string(), CtValue::Int(5)),
        ]);

        // keyword_get
        let result = builtins.get("keyword_get").expect("keyword_get exists")(
            &[keyword.clone(), CtValue::String("name".to_string())],
            &macro_env,
            &mut ct_env,
        )
        .expect("keyword_get");
        assert_eq!(result, CtValue::String("test".to_string()));

        let err = builtins.get("keyword_get").unwrap()(
            &[keyword.clone(), CtValue::String("missing".to_string())],
            &macro_env,
            &mut ct_env,
        )
        .expect_err("keyword_get missing");
        assert!(err.to_string().contains("not found"));

        // keyword_has
        let result = builtins.get("keyword_has").expect("keyword_has exists")(
            &[keyword.clone(), CtValue::String("name".to_string())],
            &macro_env,
            &mut ct_env,
        )
        .expect("keyword_has");
        assert_eq!(result, CtValue::Bool(true));

        // list_length
        let result = builtins.get("list_length").expect("list_length exists")(
            &[keyword],
            &macro_env,
            &mut ct_env,
        )
        .expect("list_length keyword");
        assert_eq!(result, CtValue::Int(2));

        let list = CtValue::List(vec![CtValue::Nil, CtValue::Nil, CtValue::Nil]);
        let result = builtins.get("list_length").unwrap()(&[list], &macro_env, &mut ct_env)
            .expect("list_length list");
        assert_eq!(result, CtValue::Int(3));
    }

    #[test]
    fn builtin_to_string_works() {
        let macro_env = MacroEnv::default();
        let mut ct_env = CtEnv::new();
        let builtins = BuiltinRegistry::new();

        let result = builtins.get("to_string").expect("to_string exists")(
            &[CtValue::Bool(true)],
            &macro_env,
            &mut ct_env,
        )
        .expect("to_string bool");
        assert_eq!(result, CtValue::String("true".to_string()));

        let result =
            builtins.get("to_string").unwrap()(&[CtValue::Int(123)], &macro_env, &mut ct_env)
                .expect("to_string int");
        assert_eq!(result, CtValue::String("123".to_string()));

        let result = builtins.get("to_string").unwrap()(&[CtValue::Nil], &macro_env, &mut ct_env)
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

        let result = builtins.get("content").expect("content exists")(&[], &macro_env, &mut ct_env)
            .expect("content");
        assert!(matches!(result, CtValue::Ast(items) if items.len() == 1));

        // Error: too many args
        let err = builtins.get("content").unwrap()(
            &[CtValue::Nil, CtValue::Nil],
            &macro_env,
            &mut ct_env,
        )
        .expect_err("too many args");
        assert!(err.to_string().contains("at most 1 argument"));
    }
}
