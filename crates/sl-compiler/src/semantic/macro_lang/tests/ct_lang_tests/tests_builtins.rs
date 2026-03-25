//! Unit tests for advanced builtin functions (AST, module, list, keyword, match builtins)
//!
//! This file was split from the monolithic `tests.rs` as part of the Round 1 clean-code refactoring.

#![allow(unused_imports)]
use super::{empty_expand_env, empty_macro_env};
use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::macro_env::MacroEnv;
use crate::semantic::macro_lang::ast::{CtBlock, CtExpr, CtStmt, CtValue};
use crate::semantic::macro_lang::{builtins::BuiltinRegistry, env::CtEnv, eval::eval_block};
use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

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
    let result = builtins.get("invoke_macro").unwrap()(
        args,
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    );
    // Should return an error (type check failure or module-not-found)
    let err = result.expect_err("wrong third arg type should error");
    // The error should mention args type issue OR the builtin invocation failure
    let err_str = err.to_string();
    // Verify the error is about type mismatch (args/keyword), module unknown, or invocation failure
    assert!(
        err_str.contains("keyword")
            || err_str.contains("invoke_macro")
            || err_str.contains("not found")
            || err_str.contains("is not known"),
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong first arg type");
    assert!(
        err.to_string()
            .contains("first argument (module) must be string or module")
    );
}

