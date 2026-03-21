use std::collections::{BTreeMap, BTreeSet};

use sl_core::ScriptLangError;

use super::scope::QualifiedConstLookup;
use crate::semantic::expr::{is_ident_continue, is_ident_start};
use crate::semantic::types::DeclaredType;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConstValue {
    Bool(bool),
    Function(String),
    Integer(i64),
    String(String),
    Script(String),
    Array(Vec<ConstValue>),
    Object(BTreeMap<String, ConstValue>),
}

pub(crate) type ConstEnv = BTreeMap<String, ConstValue>;

pub(crate) trait ConstLookup {
    fn current_module(&self) -> &str;
    fn resolve_short_const(&mut self, name: &str) -> Result<Option<ConstValue>, ScriptLangError>;
    fn resolve_qualified_const(
        &mut self,
        module_path: &str,
        name: &str,
    ) -> Result<QualifiedConstLookup, ScriptLangError>;
    fn resolve_function_literal(&mut self, raw: &str) -> Result<String, ScriptLangError>;
    fn resolve_script_literal(&mut self, raw: &str) -> Result<String, ScriptLangError>;
}

pub(crate) fn parse_const_value<R: ConstLookup>(
    source: &str,
    local_env: &ConstEnv,
    resolver: &mut R,
    blocked_names: &BTreeSet<String>,
    expected_type: Option<&DeclaredType>,
) -> Result<ConstValue, ScriptLangError> {
    let mut parser = ConstParser {
        source,
        cursor: 0,
        local_env,
        resolver,
        blocked_names,
        expected_type,
    };
    let value = parser.parse_value()?;
    let value = parser.check_declared_type(value)?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(ScriptLangError::message(format!(
            "unexpected trailing tokens in const expression `{source}`"
        )));
    }
    Ok(value)
}

