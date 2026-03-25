//! Unit tests for Form → CtStmt/CtExpr conversion (convert.rs)
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

    let result = convert_macro_body(&[FormItem::Form(if_form)]).expect("convert should succeed");
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

    let result = convert_macro_body(&[FormItem::Form(let_form)]).expect("convert should succeed");
    assert_eq!(result.stmts.len(), 1);
    match &result.stmts[0] {
        CtStmt::Let { name, value, .. } => {
            assert_eq!(name, "opts");
            // Should use keyword_attr builtin
            assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "keyword_attr"));
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

    let result = convert_macro_body(&[FormItem::Form(let_form)]).expect("convert should succeed");
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
            assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "caller_module"));
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

    let result =
        convert_macro_body(&[FormItem::Form(quote_form)]).expect("quote top-level should succeed");
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

    let result =
        convert_macro_body(&[FormItem::Form(require_form)]).expect("require_module should succeed");
    assert_eq!(result.stmts.len(), 1);
    match &result.stmts[0] {
        CtStmt::Expr { expr } => {
            assert!(matches!(expr, CtExpr::BuiltinCall { name, .. } if name == "require_module"));
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

    let result =
        convert_macro_body(&[FormItem::Form(keyword_form)]).expect("keyword_attr should succeed");
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

    let result =
        convert_macro_body(&[FormItem::Form(invoke_form)]).expect("invoke_macro should succeed");
    assert_eq!(result.stmts.len(), 1);
    match &result.stmts[0] {
        CtStmt::Return { value } => {
            assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "invoke_macro"));
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
            assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "caller_module"));
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
            assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "require_module"));
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

    let result =
        convert_macro_body(&[FormItem::Form(return_form)]).expect("empty return should succeed");
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

    let result =
        convert_macro_body(&[FormItem::Form(return_form)]).expect("var expression should succeed");
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
            assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "require_module"));
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
            assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "expand_alias"));
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
            assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "keyword_attr"));
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
            assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "invoke_macro"));
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
            assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "invoke_macro"));
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
            .contains("requires module attribute or <var>/<get-attribute>/<literal> child"),
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
            .contains("child must be <var>, <get-attribute>, or <literal>"),
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
            assert!(matches!(value, CtExpr::BuiltinCall { name, .. } if name == "invoke_macro"));
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