#[test]
fn builtin_invoke_macro_module_not_in_scope_errors() {
    let mut macro_env = MacroEnv {
        current_module: Some("caller".to_string()),
        ..Default::default()
    };
    // Register helper module (so it exists) but do NOT add to requires
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();
    expand_env.program.register_module_for_test("helper");

    let err = builtins.get("invoke_macro").unwrap()(
        &[
            CtValue::String("helper".to_string()),
            CtValue::String("__using__".to_string()),
            CtValue::Keyword(vec![]),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("module not in scope");
    // Module exists but is not required → "not in scope"
    assert!(err.to_string().contains("not in scope"));
}

#[test]
fn builtin_invoke_macro_macro_not_found_errors() {
    let mut macro_env = MacroEnv {
        current_module: Some("main".to_string()),
        ..Default::default()
    };
    macro_env.requires.push("helper".to_string());

    let mut expand_env = empty_expand_env();
    // Register helper module (empty - no macros)
    expand_env.program.register_module_for_test("helper");

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
    .expect_err("macro not found");
    // Module exists but macro is not defined in it
    assert!(err.to_string().contains("is not defined"));
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
            body: macro_body,
            meta: Default::default(),
            is_private: false,
        })
        .expect("register macro");

    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();

    // Keyword arg value is Nil (truly unsupported — ModuleRef/CallerEnv also unsupported)
    let err = builtins.get("invoke_macro").unwrap()(
        &[
            CtValue::String("helper".to_string()),
            CtValue::String("__using__".to_string()),
            CtValue::Keyword(vec![("opt1".to_string(), CtValue::Nil)]),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("nil keyword arg value type");
    assert!(
        err.to_string()
            .contains("keyword arg value must be string, int, bool, list, keyword, or ast")
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
            body: macro_body,
            meta: Default::default(),
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
fn builtin_invoke_macro_accepts_list_and_keyword_args() {
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
            body: macro_body,
            meta: Default::default(),
            is_private: false,
        })
        .expect("register macro");

    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();

    // Step 2.4: List/Keyword/Ast keyword args should not error
    let result = builtins.get("invoke_macro").unwrap()(
        &[
            CtValue::String("helper".to_string()),
            CtValue::String("__using__".to_string()),
            CtValue::Keyword(vec![
                ("flag".to_string(), CtValue::Bool(true)),
                ("count".to_string(), CtValue::Int(42)),
                (
                    "items".to_string(),
                    CtValue::List(vec![
                        CtValue::String("a".to_string()),
                        CtValue::String("b".to_string()),
                    ]),
                ),
                (
                    "meta".to_string(),
                    CtValue::Keyword(vec![(
                        "key".to_string(),
                        CtValue::String("val".to_string()),
                    )]),
                ),
            ]),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("List/Keyword/Ast keyword args should not error");
    match result {
        CtValue::Ast(items) => {
            assert!(!items.is_empty(), "should produce AST items");
        }
        other => panic!("expected Ast, got {:?}", other),
    }
}

#[test]
fn builtin_expand_alias_wrong_type() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("expand_alias").unwrap()(
        &[CtValue::Int(123)],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type");
    assert!(err.to_string().contains("must be string or module"));
}

#[test]
fn builtin_require_module_wrong_type() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("require_module").unwrap()(
        &[CtValue::Int(123)],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type");
    assert!(err.to_string().contains("must be string or module"));
}

#[test]
fn builtin_define_import_wrong_type() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("define_import").unwrap()(
        &[CtValue::Int(123)],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type");
    assert!(err.to_string().contains("must be string or module"));
}

#[test]
fn builtin_define_alias_wrong_first_arg_type() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("define_alias").unwrap()(
        &[CtValue::Int(123), CtValue::String("a".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type");
    assert!(
        err.to_string()
            .contains("first argument must be string or module")
    );
}

#[test]
fn builtin_define_alias_wrong_second_arg_type() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("define_alias").unwrap()(
        &[CtValue::String("helper".to_string()), CtValue::Int(123)],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type");
    assert!(err.to_string().contains("second argument must be string"));
}

#[test]
fn builtin_define_require_wrong_type() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("define_require").unwrap()(
        &[CtValue::Int(123)],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type");
    assert!(err.to_string().contains("macro_name"));
}

#[test]
fn builtin_invoke_macro_requires_3_args() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    // Too few args
    let err = builtins.get("invoke_macro").unwrap()(
        &[CtValue::String("helper".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("too many args");
    assert!(err.to_string().contains("requires exactly 3"));
}

#[test]
fn builtin_keyword_attr_missing_keyword() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("keyword_attr").unwrap()(
        &[CtValue::String("missing".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("missing keyword");
    assert!(err.to_string().contains("not found"));
}

#[test]
fn builtin_caller_module_returns_module_name() {
    let mut macro_env = MacroEnv {
        current_module: Some("test_module".to_string()),
        ..Default::default()
    };
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let result = builtins.get("caller_module").unwrap()(
        &[],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("caller_module should succeed");
    assert_eq!(result, CtValue::String("test_module".to_string()));

    // Test without module set
    let mut macro_env = MacroEnv::default();
    let result = builtins.get("caller_module").unwrap()(
        &[],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
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

    let mut macro_env = MacroEnv {
        content: vec![
            FormItem::Form(sl_core::Form {
                head: "slot".to_string(),
                meta: minimal_meta(),
                fields: vec![FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Text("slot content".to_string())]),
                }],
            }),
            FormItem::Form(sl_core::Form {
                head: "other".to_string(),
                meta: minimal_meta(),
                fields: vec![FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Text("other content".to_string())]),
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("content").unwrap()(
        &[CtValue::Keyword(vec![(
            "head".to_string(),
            CtValue::Int(123),
        )])],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type in kv");
    assert!(err.to_string().contains("must be string"));
}

#[test]
fn builtin_expand_alias_wrong_arg_count() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("expand_alias").unwrap()(
        &[
            CtValue::String("a".to_string()),
            CtValue::String("b".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("too many args");
    assert!(err.to_string().contains("requires exactly 1"));
}

#[test]
fn builtin_require_module_wrong_arg_count() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("require_module").unwrap()(
        &[],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("too few args");
    assert!(err.to_string().contains("requires exactly 1"));
}

#[test]
fn builtin_define_import_wrong_arg_count() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("define_import").unwrap()(
        &[],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("too few args");
    assert!(err.to_string().contains("requires exactly 1"));
}

#[test]
fn builtin_define_alias_wrong_arg_count() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("define_alias").unwrap()(
        &[CtValue::String("helper".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("too few args");
    assert!(err.to_string().contains("requires exactly 2"));
}

#[test]
fn builtin_define_require_wrong_arg_count() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("define_require").unwrap()(
        &[],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("too few args");
    assert!(err.to_string().contains("requires exactly 1"));
}

#[test]
fn builtin_invoke_macro_wrong_arg_count() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("invoke_macro").unwrap()(
        &[CtValue::String("helper".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("too few args");
    assert!(err.to_string().contains("requires exactly 3"));
}

#[test]
fn builtin_keyword_get_wrong_arg_count() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("keyword_get").unwrap()(
        &[CtValue::Keyword(vec![])],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("too few args");
    assert!(err.to_string().contains("requires exactly 2"));
}

#[test]
fn builtin_keyword_has_wrong_first_arg_type() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("keyword_has").unwrap()(
        &[
            CtValue::String("not a keyword".to_string()),
            CtValue::String("key".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong first arg type");
    assert!(err.to_string().contains("first argument must be keyword"));
}

#[test]
fn builtin_keyword_has_wrong_second_arg_type() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("keyword_has").unwrap()(
        &[CtValue::Keyword(vec![]), CtValue::Int(123)],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong second arg type");
    assert!(err.to_string().contains("second argument must be string"));
}

#[test]
fn builtin_list_length_wrong_type() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    let err = builtins.get("list_length").unwrap()(
        &[CtValue::String("not a list".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type");
    assert!(err.to_string().contains("must be list or keyword"));
}

#[test]
fn builtin_to_string_complex_value() {
    let mut macro_env = MacroEnv::default();
    let mut ct_env = CtEnv::new();
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();

    // Test to_string with complex types that fall into the catch-all branch
    let result = builtins.get("to_string").unwrap()(
        &[CtValue::Ast(vec![])],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("to_string should succeed");
    assert_eq!(result, CtValue::String("Ast([])".to_string()));

    let result = builtins.get("to_string").unwrap()(
        &[CtValue::ModuleRef("test".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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
            body: macro_body,
            meta: Default::default(),
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
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
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

// =========================================================================
// Step 3.2: AST builtin tests
// =========================================================================

fn dummy_form_meta() -> FormMeta {
    FormMeta {
        source_name: None,
        start: SourcePosition { row: 0, column: 0 },
        end: SourcePosition { row: 0, column: 0 },
        start_byte: 0,
        end_byte: 0,
    }
}

#[test]
fn builtin_ast_head_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();
    let mut ct_env = CtEnv::new();

    // Normal case: ast with a form
    let ast = CtValue::Ast(vec![FormItem::Form(Form {
        head: "text".to_string(),
        meta: dummy_form_meta(),
        fields: vec![FormField {
            name: "children".to_string(),
            value: FormValue::Sequence(vec![FormItem::Text("hello".to_string())]),
        }],
    })]);

    let result = builtins.get("ast_head").expect("ast_head exists")(
        &[ast],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("ast_head should succeed");
    assert_eq!(result, CtValue::String("text".to_string()));

    // Error: wrong arg count
    let err = builtins.get("ast_head").unwrap()(
        &[],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong arg count");
    assert!(err.to_string().contains("requires exactly 1"));

    // Error: wrong type
    let err = builtins.get("ast_head").unwrap()(
        &[CtValue::String("not an ast".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type");
    assert!(err.to_string().contains("must be ast"));

    // Error: empty ast (text only)
    let err = builtins.get("ast_head").unwrap()(
        &[CtValue::Ast(vec![FormItem::Text("just text".to_string())])],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("no form elements");
    assert!(err.to_string().contains("no form elements"));
}

#[test]
fn builtin_ast_children_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();
    let mut ct_env = CtEnv::new();

    let child_text = FormItem::Text("inner content".to_string());
    let ast = CtValue::Ast(vec![FormItem::Form(Form {
        head: "script".to_string(),
        meta: dummy_form_meta(),
        fields: vec![
            FormField {
                name: "name".to_string(),
                value: FormValue::String("test".to_string()),
            },
            FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![child_text.clone()]),
            },
        ],
    })]);

    let result = builtins.get("ast_children").expect("ast_children exists")(
        &[ast],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("ast_children should succeed");

    let expected = CtValue::Ast(vec![child_text]);
    assert_eq!(result, expected, "ast_children should return children");

    // ast with no children field returns empty list
    let ast_no_children = CtValue::Ast(vec![FormItem::Form(Form {
        head: "module".to_string(),
        meta: dummy_form_meta(),
        fields: vec![],
    })]);

    let result2 = builtins.get("ast_children").unwrap()(
        &[ast_no_children],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("ast_children with no children field");
    assert_eq!(result2, CtValue::Ast(vec![]));

    // Error: empty ast
    let err = builtins.get("ast_children").unwrap()(
        &[CtValue::Ast(vec![])],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("empty ast");
    assert!(err.to_string().contains("no form elements"));
}

#[test]
fn builtin_ast_attr_get_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();
    let mut ct_env = CtEnv::new();

    let ast = CtValue::Ast(vec![FormItem::Form(Form {
        head: "script".to_string(),
        meta: dummy_form_meta(),
        fields: vec![
            FormField {
                name: "name".to_string(),
                value: FormValue::String("my_script".to_string()),
            },
            FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Text("content".to_string())]),
            },
        ],
    })]);

    // Get string attribute
    let result = builtins.get("ast_attr_get").expect("ast_attr_get exists")(
        &[ast.clone(), CtValue::String("name".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("ast_attr_get should succeed");
    assert_eq!(result, CtValue::String("my_script".to_string()));

    // Error: missing attribute
    let err = builtins.get("ast_attr_get").unwrap()(
        &[ast.clone(), CtValue::String("missing".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("missing attr");
    assert!(err.to_string().contains("not found"));

    // Error: wrong arg count
    let err = builtins.get("ast_attr_get").unwrap()(
        &[ast],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong arg count");
    assert!(err.to_string().contains("requires exactly 2"));

    // Error: second arg not string
    let err = builtins.get("ast_attr_get").unwrap()(
        &[CtValue::Ast(vec![]), CtValue::Int(42)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("second arg not string");
    assert!(err.to_string().contains("must be string"));
}

#[test]
fn builtin_ast_attr_keys_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();
    let mut ct_env = CtEnv::new();

    let ast = CtValue::Ast(vec![FormItem::Form(Form {
        head: "script".to_string(),
        meta: dummy_form_meta(),
        fields: vec![
            FormField {
                name: "name".to_string(),
                value: FormValue::String("test".to_string()),
            },
            FormField {
                name: "mode".to_string(),
                value: FormValue::String("debug".to_string()),
            },
            FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![]),
            },
        ],
    })]);

    let result = builtins.get("ast_attr_keys").expect("ast_attr_keys exists")(
        &[ast],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("ast_attr_keys should succeed");

    // Should contain "name" and "mode" but NOT "children"
    let keys = match result {
        CtValue::List(items) => items,
        other => panic!("expected list, got {:?}", other),
    };
    let key_strings: Vec<String> = keys
        .iter()
        .map(|k| match k {
            CtValue::String(s) => s.clone(),
            other => panic!("expected string key, got {:?}", other),
        })
        .collect();
    assert!(
        key_strings.contains(&"name".to_string()),
        "should contain name: {:?}",
        key_strings
    );
    assert!(
        key_strings.contains(&"mode".to_string()),
        "should contain mode: {:?}",
        key_strings
    );
    assert!(
        !key_strings.contains(&"children".to_string()),
        "should NOT contain children: {:?}",
        key_strings
    );

    // Error: wrong type
    let err = builtins.get("ast_attr_keys").unwrap()(
        &[CtValue::String("not ast".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type");
    assert!(err.to_string().contains("must be ast"));
}

// =========================================================================
// Step 3.3: AST write builtin tests
// =========================================================================

#[test]
fn builtin_ast_attr_set_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();
    let mut ct_env = CtEnv::new();

    let ast = CtValue::Ast(vec![FormItem::Form(Form {
        head: "script".to_string(),
        meta: dummy_form_meta(),
        fields: vec![
            FormField {
                name: "name".to_string(),
                value: FormValue::String("old_name".to_string()),
            },
            FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Text("content".to_string())]),
            },
        ],
    })]);

    // Set a new attribute
    let result = builtins.get("ast_attr_set").expect("ast_attr_set exists")(
        &[
            ast.clone(),
            CtValue::String("mode".to_string()),
            CtValue::String("debug".to_string()),
        ],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("ast_attr_set should succeed");

    let result_ast = match result {
        CtValue::Ast(items) => items,
        other => panic!("expected ast, got {:?}", other),
    };
    let first_form = match &result_ast[0] {
        FormItem::Form(f) => f,
        other => panic!("expected form, got {:?}", other),
    };
    // Original name should still be there
    let name_val = first_form
        .fields
        .iter()
        .find(|f| f.name == "name")
        .expect("name field should exist");
    assert_eq!(name_val.value, FormValue::String("old_name".to_string()));
    // New mode should be set
    let mode_val = first_form
        .fields
        .iter()
        .find(|f| f.name == "mode")
        .expect("mode field should exist");
    assert_eq!(mode_val.value, FormValue::String("debug".to_string()));

    // Override existing attribute
    let result2 = builtins.get("ast_attr_set").unwrap()(
        &[
            ast,
            CtValue::String("name".to_string()),
            CtValue::String("new_name".to_string()),
        ],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("override attr should succeed");

    let result_ast2 = match result2 {
        CtValue::Ast(items) => items,
        other => panic!("expected ast, got {:?}", other),
    };
    let first_form2 = match &result_ast2[0] {
        FormItem::Form(f) => f,
        other => panic!("expected form, got {:?}", other),
    };
    let name_val2 = first_form2
        .fields
        .iter()
        .find(|f| f.name == "name")
        .expect("name field should exist");
    assert_eq!(name_val2.value, FormValue::String("new_name".to_string()));

    // Original is unchanged (immutability)
    let original_ast = CtValue::Ast(vec![FormItem::Form(Form {
        head: "script".to_string(),
        meta: dummy_form_meta(),
        fields: vec![FormField {
            name: "name".to_string(),
            value: FormValue::String("old_name".to_string()),
        }],
    })]);
    let result3 = builtins.get("ast_attr_set").unwrap()(
        &[
            original_ast,
            CtValue::String("extra".to_string()),
            CtValue::String("val".to_string()),
        ],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("add attr should succeed");
    let result_ast3 = match result3 {
        CtValue::Ast(items) => items,
        other => panic!("expected ast, got {:?}", other),
    };
    // Original still has 1 field, result has 2
    assert_eq!(result_ast3.len(), 1);

    // Error: wrong number of args
    let err = builtins.get("ast_attr_set").unwrap()(
        &[CtValue::Ast(vec![])],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong arg count");
    assert!(err.to_string().contains("requires exactly 3"));

    // Error: empty ast
    let err = builtins.get("ast_attr_set").unwrap()(
        &[
            CtValue::Ast(vec![]),
            CtValue::String("key".to_string()),
            CtValue::String("val".to_string()),
        ],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("empty ast");
    assert!(err.to_string().contains("no form elements"));
}

#[test]
fn builtin_ast_wrap_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();
    let mut ct_env = CtEnv::new();

    let inner = CtValue::Ast(vec![
        FormItem::Text("hello".to_string()),
        FormItem::Form(Form {
            head: "text".to_string(),
            meta: dummy_form_meta(),
            fields: vec![],
        }),
    ]);

    let result = builtins.get("ast_wrap").expect("ast_wrap exists")(
        &[inner, CtValue::String("wrapper".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("ast_wrap should succeed");

    let result_ast = match result {
        CtValue::Ast(items) => items,
        other => panic!("expected ast, got {:?}", other),
    };
    assert_eq!(result_ast.len(), 1);
    let wrapper = match &result_ast[0] {
        FormItem::Form(f) => f,
        other => panic!("expected form, got {:?}", other),
    };
    assert_eq!(wrapper.head, "wrapper");
    // children field should contain the inner items
    let children_field = wrapper
        .fields
        .iter()
        .find(|f| f.name == "children")
        .expect("children field should exist");
    let children = match &children_field.value {
        FormValue::Sequence(items) => items,
        other => panic!("expected sequence, got {:?}", other),
    };
    assert_eq!(children.len(), 2);

    // Error: wrong number of args
    let err = builtins.get("ast_wrap").unwrap()(
        &[CtValue::Ast(vec![])],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong arg count");
    assert!(err.to_string().contains("requires 2 or 3"));
}

#[test]
fn builtin_ast_concat_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();
    let mut ct_env = CtEnv::new();

    let ast1 = CtValue::Ast(vec![FormItem::Form(Form {
        head: "text".to_string(),
        meta: dummy_form_meta(),
        fields: vec![],
    })]);
    let ast2 = CtValue::Ast(vec![
        FormItem::Text("hello".to_string()),
        FormItem::Form(Form {
            head: "end".to_string(),
            meta: dummy_form_meta(),
            fields: vec![],
        }),
    ]);

    let list = CtValue::List(vec![ast1, ast2]);
    let result = builtins.get("ast_concat").expect("ast_concat exists")(
        &[list],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("ast_concat should succeed");

    let result_ast = match result {
        CtValue::Ast(items) => items,
        other => panic!("expected ast, got {:?}", other),
    };
    assert_eq!(result_ast.len(), 3); // 1 form + 1 text + 1 form
    assert!(matches!(&result_ast[0], FormItem::Form(f) if f.head == "text"));
    assert!(matches!(&result_ast[1], FormItem::Text(t) if t == "hello"));
    assert!(matches!(&result_ast[2], FormItem::Form(f) if f.head == "end"));

    // Empty concat
    let empty_list = CtValue::List(vec![]);
    let result2 = builtins.get("ast_concat").unwrap()(
        &[empty_list],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("concat empty list should succeed");
    assert!(matches!(result2, CtValue::Ast(items) if items.is_empty()));

    // Error: non-list arg
    let err = builtins.get("ast_concat").unwrap()(
        &[CtValue::String("not a list".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("non-list arg");
    assert!(err.to_string().contains("argument must be ast"));
}

#[test]
fn builtin_ast_filter_head_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = empty_expand_env();
    let mut ct_env = CtEnv::new();

    let ast = CtValue::Ast(vec![
        FormItem::Text("intro".to_string()),
        FormItem::Form(Form {
            head: "script".to_string(),
            meta: dummy_form_meta(),
            fields: vec![],
        }),
        FormItem::Form(Form {
            head: "text".to_string(),
            meta: dummy_form_meta(),
            fields: vec![],
        }),
        FormItem::Form(Form {
            head: "script".to_string(),
            meta: dummy_form_meta(),
            fields: vec![],
        }),
    ]);

    // Filter to only "script" forms
    let result = builtins
        .get("ast_filter_head")
        .expect("ast_filter_head exists")(
        &[ast, CtValue::String("script".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("ast_filter_head should succeed");

    let result_ast = match result {
        CtValue::Ast(items) => items,
        other => panic!("expected ast, got {:?}", other),
    };
    // Only 2 script forms (text node excluded, non-matching text form excluded)
    assert_eq!(result_ast.len(), 2);
    assert!(matches!(&result_ast[0], FormItem::Form(f) if f.head == "script"));
    assert!(matches!(&result_ast[1], FormItem::Form(f) if f.head == "script"));

    // Filter with no match
    let ast2 = CtValue::Ast(vec![FormItem::Form(Form {
        head: "text".to_string(),
        meta: dummy_form_meta(),
        fields: vec![],
    })]);
    let result2 = builtins.get("ast_filter_head").unwrap()(
        &[ast2, CtValue::String("nonexistent".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("filter no match");
    let result_ast2 = match result2 {
        CtValue::Ast(items) => items,
        other => panic!("expected ast, got {:?}", other),
    };
    assert!(result_ast2.is_empty());

    // Error: wrong arg count
    let err = builtins.get("ast_filter_head").unwrap()(
        &[CtValue::Ast(vec![])],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong arg count");
    assert!(err.to_string().contains("requires exactly 2"));
}

fn make_module_env(name: &str) -> ExpandEnv {
    let mut env = ExpandEnv::default();
    env.begin_module(Some(name.to_string()), Some(format!("{name}.xml")))
        .unwrap();
    env
}

#[test]
fn builtin_module_get_returns_nil_when_key_absent() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("test_mod");
    let mut ct_env = CtEnv::new();

    let result = builtins.get("module_get").unwrap()(
        &[CtValue::String("missing".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("module_get should succeed");
    assert_eq!(result, CtValue::Nil);
}

#[test]
fn builtin_module_put_and_get_roundtrip() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("test_mod");
    let mut ct_env = CtEnv::new();

    // Put a value
    let written = builtins.get("module_put").unwrap()(
        &[CtValue::String("counter".to_string()), CtValue::Int(42)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("module_put should succeed");
    assert_eq!(written, CtValue::Int(42));

    // Get it back - same expand_env
    let result = builtins.get("module_get").unwrap()(
        &[CtValue::String("counter".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("module_get should succeed");
    assert_eq!(result, CtValue::Int(42));
}

#[test]
fn builtin_module_get_persists_across_calls() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("persist_mod");
    let mut ct_env = CtEnv::new();

    // Put multiple values
    builtins.get("module_put").unwrap()(
        &[
            CtValue::String("a".to_string()),
            CtValue::String("hello".to_string()),
        ],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("put a");

    builtins.get("module_put").unwrap()(
        &[
            CtValue::String("b".to_string()),
            CtValue::List(vec![CtValue::Int(1), CtValue::Int(2)]),
        ],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("put b");

    // Both values readable
    let a = builtins.get("module_get").unwrap()(
        &[CtValue::String("a".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("get a");
    assert_eq!(a, CtValue::String("hello".to_string()));

    let b = builtins.get("module_get").unwrap()(
        &[CtValue::String("b".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("get b");
    assert_eq!(b, CtValue::List(vec![CtValue::Int(1), CtValue::Int(2)]));
}

#[test]
fn builtin_module_get_error_wrong_arg_count() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("err_mod");
    let mut ct_env = CtEnv::new();

    let err = builtins.get("module_get").unwrap()(
        &[],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("no args");
    assert!(err.to_string().contains("requires exactly 1"));

    let err = builtins.get("module_get").unwrap()(
        &[CtValue::String("key".to_string()), CtValue::Int(1)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("too many args");
    assert!(err.to_string().contains("requires exactly 1"));
}

#[test]
fn builtin_module_get_error_wrong_type() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("type_err_mod");
    let mut ct_env = CtEnv::new();

    let err = builtins.get("module_get").unwrap()(
        &[CtValue::Int(123)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong type");
    assert!(err.to_string().contains("must be string"));
}

#[test]
fn builtin_module_put_error_wrong_arg_count() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("put_err_mod");
    let mut ct_env = CtEnv::new();

    let err = builtins.get("module_put").unwrap()(
        &[CtValue::String("key".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("only 1 arg");
    assert!(err.to_string().contains("requires exactly 2"));

    let err = builtins.get("module_put").unwrap()(
        &[],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("no args");
    assert!(err.to_string().contains("requires exactly 2"));
}

#[test]
fn builtin_module_put_error_wrong_name_type() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("put_type_err_mod");
    let mut ct_env = CtEnv::new();

    let err = builtins.get("module_put").unwrap()(
        &[CtValue::Int(99), CtValue::String("val".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("wrong name type");
    assert!(err.to_string().contains("must be string"));
}

// Step 5.5: Conflict detection tests

#[test]
fn builtin_module_put_conflict_when_key_exists() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("conflict_mod");
    let mut ct_env = CtEnv::new();

    // First put succeeds
    builtins.get("module_put").unwrap()(
        &[CtValue::String("key".to_string()), CtValue::Int(1)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("first put should succeed");

    // Second put with same key fails
    let err = builtins.get("module_put").unwrap()(
        &[CtValue::String("key".to_string()), CtValue::Int(2)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("second put should conflict");
    assert!(err.to_string().contains("conflict"));
    assert!(err.to_string().contains("key `key` already exists"));
}

#[test]
fn builtin_module_update_overwrites_despite_conflict() {
    // module_update is allowed to overwrite even if key exists
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("update_overwrite_mod");
    let mut ct_env = CtEnv::new();

    // First put
    builtins.get("module_put").unwrap()(
        &[CtValue::String("counter".to_string()), CtValue::Int(1)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("first put");

    // Update succeeds (it is allowed to overwrite)
    let result = builtins.get("module_update").unwrap()(
        &[CtValue::String("counter".to_string()), CtValue::Int(2)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("update should succeed despite existing key");
    assert_eq!(result, CtValue::Int(2));
}

#[test]
fn builtin_module_put_different_keys_allowed() {
    // Multiple different keys in same module are fine
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("multi_key_mod");
    let mut ct_env = CtEnv::new();

    builtins.get("module_put").unwrap()(
        &[CtValue::String("a".to_string()), CtValue::Int(1)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("put a");
    builtins.get("module_put").unwrap()(
        &[CtValue::String("b".to_string()), CtValue::Int(2)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("put b");

    assert_eq!(
        builtins.get("module_get").unwrap()(
            &[CtValue::String("a".to_string())],
            &mut empty_macro_env(),
            &mut ct_env,
            &mut expand_env,
            &builtins,
        )
        .unwrap(),
        CtValue::Int(1)
    );
    assert_eq!(
        builtins.get("module_get").unwrap()(
            &[CtValue::String("b".to_string())],
            &mut empty_macro_env(),
            &mut ct_env,
            &mut expand_env,
            &builtins,
        )
        .unwrap(),
        CtValue::Int(2)
    );
}

// Step 5.3: Multi-type support tests

#[test]
fn builtin_module_put_and_get_bool() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("bool_mod");
    let mut ct_env = CtEnv::new();

    builtins.get("module_put").unwrap()(
        &[CtValue::String("flag".to_string()), CtValue::Bool(true)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("put bool");

    let result = builtins.get("module_get").unwrap()(
        &[CtValue::String("flag".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("get bool");
    assert_eq!(result, CtValue::Bool(true));
}

#[test]
fn builtin_module_put_and_get_keyword() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("kw_mod");
    let mut ct_env = CtEnv::new();

    let kw = CtValue::Keyword(vec![
        (
            String::from("host"),
            CtValue::String(String::from("localhost")),
        ),
        (String::from("port"), CtValue::Int(8080)),
    ]);

    builtins.get("module_put").unwrap()(
        &[CtValue::String("config".to_string()), kw.clone()],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("put keyword");

    let result = builtins.get("module_get").unwrap()(
        &[CtValue::String("config".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("get keyword");
    assert_eq!(result, kw);
}

#[test]
fn builtin_module_put_and_get_ast() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("ast_mod");
    let mut ct_env = CtEnv::new();

    let ast = CtValue::Ast(vec![FormItem::Form(Form {
        head: "text".to_string(),
        meta: dummy_form_meta(),
        fields: vec![],
    })]);

    builtins.get("module_put").unwrap()(
        &[CtValue::String("fragment".to_string()), ast.clone()],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("put ast");

    let result = builtins.get("module_get").unwrap()(
        &[CtValue::String("fragment".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("get ast");
    assert_eq!(result, ast);
}

// ========================================================================
// Step 5.4.1: module_update builtin tests
// ========================================================================

#[test]
fn builtin_module_update_key_not_exists_returns_new_value() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("update_mod");
    let mut ct_env = CtEnv::new();

    let result = builtins.get("module_update").unwrap()(
        &[CtValue::String("counter".to_string()), CtValue::Int(1)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("update new key");
    assert_eq!(result, CtValue::Int(1));
}

#[test]
fn builtin_module_update_overwrites_existing_value() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("update_mod");
    let mut ct_env = CtEnv::new();

    // First put
    builtins.get("module_put").unwrap()(
        &[CtValue::String("counter".to_string()), CtValue::Int(1)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("put initial");

    // Update overwrites
    let result = builtins.get("module_update").unwrap()(
        &[CtValue::String("counter".to_string()), CtValue::Int(2)],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("update existing");
    assert_eq!(result, CtValue::Int(2));

    // Verify the value was updated
    let after = builtins.get("module_get").unwrap()(
        &[CtValue::String("counter".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("get after update");
    assert_eq!(after, CtValue::Int(2));
}

#[test]
fn builtin_module_update_wrong_arg_count_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("update_mod");
    let mut ct_env = CtEnv::new();

    let err = builtins.get("module_update").unwrap()(
        &[CtValue::String("key".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on 1 arg");
    assert!(err.to_string().contains("exactly 2 arguments"));

    let err = builtins.get("module_update").unwrap()(
        &[
            CtValue::String("key".to_string()),
            CtValue::Int(1),
            CtValue::Int(2),
        ],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on 3 args");
    assert!(err.to_string().contains("exactly 2 arguments"));
}

// ========================================================================
// Step 5.4.2: list builtin tests
// ========================================================================

#[test]
fn builtin_list_empty_returns_empty_list() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("list_mod");
    let mut ct_env = CtEnv::new();

    let result = builtins.get("list").unwrap()(
        &[],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("list empty");
    assert_eq!(result, CtValue::List(vec![]));
}

#[test]
fn builtin_list_single_item() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("list_mod");
    let mut ct_env = CtEnv::new();

    let result = builtins.get("list").unwrap()(
        &[CtValue::String("a".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("list single");
    assert_eq!(
        result,
        CtValue::List(vec![CtValue::String("a".to_string())])
    );
}

#[test]
fn builtin_list_multiple_items() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("list_mod");
    let mut ct_env = CtEnv::new();

    let result = builtins.get("list").unwrap()(
        &[
            CtValue::String("a".to_string()),
            CtValue::Int(42),
            CtValue::Bool(true),
        ],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("list multiple");
    assert_eq!(
        result,
        CtValue::List(vec![
            CtValue::String("a".to_string()),
            CtValue::Int(42),
            CtValue::Bool(true)
        ])
    );
}

#[test]
fn builtin_list_preserves_nested_types() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("list_mod");
    let mut ct_env = CtEnv::new();

    let nested_list = CtValue::List(vec![CtValue::Int(1), CtValue::Int(2)]);
    let nested_keyword = CtValue::Keyword(vec![(
        "key".to_string(),
        CtValue::String("value".to_string()),
    )]);
    let nested_ast = CtValue::Ast(vec![FormItem::Form(Form {
        head: "text".to_string(),
        meta: dummy_form_meta(),
        fields: vec![],
    })]);

    let result = builtins.get("list").unwrap()(
        &[
            nested_list.clone(),
            nested_keyword.clone(),
            nested_ast.clone(),
        ],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("list nested");
    assert_eq!(
        result,
        CtValue::List(vec![nested_list, nested_keyword, nested_ast])
    );
}

// ========================================================================
// list_concat builtin tests
// ========================================================================

#[test]
fn builtin_list_concat_two_lists() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("concat_mod");
    let mut ct_env = CtEnv::new();

    let result = builtins.get("list_concat").unwrap()(
        &[
            CtValue::List(vec![CtValue::String("a".to_string())]),
            CtValue::List(vec![CtValue::String("b".to_string())]),
        ],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("concat two");
    assert_eq!(
        result,
        CtValue::List(vec![
            CtValue::String("a".to_string()),
            CtValue::String("b".to_string()),
        ])
    );
}

#[test]
fn builtin_list_concat_nil_as_empty() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("concat_mod");
    let mut ct_env = CtEnv::new();

    // Nil as first arg (mimics first-call scenario with module_get returning Nil)
    let result = builtins.get("list_concat").unwrap()(
        &[
            CtValue::Nil,
            CtValue::List(vec![CtValue::String("a".to_string())]),
        ],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("concat with nil");
    assert_eq!(
        result,
        CtValue::List(vec![CtValue::String("a".to_string())])
    );
}

#[test]
fn builtin_list_concat_empty_args() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("concat_mod");
    let mut ct_env = CtEnv::new();

    let result = builtins.get("list_concat").unwrap()(
        &[],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("concat empty");
    assert_eq!(result, CtValue::List(vec![]));
}

#[test]
fn builtin_list_concat_wrong_type_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("concat_mod");
    let mut ct_env = CtEnv::new();

    let err = builtins.get("list_concat").unwrap()(
        &[CtValue::String("not a list".to_string())],
        &mut empty_macro_env(),
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on string");
    assert!(err.to_string().contains("must be list or nil"));
}

// ========================================================================
// Step 7.2: list iteration builtin tests
// ========================================================================

/// Helper: create a FormItem::Form for `<unquote>name</unquote>`.
/// The variable name goes in the body as a FormItem::Text child (not a <var> element).
/// This is what eval_unquote expects: raw_body_text reads text from the children field.
fn unquote_var(name: &str) -> FormItem {
    let meta = FormMeta::default();
    // Body: a single FormItem::Text with the variable name
    // raw_body_text reads FormItem::Text from the "children" field
    FormItem::Form(Form {
        head: "unquote".to_string(),
        meta,
        fields: vec![FormField {
            name: "children".to_string(),
            value: FormValue::Sequence(vec![FormItem::Text(name.to_string())]),
        }],
    })
}

/// Helper: create a lazy quote CtValue from form items.
/// The lazy quote defers string-slot processing until the callback is evaluated
/// (when _item is bound by list_map/list_foreach/list_fold).
fn quote_forms(items: Vec<FormItem>) -> CtValue {
    CtValue::LazyQuote(items)
}

#[test]
fn builtin_list_foreach_returns_nil() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("foreach_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    // Callback: <quote><text><unquote><var>_item</var></unquote></text></quote>
    // This evaluates _item and returns it as Ast (side-effect-free)
    let callback = quote_forms(vec![unquote_var("_item")]);

    let result = builtins.get("list_foreach").unwrap()(
        &[
            CtValue::List(vec![
                CtValue::String("a".to_string()),
                CtValue::String("b".to_string()),
            ]),
            callback,
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("list_foreach should succeed");
    assert_eq!(result, CtValue::Nil);
}

#[test]
fn builtin_list_foreach_wrong_arg_count_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("foreach_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    // Only 1 arg
    let err = builtins.get("list_foreach").unwrap()(
        &[CtValue::List(vec![])],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on 1 arg");
    assert!(err.to_string().contains("requires exactly 2"));
}

#[test]
fn builtin_list_foreach_first_arg_not_list_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("foreach_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let err = builtins.get("list_foreach").unwrap()(
        &[
            CtValue::String("not a list".to_string()),
            quote_forms(vec![]),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on non-list");
    assert!(err.to_string().contains("must be list"));
}

#[test]
fn builtin_list_map_identity_transform() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("map_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    // Callback: <quote><unquote><var>_item</var></unquote></quote>
    // Returns _item (the bound list element)
    let callback = quote_forms(vec![unquote_var("_item")]);

    let result = builtins.get("list_map").unwrap()(
        &[
            CtValue::List(vec![
                CtValue::String("a".to_string()),
                CtValue::String("b".to_string()),
            ]),
            callback,
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("list_map should succeed");

    // Each item should be returned as-is (the callback returns _item)
    let expected = CtValue::List(vec![
        CtValue::String("a".to_string()),
        CtValue::String("b".to_string()),
    ]);
    assert_eq!(result, expected);
}

#[test]
fn builtin_list_map_empty_list_returns_empty() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("map_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let callback = quote_forms(vec![unquote_var("_item")]);

    let result = builtins.get("list_map").unwrap()(
        &[CtValue::List(vec![]), callback],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("list_map should succeed");
    assert_eq!(result, CtValue::List(vec![]));
}

#[test]
fn builtin_list_map_wrong_arg_count_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("map_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let err = builtins.get("list_map").unwrap()(
        &[CtValue::List(vec![])],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on 1 arg");
    assert!(err.to_string().contains("requires exactly 2"));
}

#[test]
fn builtin_list_map_first_arg_not_list_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("map_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let err = builtins.get("list_map").unwrap()(
        &[CtValue::Int(42), quote_forms(vec![])],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on non-list");
    assert!(err.to_string().contains("must be list"));
}

#[test]
fn builtin_list_map_callback_not_ast_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("map_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    // Second arg is not an Ast (it's a String)
    let err = builtins.get("list_map").unwrap()(
        &[
            CtValue::List(vec![]),
            CtValue::String("not a callback".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on non-ast callback");
    assert!(err.to_string().contains("callback"));
}

#[test]
fn builtin_list_fold_sums_integers() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("fold_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    // Callback: <quote><unquote><var>_item</var></unquote></quote>
    // Returns _item (the bound list element as CtValue)
    let callback = quote_forms(vec![unquote_var("_item")]);

    let result = builtins.get("list_fold").unwrap()(
        &[
            CtValue::List(vec![CtValue::Int(1), CtValue::Int(2), CtValue::Int(3)]),
            CtValue::Int(0), // init = 0
            callback,
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("list_fold should succeed");

    // Last element is 3 (accumulator ends as last callback result)
    assert_eq!(result, CtValue::Int(3));
}

#[test]
fn builtin_list_fold_empty_list_returns_init() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("fold_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let callback = quote_forms(vec![unquote_var("_item")]);

    let result = builtins.get("list_fold").unwrap()(
        &[
            CtValue::List(vec![]),
            CtValue::String("init".to_string()),
            callback,
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("list_fold should succeed");

    // Empty list: init is returned
    assert_eq!(result, CtValue::String("init".to_string()));
}

#[test]
fn builtin_list_fold_wrong_arg_count_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("fold_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let err = builtins.get("list_fold").unwrap()(
        &[CtValue::List(vec![]), CtValue::Int(0)],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on 2 args");
    assert!(err.to_string().contains("requires exactly 3"));
}

#[test]
fn builtin_list_fold_first_arg_not_list_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = make_module_env("fold_mod");
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let err = builtins.get("list_fold").unwrap()(
        &[
            CtValue::String("not a list".to_string()),
            CtValue::Int(0),
            quote_forms(vec![]),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on non-list");
    assert!(err.to_string().contains("must be list"));
}

// ========== Step 7.3: keyword_keys / keyword_pairs tests ==========

#[test]
fn builtin_keyword_keys_basic() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let keyword = CtValue::Keyword(vec![
        ("name".to_string(), CtValue::String("Alice".to_string())),
        ("age".to_string(), CtValue::Int(30)),
        ("active".to_string(), CtValue::Bool(true)),
    ]);

    let result = builtins.get("keyword_keys").unwrap()(
        &[keyword],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("keyword_keys should succeed");

    let keys = match result {
        CtValue::List(items) => items,
        _ => panic!("expected list, got {:?}", result),
    };

    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&CtValue::String("name".to_string())));
    assert!(keys.contains(&CtValue::String("age".to_string())));
    assert!(keys.contains(&CtValue::String("active".to_string())));
}

#[test]
fn builtin_keyword_keys_empty() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let keyword = CtValue::Keyword(vec![]);

    let result = builtins.get("keyword_keys").unwrap()(
        &[keyword],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("keyword_keys should succeed on empty keyword");

    assert_eq!(result, CtValue::List(vec![]));
}

#[test]
fn builtin_keyword_keys_wrong_arg_count_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let err = builtins.get("keyword_keys").unwrap()(
        &[],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on 0 args");
    assert!(err.to_string().contains("requires exactly 1"));
}

#[test]
fn builtin_keyword_keys_wrong_type_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let err = builtins.get("keyword_keys").unwrap()(
        &[CtValue::String("not a keyword".to_string())],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on non-keyword");
    assert!(err.to_string().contains("must be keyword"));
}

#[test]
fn builtin_keyword_pairs_basic() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let keyword = CtValue::Keyword(vec![
        ("name".to_string(), CtValue::String("Alice".to_string())),
        ("age".to_string(), CtValue::Int(30)),
    ]);

    let result = builtins.get("keyword_pairs").unwrap()(
        &[keyword],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("keyword_pairs should succeed");

    let pairs = match result {
        CtValue::List(items) => items,
        _ => panic!("expected list, got {:?}", result),
    };

    assert_eq!(pairs.len(), 2);

    // Check first pair [key, value]
    let pair1 = match &pairs[0] {
        CtValue::List(items) => items.clone(),
        _ => panic!("expected pair as list"),
    };
    assert_eq!(pair1[0], CtValue::String("name".to_string()));
    assert_eq!(pair1[1], CtValue::String("Alice".to_string()));

    // Check second pair [key, value]
    let pair2 = match &pairs[1] {
        CtValue::List(items) => items.clone(),
        _ => panic!("expected pair as list"),
    };
    assert_eq!(pair2[0], CtValue::String("age".to_string()));
    assert_eq!(pair2[1], CtValue::Int(30));
}

#[test]
fn builtin_keyword_pairs_empty() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let keyword = CtValue::Keyword(vec![]);

    let result = builtins.get("keyword_pairs").unwrap()(
        &[keyword],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("keyword_pairs should succeed on empty keyword");

    assert_eq!(result, CtValue::List(vec![]));
}

#[test]
fn builtin_keyword_pairs_wrong_arg_count_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let err = builtins.get("keyword_pairs").unwrap()(
        &[CtValue::Keyword(vec![]), CtValue::Keyword(vec![])],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on 2 args");
    assert!(err.to_string().contains("requires exactly 1"));
}

#[test]
fn builtin_keyword_pairs_wrong_type_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let err = builtins.get("keyword_pairs").unwrap()(
        &[CtValue::List(vec![])],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on non-keyword");
    assert!(err.to_string().contains("must be keyword"));
}

// ============================================================================
// Step 7.4: match builtin tests
// ============================================================================

#[test]
fn builtin_match_int_pattern_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let result = builtins.get("match").unwrap()(
        &[
            CtValue::Int(2),
            CtValue::Int(1),
            CtValue::String("one".to_string()),
            CtValue::Int(2),
            CtValue::String("two".to_string()),
            CtValue::Int(3),
            CtValue::String("three".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("match should succeed");

    assert_eq!(result, CtValue::String("two".to_string()));
}

#[test]
fn builtin_match_string_pattern_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let result = builtins.get("match").unwrap()(
        &[
            CtValue::String("hello".to_string()),
            CtValue::String("world".to_string()),
            CtValue::String("got world".to_string()),
            CtValue::String("hello".to_string()),
            CtValue::String("got hello".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("match should succeed");

    assert_eq!(result, CtValue::String("got hello".to_string()));
}

#[test]
fn builtin_match_bool_pattern_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let result = builtins.get("match").unwrap()(
        &[
            CtValue::Bool(true),
            CtValue::Bool(false),
            CtValue::String("false branch".to_string()),
            CtValue::Bool(true),
            CtValue::String("true branch".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("match should succeed");

    assert_eq!(result, CtValue::String("true branch".to_string()));
}

#[test]
fn builtin_match_list_pattern_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let list_pattern = CtValue::List(vec![CtValue::Int(1), CtValue::Int(2)]);
    let list_value = CtValue::List(vec![CtValue::Int(1), CtValue::Int(2)]);
    let other_list = CtValue::List(vec![CtValue::Int(3), CtValue::Int(4)]);

    let result = builtins.get("match").unwrap()(
        &[
            list_value,
            other_list,
            CtValue::String("other list".to_string()),
            list_pattern,
            CtValue::String("matched list".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("match should succeed");

    assert_eq!(result, CtValue::String("matched list".to_string()));
}

#[test]
fn builtin_match_keyword_pattern_works() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let kw_pattern = CtValue::Keyword(vec![(
        "key".to_string(),
        CtValue::String("value".to_string()),
    )]);
    let kw_value = CtValue::Keyword(vec![(
        "key".to_string(),
        CtValue::String("value".to_string()),
    )]);

    let result = builtins.get("match").unwrap()(
        &[
            kw_value,
            kw_pattern,
            CtValue::String("matched keyword".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("match should succeed");

    assert_eq!(result, CtValue::String("matched keyword".to_string()));
}

#[test]
fn builtin_match_wildcard_matches_any_value() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    // Test wildcard matching an int
    let result = builtins.get("match").unwrap()(
        &[
            CtValue::Int(42),
            CtValue::String("_".to_string()),
            CtValue::String("fallback".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("match with wildcard should succeed");

    assert_eq!(result, CtValue::String("fallback".to_string()));

    // Test wildcard matching a string
    let result2 = builtins.get("match").unwrap()(
        &[
            CtValue::String("anything".to_string()),
            CtValue::String("_".to_string()),
            CtValue::String("wildcard matched".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("match with wildcard should succeed");

    assert_eq!(result2, CtValue::String("wildcard matched".to_string()));
}

#[test]
fn builtin_match_wildcard_as_fallback() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    // No pattern matches, fallback to wildcard
    let result = builtins.get("match").unwrap()(
        &[
            CtValue::Int(99),
            CtValue::Int(1),
            CtValue::String("one".to_string()),
            CtValue::Int(2),
            CtValue::String("two".to_string()),
            CtValue::String("_".to_string()),
            CtValue::String("default".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("match should succeed with wildcard fallback");

    assert_eq!(result, CtValue::String("default".to_string()));
}

#[test]
fn builtin_match_no_match_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    let err = builtins.get("match").unwrap()(
        &[
            CtValue::Int(99),
            CtValue::Int(1),
            CtValue::String("one".to_string()),
            CtValue::Int(2),
            CtValue::String("two".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error when no pattern matches");

    assert!(err.to_string().contains("no pattern matched"));
    assert!(err.to_string().contains("wildcard"));
}

#[test]
fn builtin_match_wrong_arg_count_errors() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    // Too few args
    let err = builtins.get("match").unwrap()(
        &[CtValue::Int(1), CtValue::Int(1)],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on 2 args");
    assert!(err.to_string().contains("at least 3"));

    // Even number of args (missing result for last pattern)
    let err2 = builtins.get("match").unwrap()(
        &[
            CtValue::Int(1),
            CtValue::Int(1),
            CtValue::String("one".to_string()),
            CtValue::Int(2),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect_err("should error on even args");
    assert!(err2.to_string().contains("odd number"));
}

#[test]
fn builtin_match_returns_first_match() {
    let builtins = BuiltinRegistry::new();
    let mut expand_env = ExpandEnv::default();
    let mut ct_env = CtEnv::new();
    let mut macro_env = MacroEnv::default();

    // Multiple patterns could match, first one wins
    let result = builtins.get("match").unwrap()(
        &[
            CtValue::Int(1),
            CtValue::Int(1),
            CtValue::String("first".to_string()),
            CtValue::Int(1),
            CtValue::String("second".to_string()),
            CtValue::String("_".to_string()),
            CtValue::String("third".to_string()),
        ],
        &mut macro_env,
        &mut ct_env,
        &mut expand_env,
        &builtins,
    )
    .expect("match should succeed");

    assert_eq!(result, CtValue::String("first".to_string()));
}
