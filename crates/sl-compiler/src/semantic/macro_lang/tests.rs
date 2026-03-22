//! Unit tests for the compile-time macro language.

#[cfg(test)]
mod tests {
    use crate::semantic::macro_lang::*;
    use crate::semantic::expand::macro_env::MacroEnv;
    use sl_core::FormItem;

    fn empty_macro_env() -> MacroEnv {
        MacroEnv::default()
    }

    #[test]
    fn compile_time_if_selects_correct_branch() {
        let block = CtBlock {
            stmts: vec![
                CtStmt::If {
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
}
