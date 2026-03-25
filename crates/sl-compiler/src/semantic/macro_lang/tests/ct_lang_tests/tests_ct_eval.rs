//! Unit tests for compile-time eval (CtExpr/CtStmt evaluation) and core builtin functions
//!
//! This file was split from the monolithic `tests.rs` as part of the Round 1 clean-code refactoring.

#![allow(unused_imports)]
use super::{empty_expand_env, empty_macro_env};
use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::macro_lang::ast::{CtBlock, CtExpr, CtStmt, CtValue};
use crate::semantic::macro_lang::{builtins::BuiltinRegistry, env::CtEnv, eval::eval_block};
use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

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

    let mut macro_env = empty_macro_env();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let result = eval_block(
        &block,
        &mut macro_env,
        &mut ct_env,
        &builtins,
        &mut expand_env,
    )
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

    let mut macro_env = empty_macro_env();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let result = eval_block(
        &block,
        &mut macro_env,
        &mut ct_env,
        &builtins,
        &mut expand_env,
    )
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

    let mut macro_env = empty_macro_env();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let result = eval_block(
        &block,
        &mut macro_env,
        &mut ct_env,
        &builtins,
        &mut expand_env,
    )
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

    let mut macro_env = empty_macro_env();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let result = eval_block(
        &block,
        &mut macro_env,
        &mut ct_env,
        &builtins,
        &mut expand_env,
    );
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

    let mut macro_env = empty_macro_env();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let result = eval_block(
        &block,
        &mut macro_env,
        &mut ct_env,
        &builtins,
        &mut expand_env,
    )
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
fn ct_value_list_preserves_structure_across_macro_value_bridge() {
    // Regression test: CtValue::List must NOT degrade to MacroValue::Keyword
    // when crossing the ct_value_to_macro_value bridge.
    use crate::semantic::expand::macro_values::MacroValue;
    use crate::semantic::macro_lang::eval::{ct_value_to_macro_value, macro_value_to_ct_value};

    // Simple list
    let list = CtValue::List(vec![CtValue::Int(1), CtValue::String("a".to_string())]);
    let mv = ct_value_to_macro_value(&list);
    assert!(
        matches!(mv, MacroValue::List(_)),
        "CtValue::List must convert to MacroValue::List, got {:?}",
        mv
    );

    // Nested list
    let nested = CtValue::List(vec![
        CtValue::Int(1),
        CtValue::List(vec![CtValue::String("inner".to_string())]),
    ]);
    let mv_nested = ct_value_to_macro_value(&nested);
    assert!(matches!(mv_nested, MacroValue::List(_)));

    // Round-trip: CtValue -> MacroValue -> CtValue preserves type and content
    let original = CtValue::List(vec![
        CtValue::Bool(true),
        CtValue::Keyword(vec![("k".to_string(), CtValue::Int(42))]),
    ]);
    let round_tripped = macro_value_to_ct_value(&ct_value_to_macro_value(&original));
    assert_eq!(original, round_tripped, "Round-trip must preserve value");
}

#[test]
fn ct_value_keyword_preserves_structure_across_macro_value_bridge() {
    // Regression test: CtValue::Keyword must preserve all value types across
    // the ct_value_to_macro_value / macro_value_to_ct_value bridge.
    use crate::semantic::macro_lang::eval::{ct_value_to_macro_value, macro_value_to_ct_value};

    // Simple keyword with string value
    let kw = CtValue::Keyword(vec![
        ("a".to_string(), CtValue::String("hello".to_string())),
        ("b".to_string(), CtValue::Int(42)),
    ]);
    let round_tripped = macro_value_to_ct_value(&ct_value_to_macro_value(&kw));
    assert_eq!(
        kw, round_tripped,
        "Simple keyword round-trip must preserve value"
    );

    // Keyword with nested list value
    let kw_list = CtValue::Keyword(vec![(
        "items".to_string(),
        CtValue::List(vec![CtValue::String("first".to_string()), CtValue::Int(2)]),
    )]);
    let round_tripped_list = macro_value_to_ct_value(&ct_value_to_macro_value(&kw_list));
    assert_eq!(
        kw_list, round_tripped_list,
        "Keyword with list value must round-trip preserving list"
    );

    // Keyword with nested keyword value
    let kw_nested = CtValue::Keyword(vec![(
        "nested".to_string(),
        CtValue::Keyword(vec![("x".to_string(), CtValue::Int(1))]),
    )]);
    let round_tripped_nested = macro_value_to_ct_value(&ct_value_to_macro_value(&kw_nested));
    assert_eq!(
        kw_nested, round_tripped_nested,
        "Keyword with nested keyword must round-trip preserving nesting"
    );

    // Keyword with bool value
    let kw_bool = CtValue::Keyword(vec![("flag".to_string(), CtValue::Bool(true))]);
    let round_tripped_bool = macro_value_to_ct_value(&ct_value_to_macro_value(&kw_bool));
    assert_eq!(
        kw_bool, round_tripped_bool,
        "Keyword with bool must round-trip"
    );

    // Keyword with nil value
    let kw_nil = CtValue::Keyword(vec![("empty".to_string(), CtValue::Nil)]);
    let round_tripped_nil = macro_value_to_ct_value(&ct_value_to_macro_value(&kw_nil));
    assert_eq!(
        kw_nil, round_tripped_nil,
        "Keyword with nil must round-trip"
    );
}

