use std::collections::{BTreeMap, BTreeSet};

use sl_core::{ScriptLangError, TextSegment, TextTemplate};

use super::resolve::QualifiedConstLookup;
use super::types::DeclaredType;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConstValue {
    Bool(bool),
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

pub(crate) fn rewrite_expr_with_consts<R: ConstLookup>(
    source: &str,
    local_env: &ConstEnv,
    resolver: &mut R,
    blocked_names: &BTreeSet<String>,
    shadowed_names: &BTreeSet<String>,
) -> Result<String, ScriptLangError> {
    let mut rewritten = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let ch = bytes[cursor] as char;
        if ch == '"' || ch == '\'' {
            let end = scan_quoted(bytes, cursor)?;
            rewritten.push_str(&source[cursor..end]);
            cursor = end;
            continue;
        }
        if is_ident_start(ch) {
            let (end, segments) = scan_reference_path(source, cursor);
            let raw = &source[cursor..end];
            let first = segments[0].as_str();

            if shadowed_names.contains(first) || is_property_access(bytes, cursor) {
                rewritten.push_str(raw);
                cursor = end;
                continue;
            }

            if segments.len() == 1 {
                let ident = first;
                if is_map_key(source, end) {
                    rewritten.push_str(ident);
                } else if let Some(value) = local_env.get(ident) {
                    rewritten.push_str(&value.to_rhai_literal());
                } else if let Some(value) = resolver.resolve_short_const(ident)? {
                    rewritten.push_str(&value.to_rhai_literal());
                } else if blocked_names.contains(ident) {
                    return Err(ScriptLangError::message(format!(
                        "const `{ident}` cannot be referenced before it is defined"
                    )));
                } else {
                    rewritten.push_str(ident);
                }
            } else {
                let module_path = segments[..segments.len() - 1].join(".");
                let name = segments.last().expect("qualified path");
                if module_path == resolver.current_module() {
                    if let Some(value) = local_env.get(name) {
                        rewritten.push_str(&value.to_rhai_literal());
                    } else {
                        rewritten.push_str(raw);
                    }
                } else {
                    match resolver.resolve_qualified_const(&module_path, name)? {
                        QualifiedConstLookup::Value(value) => {
                            rewritten.push_str(&value.to_rhai_literal());
                        }
                        QualifiedConstLookup::HiddenModule
                        | QualifiedConstLookup::UnknownConst
                        | QualifiedConstLookup::NotModulePath => rewritten.push_str(raw),
                    }
                }
            }

            cursor = end;
            continue;
        }
        rewritten.push(ch);
        cursor += ch.len_utf8();
    }

    Ok(rewritten)
}

pub(crate) fn rewrite_template_with_consts<R: ConstLookup>(
    template: TextTemplate,
    local_env: &ConstEnv,
    resolver: &mut R,
    blocked_names: &BTreeSet<String>,
    shadowed_names: &BTreeSet<String>,
) -> Result<TextTemplate, ScriptLangError> {
    let segments = template
        .segments
        .into_iter()
        .map(|segment| match segment {
            TextSegment::Literal(text) => Ok(TextSegment::Literal(text)),
            TextSegment::Expr(expr) => Ok(TextSegment::Expr(rewrite_expr_with_consts(
                &expr,
                local_env,
                resolver,
                blocked_names,
                shadowed_names,
            )?)),
        })
        .collect::<Result<Vec<_>, ScriptLangError>>()?;
    Ok(TextTemplate { segments })
}

impl ConstValue {
    pub(crate) fn to_rhai_literal(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
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

fn scan_quoted(bytes: &[u8], start: usize) -> Result<usize, ScriptLangError> {
    let quote = bytes[start];
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor += 2,
            ch if ch == quote => return Ok(cursor + 1),
            _ => cursor += 1,
        }
    }
    Err(ScriptLangError::message("unterminated string literal"))
}

fn scan_reference_path(source: &str, start: usize) -> (usize, Vec<String>) {
    let mut cursor = start;
    let mut segments = Vec::new();
    loop {
        let ident_start = cursor;
        cursor += 1;
        let bytes = source.as_bytes();
        while cursor < bytes.len() && is_ident_continue(bytes[cursor] as char) {
            cursor += 1;
        }
        segments.push(source[ident_start..cursor].to_string());
        if cursor >= bytes.len() || bytes[cursor] != b'.' {
            break;
        }
        let next = cursor + 1;
        if next >= bytes.len() || !is_ident_start(bytes[next] as char) {
            break;
        }
        cursor = next;
    }
    (cursor, segments)
}

fn is_property_access(bytes: &[u8], ident_start: usize) -> bool {
    let mut cursor = ident_start;
    while cursor > 0 {
        cursor -= 1;
        let ch = bytes[cursor] as char;
        if ch.is_whitespace() {
            continue;
        }
        return ch == '.';
    }
    false
}

fn is_map_key(source: &str, ident_end: usize) -> bool {
    let mut chars = source[ident_end..].chars();
    loop {
        match chars.next() {
            Some(ch) if ch.is_whitespace() => continue,
            Some(':') => return true,
            _ => return false,
        }
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::semantic::resolve::QualifiedConstLookup;
    use crate::semantic::types::DeclaredType;
    use sl_core::{ScriptLangError, TextSegment, TextTemplate};

    use super::{
        ConstEnv, ConstLookup, ConstValue, parse_const_value, rewrite_expr_with_consts,
        rewrite_template_with_consts,
    };

    struct TestResolver {
        current_module: String,
        imported_short_env: BTreeMap<String, ConstValue>,
        visible_modules: BTreeMap<String, ConstEnv>,
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
    }

    fn visible() -> TestResolver {
        TestResolver {
            current_module: "main".to_string(),
            imported_short_env: BTreeMap::from([("answer".to_string(), ConstValue::Integer(42))]),
            visible_modules: BTreeMap::from([(
                "lib.math".to_string(),
                BTreeMap::from([("zero".to_string(), ConstValue::Integer(0))]),
            )]),
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
}
