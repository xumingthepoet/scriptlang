//! Tests for the const_eval module.

use std::collections::{BTreeMap, BTreeSet};

use crate::semantic::expand::scope::QualifiedConstLookup;
use crate::semantic::expr::{rewrite_expr_with_consts, rewrite_template_with_consts};
use crate::semantic::types::DeclaredType;
use sl_core::{ScriptLangError, TextSegment, TextTemplate};

use super::{ConstEnv, ConstLookup, ConstParser, ConstValue, parse_const_value};

struct TestResolver {
    current_module: String,
    imported_short_env: BTreeMap<String, ConstValue>,
    visible_modules: BTreeMap<String, ConstEnv>,
    visible_functions: BTreeSet<String>,
    known_modules: BTreeSet<String>,
}

impl ConstLookup for TestResolver {
    fn current_module(&self) -> &str {
        &self.current_module
    }

    fn resolve_short_const(&mut self, name: &str) -> Result<Option<ConstValue>, ScriptLangError> {
        Ok(self.imported_short_env.get(name).cloned())
    }

    fn resolve_qualified_const(
        &mut self,
        module_path: &str,
        name: &str,
    ) -> Result<QualifiedConstLookup, ScriptLangError> {
        if let Some(module_env) = self.visible_modules.get(module_path) {
            if let Some(value) = module_env.get(name) {
                Ok(QualifiedConstLookup::Value(value.clone()))
            } else {
                Ok(QualifiedConstLookup::UnknownConst)
            }
        } else if self.known_modules.contains(module_path) {
            Ok(QualifiedConstLookup::HiddenModule)
        } else {
            Ok(QualifiedConstLookup::NotModulePath)
        }
    }

    fn resolve_script_literal(&mut self, raw: &str) -> Result<String, ScriptLangError> {
        let raw = raw.strip_prefix('@').expect("script literal");
        Ok(if raw.contains('.') {
            raw.to_string()
        } else {
            format!("{}.{}", self.current_module, raw)
        })
    }

    fn resolve_function_literal(&mut self, raw: &str) -> Result<String, ScriptLangError> {
        let raw = raw.strip_prefix('#').expect("function literal");
        let resolved = if raw.contains('.') {
            raw.to_string()
        } else {
            format!("{}.{}", self.current_module, raw)
        };
        if self.visible_functions.contains(&resolved) {
            Ok(resolved)
        } else {
            Err(ScriptLangError::message(format!(
                "unknown function `{resolved}`"
            )))
        }
    }
}

fn visible() -> TestResolver {
    TestResolver {
        current_module: "main".to_string(),
        imported_short_env: BTreeMap::from([("answer".to_string(), ConstValue::Integer(42))]),
        visible_modules: BTreeMap::from([(
            "lib.math".to_string(),
            BTreeMap::from([("zero".to_string(), ConstValue::Integer(0))]),
        )]),
        visible_functions: BTreeSet::from(["main.pick".to_string(), "lib.math.pick".to_string()]),
        known_modules: BTreeSet::from(["lib.math".to_string(), "hidden".to_string()]),
    }
}

#[test]
fn parse_const_value_supports_literals_containers_and_const_refs() {
    let local_env = BTreeMap::from([("local".to_string(), ConstValue::Integer(7))]);
    let mut visible = visible();

    assert_eq!(
        parse_const_value(
            r#"[local, answer, true, "ok"]"#,
            &local_env,
            &mut visible,
            &BTreeSet::new(),
            None,
        )
        .expect("parse"),
        ConstValue::Array(vec![
            ConstValue::Integer(7),
            ConstValue::Integer(42),
            ConstValue::Bool(true),
            ConstValue::String("ok".to_string()),
        ])
    );
    assert_eq!(
        parse_const_value(
            "#{foo: lib.math.zero}",
            &local_env,
            &mut visible,
            &BTreeSet::new(),
            None,
        )
        .expect("parse"),
        ConstValue::Object(BTreeMap::from([(
            "foo".to_string(),
            ConstValue::Integer(0)
        )]))
    );
    assert_eq!(
        parse_const_value(
            "#lib.math.pick",
            &local_env,
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::Function),
        )
        .expect("function literal"),
        ConstValue::Function("lib.math.pick".to_string())
    );
    assert_eq!(
        parse_const_value(
            "#pick",
            &local_env,
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::Function),
        )
        .expect("short function literal"),
        ConstValue::Function("main.pick".to_string())
    );
}