impl ConstValue {
    pub(crate) fn to_rhai_literal(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Function(value) => format!("{value:?}"),
            Self::Integer(value) => value.to_string(),
            Self::String(value) => format!("{value:?}"),
            Self::Script(value) => format!("{value:?}"),
            Self::Array(items) => format!(
                "[{}]",
                items
                    .iter()
                    .map(Self::to_rhai_literal)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Object(items) => format!(
                "#{{{}}}",
                items
                    .iter()
                    .map(|(key, value)| format!("{key}: {}", value.to_rhai_literal()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

struct ConstParser<'a, R: ConstLookup> {
    source: &'a str,
    cursor: usize,
    local_env: &'a ConstEnv,
    resolver: &'a mut R,
    blocked_names: &'a BTreeSet<String>,
    expected_type: Option<&'a DeclaredType>,
}

impl<'a, R: ConstLookup> ConstParser<'a, R> {
    fn parse_value(&mut self) -> Result<ConstValue, ScriptLangError> {
        self.skip_ws();
        let Some(ch) = self.peek_char() else {
            return Err(ScriptLangError::message("empty const expression"));
        };
        match ch {
            '#' if self.peek_next_char() != Some('{') => self.parse_function_literal(),
            '@' => self.parse_script_literal(),
            '"' | '\'' => Ok(ConstValue::String(self.parse_string()?)),
            '[' => self.parse_array(),
            '#' => self.parse_object(),
            '-' | '0'..='9' => self.parse_number(),
            _ if is_ident_start(ch) => self.parse_reference_value(),
            _ => Err(ScriptLangError::message(format!(
                "unsupported const expression starting with `{ch}`"
            ))),
        }
    }

    fn parse_array(&mut self) -> Result<ConstValue, ScriptLangError> {
        self.expect_char('[')?;
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            if self.consume_char(']') {
                break;
            }
            items.push(self.parse_value()?);
            self.skip_ws();
            if self.consume_char(']') {
                break;
            }
            self.expect_char(',')?;
        }
        Ok(ConstValue::Array(items))
    }

    fn parse_object(&mut self) -> Result<ConstValue, ScriptLangError> {
        self.expect_char('#')?;
        self.expect_char('{')?;
        let mut items = BTreeMap::new();
        loop {
            self.skip_ws();
            if self.consume_char('}') {
                break;
            }
            let key = self.parse_object_key()?;
            self.skip_ws();
            self.expect_char(':')?;
            let value = self.parse_value()?;
            items.insert(key, value);
            self.skip_ws();
            if self.consume_char('}') {
                break;
            }
            self.expect_char(',')?;
        }
        Ok(ConstValue::Object(items))
    }

    fn parse_object_key(&mut self) -> Result<String, ScriptLangError> {
        self.skip_ws();
        match self.peek_char() {
            Some('"') | Some('\'') => self.parse_string(),
            Some(ch) if is_ident_start(ch) => self.parse_ident(),
            Some(ch) => Err(ScriptLangError::message(format!(
                "unsupported object key starting with `{ch}`"
            ))),
            None => Err(ScriptLangError::message("unexpected end of object literal")),
        }
    }

    fn parse_number(&mut self) -> Result<ConstValue, ScriptLangError> {
        let start = self.cursor;
        if self.peek_char() == Some('-') {
            self.cursor += 1;
        }
        let digit_start = self.cursor;
        while matches!(self.peek_char(), Some('0'..='9')) {
            self.cursor += 1;
        }
        if self.cursor == digit_start {
            return Err(ScriptLangError::message("invalid const number literal"));
        }
        if self.peek_char() == Some('.') {
            return Err(ScriptLangError::message(
                "float const literals are not supported in MVP",
            ));
        }
        let raw = &self.source[start..self.cursor];
        let value = raw.parse::<i64>().map_err(|_| {
            ScriptLangError::message(format!("invalid integer const literal `{raw}`"))
        })?;
        Ok(ConstValue::Integer(value))
    }

    fn parse_reference_value(&mut self) -> Result<ConstValue, ScriptLangError> {
        let segments = self.parse_reference_path()?;
        if segments.len() == 1 {
            let ident = segments[0].as_str();
            match ident {
                "true" => Ok(ConstValue::Bool(true)),
                "false" => Ok(ConstValue::Bool(false)),
                _ => {
                    if let Some(value) = self.local_env.get(ident) {
                        return Ok(value.clone());
                    }
                    if let Some(value) = self.resolver.resolve_short_const(ident)? {
                        return Ok(value);
                    }
                    if self.blocked_names.contains(ident) {
                        return Err(ScriptLangError::message(format!(
                            "const `{ident}` cannot be referenced before it is defined"
                        )));
                    }
                    Err(ScriptLangError::message(format!(
                        "unsupported const reference `{ident}`; only visible const values are allowed"
                    )))
                }
            }
        } else {
            let module_path = segments[..segments.len() - 1].join(".");
            let name = segments.last().expect("qualified path").as_str();
            if module_path == self.resolver.current_module() {
                self.local_env.get(name).cloned().ok_or_else(|| {
                    ScriptLangError::message(format!(
                        "module `{module_path}` does not export const `{name}`"
                    ))
                })
            } else {
                match self.resolver.resolve_qualified_const(&module_path, name)? {
                    QualifiedConstLookup::Value(value) => Ok(value),
                    QualifiedConstLookup::HiddenModule => Err(ScriptLangError::message(format!(
                        "module `{module_path}` is not imported into `{}`",
                        self.resolver.current_module()
                    ))),
                    QualifiedConstLookup::UnknownConst => Err(ScriptLangError::message(format!(
                        "module `{module_path}` does not export const `{name}`"
                    ))),
                    QualifiedConstLookup::NotModulePath => Err(ScriptLangError::message(format!(
                        "unsupported const reference `{}`; only visible const values are allowed",
                        segments.join(".")
                    ))),
                }
            }
        }
    }

    fn parse_script_literal(&mut self) -> Result<ConstValue, ScriptLangError> {
        self.expect_char('@')?;
        let mut literal = String::from("@");
        literal.push_str(&self.parse_ident()?);
        while self.consume_char('.') {
            literal.push('.');
            literal.push_str(&self.parse_ident()?);
        }
        let resolved = self.resolver.resolve_script_literal(&literal)?;
        Ok(ConstValue::Script(resolved))
    }

    fn parse_function_literal(&mut self) -> Result<ConstValue, ScriptLangError> {
        self.expect_char('#')?;
        let mut literal = String::from("#");
        literal.push_str(&self.parse_ident()?);
        while self.consume_char('.') {
            literal.push('.');
            literal.push_str(&self.parse_ident()?);
        }
        let resolved = self.resolver.resolve_function_literal(&literal)?;
        Ok(ConstValue::Function(resolved))
    }

    fn check_declared_type(&self, value: ConstValue) -> Result<ConstValue, ScriptLangError> {
        match self.expected_type {
            Some(DeclaredType::Array) if !matches!(value, ConstValue::Array(_)) => {
                Err(ScriptLangError::message(
                    "const declared as `array` must evaluate to an array literal",
                ))
            }
            Some(DeclaredType::Bool) if !matches!(value, ConstValue::Bool(_)) => {
                Err(ScriptLangError::message(
                    "const declared as `bool` must evaluate to a boolean literal",
                ))
            }
            Some(DeclaredType::Function) if !matches!(value, ConstValue::Function(_)) => {
                Err(ScriptLangError::message(
                    "const declared as `function` must evaluate to a function literal",
                ))
            }
            Some(DeclaredType::Int) if !matches!(value, ConstValue::Integer(_)) => {
                Err(ScriptLangError::message(
                    "const declared as `int` must evaluate to an integer literal",
                ))
            }
            Some(DeclaredType::Object) if !matches!(value, ConstValue::Object(_)) => {
                Err(ScriptLangError::message(
                    "const declared as `object` must evaluate to an object literal",
                ))
            }
            Some(DeclaredType::Script) if !matches!(value, ConstValue::Script(_)) => {
                Err(ScriptLangError::message(
                    "const declared as `script` must evaluate to a script literal",
                ))
            }
            Some(DeclaredType::String) if !matches!(value, ConstValue::String(_)) => {
                Err(ScriptLangError::message(
                    "const declared as `string` must evaluate to a string literal",
                ))
            }
            _ => Ok(value),
        }
    }

    fn parse_reference_path(&mut self) -> Result<Vec<String>, ScriptLangError> {
        let mut segments = vec![self.parse_ident()?];
        while self.consume_char('.') {
            segments.push(self.parse_ident()?);
        }
        Ok(segments)
    }

    fn parse_ident(&mut self) -> Result<String, ScriptLangError> {
        let Some(ch) = self.peek_char() else {
            return Err(ScriptLangError::message("unexpected end of input"));
        };
        if !is_ident_start(ch) {
            return Err(ScriptLangError::message(format!(
                "expected identifier, got `{ch}`"
            )));
        }
        let start = self.cursor;
        self.cursor += 1;
        while matches!(self.peek_char(), Some(ch) if is_ident_continue(ch)) {
            self.cursor += 1;
        }
        Ok(self.source[start..self.cursor].to_string())
    }

    fn parse_string(&mut self) -> Result<String, ScriptLangError> {
        let quote = self
            .peek_char()
            .ok_or_else(|| ScriptLangError::message("unexpected end of string literal"))?;
        self.cursor += 1;
        let mut value = String::new();
        while let Some(ch) = self.peek_char() {
            self.cursor += ch.len_utf8();
            if ch == quote {
                return Ok(value);
            }
            if ch == '\\' {
                let escaped = self
                    .peek_char()
                    .ok_or_else(|| ScriptLangError::message("unterminated escape sequence"))?;
                self.cursor += escaped.len_utf8();
                value.push(match escaped {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '\\' => '\\',
                    '\'' => '\'',
                    '"' => '"',
                    other => other,
                });
            } else {
                value.push(ch);
            }
        }
        Err(ScriptLangError::message("unterminated string literal"))
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek_char(), Some(ch) if ch.is_whitespace()) {
            self.cursor += self.peek_char().expect("checked above").len_utf8();
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.cursor..].chars().next()
    }

    fn peek_next_char(&self) -> Option<char> {
        self.source[self.cursor..].chars().nth(1)
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.cursor += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), ScriptLangError> {
        if self.consume_char(expected) {
            Ok(())
        } else {
            Err(ScriptLangError::message(format!(
                "expected `{expected}` in const expression"
            )))
        }
    }

    fn is_eof(&self) -> bool {
        self.cursor >= self.source.len()
    }
}

#[cfg(test)]
mod tests {
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

        fn resolve_short_const(
            &mut self,
            name: &str,
        ) -> Result<Option<ConstValue>, ScriptLangError> {
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
            visible_functions: BTreeSet::from([
                "main.pick".to_string(),
                "lib.math.pick".to_string(),
            ]),
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
        let local_env =
            BTreeMap::from([("name".to_string(), ConstValue::String("neo".to_string()))]);
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
}