#[test]
fn builtin_keyword_attr_preserves_nested_types() {
    // Regression test: builtin_keyword_attr must not degrade nested values
    // to strings when converting MacroValue::Keyword to CtValue::Keyword.
    use crate::semantic::expand::macro_values::MacroValue;

    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();

    // Case 1: keyword with nested list value
    let mut macro_env = MacroEnv::default();
    macro_env.locals.insert(
        "opts".to_string(),
        MacroValue::Keyword(vec![(
            "items".to_string(),
            MacroValue::List(vec![
                MacroValue::String("a".to_string()),
                MacroValue::Int(1),
            ]),
        )]),
    );

    let mut ct_env = CtEnv::new();
    let result = builtins.get("keyword_attr").unwrap()(
        &[CtValue::String("opts".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("keyword_attr should succeed");

    let expected = CtValue::Keyword(vec![(
        "items".to_string(),
        CtValue::List(vec![CtValue::String("a".to_string()), CtValue::Int(1)]),
    )]);
    assert_eq!(
        result, expected,
        "keyword_attr must preserve list values, got {:?}",
        result
    );

    // Case 2: keyword with nested keyword value
    let mut macro_env2 = MacroEnv::default();
    macro_env2.locals.insert(
        "opts".to_string(),
        MacroValue::Keyword(vec![(
            "nested".to_string(),
            MacroValue::Keyword(vec![("x".to_string(), MacroValue::Int(99))]),
        )]),
    );

    let result2 = builtins.get("keyword_attr").unwrap()(
        &[CtValue::String("opts".to_string())],
        &mut macro_env2,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("keyword_attr nested keyword should succeed");

    let expected2 = CtValue::Keyword(vec![(
        "nested".to_string(),
        CtValue::Keyword(vec![("x".to_string(), CtValue::Int(99))]),
    )]);
    assert_eq!(
        result2, expected2,
        "keyword_attr must preserve nested keyword values, got {:?}",
        result2
    );

    // Case 3: keyword with bool value
    let mut macro_env3 = MacroEnv::default();
    macro_env3.locals.insert(
        "opts".to_string(),
        MacroValue::Keyword(vec![("flag".to_string(), MacroValue::Bool(true))]),
    );

    let result3 = builtins.get("keyword_attr").unwrap()(
        &[CtValue::String("opts".to_string())],
        &mut macro_env3,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("keyword_attr bool should succeed");

    let expected3 = CtValue::Keyword(vec![("flag".to_string(), CtValue::Bool(true))]);
    assert_eq!(
        result3, expected3,
        "keyword_attr must preserve bool values, got {:?}",
        result3
    );
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

    let mut macro_env = empty_macro_env();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let result = eval_block(
        &block,
        &mut macro_env,
        &mut ct_env,
        &builtins,
        &mut expand_env,
    )
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("attr should succeed");
    assert_eq!(result, CtValue::String("test".to_string()));

    // Error: wrong arg count
    let err =
        builtins.get("attr").unwrap()(&[], &mut macro_env, &mut ct_env, &mut expand_env, &builtins)
            .expect_err("wrong arg count");
    assert!(err.to_string().contains("requires exactly 1 argument"));

    // Error: wrong type
    let err = builtins.get("attr").unwrap()(
        &[CtValue::Int(1)],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type");
    assert!(err.to_string().contains("must be string"));

    // Error: missing attribute
    let err = builtins.get("attr").unwrap()(
        &[CtValue::String("missing".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("has_attr should succeed");
    assert_eq!(result, CtValue::Bool(true));

    let result = builtins.get("has_attr").unwrap()(
        &[CtValue::String("missing".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("has_attr missing");
    assert_eq!(result, CtValue::Bool(false));

    // Error: wrong arg count
    let err = builtins.get("has_attr").unwrap()(
        &[],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong arg count");
    assert!(err.to_string().contains("requires exactly 1 argument"));
}

#[test]
fn builtin_parse_bool_and_int_work() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    // parse_bool
    let result = builtins.get("parse_bool").expect("parse_bool exists")(
        &[CtValue::String("true".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("parse_bool true");
    assert_eq!(result, CtValue::Bool(true));

    let result = builtins.get("parse_bool").unwrap()(
        &[CtValue::String("false".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("parse_bool false");
    assert_eq!(result, CtValue::Bool(false));

    let err = builtins.get("parse_bool").unwrap()(
        &[CtValue::String("invalid".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("parse_bool invalid");
    assert!(err.to_string().contains("cannot parse"));

    // parse_int
    let result = builtins.get("parse_int").expect("parse_int exists")(
        &[CtValue::String("42".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("parse_int 42");
    assert_eq!(result, CtValue::Int(42));

    let err = builtins.get("parse_int").unwrap()(
        &[CtValue::String("abc".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("parse_int invalid");
    assert!(err.to_string().contains("cannot parse"));
}

#[test]
fn builtin_keyword_and_list_operations() {
    let mut macro_env = MacroEnv::default();
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("keyword_get");
    assert_eq!(result, CtValue::String("test".to_string()));

    let err = builtins.get("keyword_get").unwrap()(
        &[keyword.clone(), CtValue::String("missing".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("keyword_get missing");
    assert!(err.to_string().contains("not found"));

    // keyword_has
    let result = builtins.get("keyword_has").expect("keyword_has exists")(
        &[keyword.clone(), CtValue::String("name".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("keyword_has");
    assert_eq!(result, CtValue::Bool(true));

    // list_length
    let result = builtins.get("list_length").expect("list_length exists")(
        &[keyword],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("list_length keyword");
    assert_eq!(result, CtValue::Int(2));

    let list = CtValue::List(vec![CtValue::Nil, CtValue::Nil, CtValue::Nil]);
    let result = builtins.get("list_length").unwrap()(
        &[list],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("list_length list");
    assert_eq!(result, CtValue::Int(3));
}

#[test]
fn builtin_to_string_works() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let result = builtins.get("to_string").expect("to_string exists")(
        &[CtValue::Bool(true)],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("to_string bool");
    assert_eq!(result, CtValue::String("true".to_string()));

    let result = builtins.get("to_string").unwrap()(
        &[CtValue::Int(123)],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("to_string int");
    assert_eq!(result, CtValue::String("123".to_string()));

    let result = builtins.get("to_string").unwrap()(
        &[CtValue::Nil],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("to_string nil");
    assert_eq!(result, CtValue::String("nil".to_string()));
}

#[test]
fn builtin_content_works() {
    use sl_core::FormItem;

    let mut macro_env = MacroEnv {
        content: vec![FormItem::Text("test content".to_string())],
        ..Default::default()
    };
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let result = builtins.get("content").expect("content exists")(
        &[],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("content");
    assert!(matches!(result, CtValue::Ast(items) if items.len() == 1));

    // Error: too many args
    let err = builtins.get("content").unwrap()(
        &[CtValue::Nil, CtValue::Nil],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
        macro_name: "my_macro".to_string(),
        source_file: Some("main.xml".to_string()),
        line: Some(10),
        column: Some(5),
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
            // New Step 4.2 fields
            assert!(matches!(
                map.get("macro_name"),
                Some(CtValue::String(s)) if s == "my_macro"
            ));
            assert!(matches!(
                map.get("file"),
                Some(CtValue::String(s)) if s == "main.xml"
            ));
            assert!(matches!(map.get("line"), Some(CtValue::Int(10))));
            assert!(matches!(map.get("column"), Some(CtValue::Int(5))));
        }
        other => panic!("expected keyword, got {}", other.type_name()),
    }

    // Verify default (no source location)
    let mut empty_macro_env = MacroEnv::default();
    let result = builtins.get("caller_env").expect("caller_env exists")(
        &[],
        &mut empty_macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("caller_env should succeed");
    match result {
        CtValue::Keyword(items) => {
            let map: std::collections::BTreeMap<_, _> = items.into_iter().collect();
            assert!(
                !map.contains_key("macro_name"),
                "empty macro_name should not be exposed"
            );
            assert!(
                !map.contains_key("file"),
                "source_file should not be exposed when None"
            );
            assert!(
                !map.contains_key("line"),
                "line should not be exposed when None"
            );
            assert!(
                !map.contains_key("column"),
                "column should not be exposed when None"
            );
        }
        other => panic!("expected keyword, got {}", other.type_name()),
    }

    // Error: too many args
    let err = builtins.get("caller_env").unwrap()(
        &[CtValue::Nil],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("expand alias");
    assert_eq!(result, CtValue::String("helper".to_string()));

    // Unknown name returns as-is
    let result = builtins.get("expand_alias").unwrap()(
        &[CtValue::String("unknown".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("unknown alias");
    assert_eq!(result, CtValue::String("unknown".to_string()));

    // ModuleRef also works
    let result = builtins.get("expand_alias").unwrap()(
        &[CtValue::ModuleRef("mh".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("module ref");
    assert_eq!(result, CtValue::String("main.helper".to_string()));

    // Error: wrong arg count
    let err = builtins.get("expand_alias").unwrap()(
        &[],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong args");
    assert!(err.to_string().contains("requires exactly 1"));
}

#[test]
fn builtin_require_module_adds_to_expand_env() {
    let mut macro_env = MacroEnv {
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("require_module");
    // Returns the expanded module name (helper has no alias, so returns "helper")
    assert_eq!(result, CtValue::String("helper".to_string()));

    // Verify it was added
    assert!(expand_env.module.requires.contains(&"helper".to_string()));

    // Idempotent: calling again doesn't panic
    let result = builtins.get("require_module").unwrap()(
        &[CtValue::String("helper".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env2,
        &builtins,
    )
    .expect("define_import with alias");
    assert_eq!(result, CtValue::Nil);
    assert!(expand_env2.module.imports.contains(&"kernel".to_string()));
}

#[test]
fn builtin_define_alias_adds_alias() {
    let mut macro_env = MacroEnv::default();

    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let result = builtins.get("define_alias").expect("define_alias exists")(
        &[
            CtValue::String("helper".to_string()),
            CtValue::String("h".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
    let mut macro_env = MacroEnv::default();

    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let result = builtins
        .get("define_require")
        .expect("define_require exists")(
        &[CtValue::String("helper".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("define_require");
    assert_eq!(result, CtValue::Nil);
    assert!(expand_env.module.requires.contains(&"helper".to_string()));
}

#[test]
fn builtin_invoke_macro_rejects_unrequired_module() {
    let mut macro_env = MacroEnv {
        current_module: Some("main".to_string()),
        ..Default::default()
    };
    // Register helper module so it exists (but is not required)
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();
    expand_env.program.register_module_for_test("helper");
    // Note: "helper" is registered but NOT in requires

    let err = builtins.get("invoke_macro").expect("invoke_macro exists")(
        &[
            CtValue::String("helper".to_string()),
            CtValue::String("some_macro".to_string()),
            CtValue::Keyword(vec![]),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should fail");
    // Module exists but is not in scope (not required)
    assert!(err.to_string().contains("not in scope"));
    assert!(err.to_string().contains("require"));
}

#[test]
fn builtin_invoke_macro_rejects_unknown_macro() {
    let mut macro_env = MacroEnv {
        current_module: Some("main".to_string()),
        ..Default::default()
    };
    macro_env.requires.push("helper".to_string());

    // Register the helper module in expand_env (empty - no macros)
    let mut expand_env = empty_expand_env();
    expand_env.program.register_module_for_test("helper");
    // Note: no macros registered in helper

    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();

    let err = builtins.get("invoke_macro").unwrap()(
        &[
            CtValue::String("helper".to_string()),
            CtValue::String("nonexistent".to_string()),
            CtValue::Keyword(vec![]),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should fail");
    // Module exists but macro is not defined in it
    assert!(err.to_string().contains("is not defined"));
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
    fn make_form_item(meta: &FormMeta, head: &str, fields: Vec<FormField>) -> sl_core::FormItem {
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
            body: macro_body,
            meta: Default::default(),
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("invoke_macro should succeed");

    match result {
        CtValue::Ast(items) => {
            assert!(!items.is_empty(), "should produce AST items");
        }
        other => panic!("expected Ast, got {}", other.type_name()),
    }
}