#[test]
fn parse_const_value_rejects_forward_refs_and_unsupported_shapes() {
    let blocked = BTreeSet::from(["later".to_string()]);
    let mut visible = visible();

    assert!(
        parse_const_value("later", &BTreeMap::new(), &mut visible, &blocked, None)
            .expect_err("forward ref")
            .to_string()
            .contains("cannot be referenced before it is defined")
    );
    assert!(
        parse_const_value(
            "call()",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None
        )
        .expect_err("call")
        .to_string()
        .contains("unsupported const reference `call`")
    );
    assert!(
        parse_const_value(
            "hidden.zero",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None,
        )
        .expect_err("hidden")
        .to_string()
        .contains("module `hidden` is not imported")
    );
    assert!(
        parse_const_value(
            "1.5",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None
        )
        .expect_err("float")
        .to_string()
        .contains("float const literals are not supported")
    );
    assert!(
        parse_const_value(
            "#missing",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::Function),
        )
        .expect_err("missing function")
        .to_string()
        .contains("unknown function")
    );
    assert!(
        parse_const_value(
            "#pick",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::Script),
        )
        .expect_err("wrong type")
        .to_string()
        .contains("script literal")
    );
}

#[test]
fn parse_const_value_supports_script_literals_for_script_type() {
    let mut visible = visible();
    let value = parse_const_value(
        "@loop",
        &BTreeMap::new(),
        &mut visible,
        &BTreeSet::new(),
        Some(&DeclaredType::Script),
    )
    .expect("script literal");
    assert_eq!(value, ConstValue::Script("main.loop".to_string()));
}

#[test]
fn parse_const_value_covers_parse_and_type_error_paths() {
    let mut visible = visible();

    assert!(
        parse_const_value("", &BTreeMap::new(), &mut visible, &BTreeSet::new(), None)
            .expect_err("empty expression")
            .to_string()
            .contains("empty const expression")
    );
    assert!(
        parse_const_value(
            "42 trailing",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None
        )
        .expect_err("trailing tokens")
        .to_string()
        .contains("unexpected trailing tokens")
    );
    assert!(
        parse_const_value("(", &BTreeMap::new(), &mut visible, &BTreeSet::new(), None)
            .expect_err("unsupported start")
            .to_string()
            .contains("unsupported const expression starting")
    );
    assert!(
        parse_const_value(
            "#{1: 2}",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None
        )
        .expect_err("bad object key")
        .to_string()
        .contains("unsupported object key")
    );
    assert!(
        parse_const_value("-", &BTreeMap::new(), &mut visible, &BTreeSet::new(), None)
            .expect_err("bad number")
            .to_string()
            .contains("invalid const number literal")
    );
    assert!(
        parse_const_value(
            "\"unterminated",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None
        )
        .expect_err("unterminated string")
        .to_string()
        .contains("unterminated string literal")
    );
    assert!(
        parse_const_value(
            "\"a\\",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None
        )
        .expect_err("unterminated escape")
        .to_string()
        .contains("unterminated escape sequence")
    );
    assert!(
        parse_const_value(
            "main.missing",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None,
        )
        .expect_err("current module missing const")
        .to_string()
        .contains("does not export const `missing`")
    );
    assert!(
        parse_const_value(
            "lib.math.nope",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None,
        )
        .expect_err("unknown qualified const")
        .to_string()
        .contains("does not export const `nope`")
    );
    assert!(
        parse_const_value(
            "missing.path",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None,
        )
        .expect_err("not module path")
        .to_string()
        .contains("unsupported const reference `missing.path`")
    );
    assert!(
        parse_const_value(
            "true",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::Array),
        )
        .expect_err("array mismatch")
        .to_string()
        .contains("const declared as `array`")
    );
    assert!(
        parse_const_value(
            "\"ok\"",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::Bool),
        )
        .expect_err("bool mismatch")
        .to_string()
        .contains("const declared as `bool`")
    );
    assert!(
        parse_const_value(
            "1",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::Object),
        )
        .expect_err("object mismatch")
        .to_string()
        .contains("const declared as `object`")
    );
    assert!(
        parse_const_value(
            "1",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::String),
        )
        .expect_err("string mismatch")
        .to_string()
        .contains("const declared as `string`")
    );

    let escaped = parse_const_value(
        "\"line\\nquote\\\"tab\\t\"",
        &BTreeMap::new(),
        &mut visible,
        &BTreeSet::new(),
        Some(&DeclaredType::String),
    )
    .expect("escaped string");
    assert_eq!(
        escaped,
        ConstValue::String("line\nquote\"tab\t".to_string())
    );
}

#[test]
fn const_value_to_rhai_literal_covers_all_shapes() {
    let value = ConstValue::Object(BTreeMap::from([
        ("enabled".to_string(), ConstValue::Bool(true)),
        (
            "items".to_string(),
            ConstValue::Array(vec![
                ConstValue::Integer(1),
                ConstValue::String("ok".to_string()),
                ConstValue::Script("main.loop".to_string()),
                ConstValue::Function("main.pick".to_string()),
            ]),
        ),
    ]));

    let rhai = value.to_rhai_literal();
    assert!(rhai.contains("enabled: true"));
    assert!(rhai.contains("[1, \"ok\", \"main.loop\", \"main.pick\"]"));
}

#[test]
fn parse_const_value_covers_empty_literals_and_parser_punctuation_errors() {
    let mut visible = visible();

    assert_eq!(
        parse_const_value(
            "[]",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::Array)
        )
        .expect("empty array"),
        ConstValue::Array(vec![])
    );
    assert_eq!(
        parse_const_value(
            "#{\"quoted\": 1}",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::Object),
        )
        .expect("quoted key object"),
        ConstValue::Object(BTreeMap::from([(
            "quoted".to_string(),
            ConstValue::Integer(1),
        )]))
    );
    assert_eq!(
        parse_const_value(
            "@lib.math.zero",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::Script),
        )
        .expect("qualified script literal"),
        ConstValue::Script("lib.math.zero".to_string())
    );

    assert!(
        parse_const_value(
            "[1 2]",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None
        )
        .expect_err("missing array comma")
        .to_string()
        .contains("expected `,`")
    );
    assert!(
        parse_const_value(
            "#{foo 1}",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None
        )
        .expect_err("missing object colon")
        .to_string()
        .contains("expected `:`")
    );
    assert!(
        parse_const_value(
            "@foo.",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None
        )
        .expect_err("missing ident after script dot")
        .to_string()
        .contains("unexpected end of input")
    );
    assert!(
        parse_const_value(
            "#foo.",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None
        )
        .expect_err("missing ident after function dot")
        .to_string()
        .contains("unexpected end of input")
    );
    assert!(
        parse_const_value(
            "\"x\\q\"",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            Some(&DeclaredType::String),
        )
        .expect("unknown escape passthrough")
            == ConstValue::String("xq".to_string())
    );
    assert!(
        parse_const_value(
            "9999999999999999999999999999999999999",
            &BTreeMap::new(),
            &mut visible,
            &BTreeSet::new(),
            None,
        )
        .expect_err("integer overflow")
        .to_string()
        .contains("invalid integer const literal")
    );
}

#[test]
fn rewrite_helpers_replace_only_visible_const_refs() {
    let local_env = BTreeMap::from([("name".to_string(), ConstValue::String("neo".to_string()))]);
    let mut visible = visible();
    let rewritten = rewrite_expr_with_consts(
        r##"answer + answer_more + obj.answer + lib.math.zero + "#{answer}" + #{answer: answer} + name + hidden.zero"##,
        &local_env,
        &mut visible,
        &BTreeSet::new(),
        &BTreeSet::new(),
    )
    .expect("rewrite");
    assert_eq!(
        rewritten,
        r##"42 + answer_more + obj.answer + 0 + "#{answer}" + #{answer: 42} + "neo" + hidden.zero"##
    );

    let template = rewrite_template_with_consts(
        TextTemplate {
            segments: vec![
                TextSegment::Literal("x=".to_string()),
                TextSegment::Expr("lib.math.zero".to_string()),
            ],
        },
        &BTreeMap::new(),
        &mut visible,
        &BTreeSet::new(),
        &BTreeSet::new(),
    )
    .expect("template");
    assert!(matches!(
        &template.segments[1],
        TextSegment::Expr(expr) if expr == "0"
    ));

    let untouched = rewrite_expr_with_consts(
        "obj.answer",
        &BTreeMap::new(),
        &mut visible,
        &BTreeSet::new(),
        &BTreeSet::new(),
    )
    .expect("property access");
    assert_eq!(untouched, "obj.answer");
}

#[test]
fn const_parser_direct_helpers_cover_remaining_shapes() {
    let mut resolver = visible();
    let empty = BTreeMap::new();
    let blocked = BTreeSet::new();

    let mut parser = ConstParser {
        source: "#{}",
        cursor: 0,
        local_env: &empty,
        resolver: &mut resolver,
        blocked_names: &blocked,
        expected_type: None,
    };
    assert_eq!(
        parser.parse_object().expect("empty object"),
        ConstValue::Object(BTreeMap::new())
    );

    let mut parser = ConstParser {
        source: "#{a: 1}",
        cursor: 0,
        local_env: &empty,
        resolver: &mut resolver,
        blocked_names: &blocked,
        expected_type: None,
    };
    assert_eq!(
        parser.parse_object().expect("object"),
        ConstValue::Object(BTreeMap::from([("a".to_string(), ConstValue::Integer(1))]))
    );

    let mut parser = ConstParser {
        source: "#pick",
        cursor: 0,
        local_env: &empty,
        resolver: &mut resolver,
        blocked_names: &blocked,
        expected_type: None,
    };
    assert_eq!(
        parser.parse_function_literal().expect("function"),
        ConstValue::Function("main.pick".to_string())
    );

    let mut parser = ConstParser {
        source: "-9223372036854775809",
        cursor: 0,
        local_env: &empty,
        resolver: &mut resolver,
        blocked_names: &blocked,
        expected_type: None,
    };
    assert!(
        parser
            .parse_number()
            .expect_err("overflow")
            .to_string()
            .contains("invalid integer const literal")
    );

    let mut parser = ConstParser {
        source: "main.answer",
        cursor: 0,
        local_env: &empty,
        resolver: &mut resolver,
        blocked_names: &blocked,
        expected_type: None,
    };
    assert!(
        parser
            .parse_reference_value()
            .expect_err("missing")
            .to_string()
            .contains("does not export const `answer`")
    );

    let mut parser = ConstParser {
        source: "\"x\" \"a\\r\\t\\\\\\'\\\"\"",
        cursor: 0,
        local_env: &empty,
        resolver: &mut resolver,
        blocked_names: &blocked,
        expected_type: None,
    };
    assert_eq!(parser.parse_string().expect("string"), "x");
    parser.skip_ws();
    assert_eq!(parser.parse_string().expect("escaped"), "a\r\t\\'\"");

    let mut parser = ConstParser {
        source: "",
        cursor: 0,
        local_env: &empty,
        resolver: &mut resolver,
        blocked_names: &blocked,
        expected_type: None,
    };
    assert!(
        parser
            .parse_string()
            .expect_err("unexpected end")
            .to_string()
            .contains("unexpected end of string literal")
    );
    assert!(
        parser
            .parse_object_key()
            .expect_err("object eof")
            .to_string()
            .contains("unexpected end of object literal")
    );
    assert!(
        parser
            .parse_ident()
            .expect_err("ident eof")
            .to_string()
            .contains("unexpected end of input")
    );
}
