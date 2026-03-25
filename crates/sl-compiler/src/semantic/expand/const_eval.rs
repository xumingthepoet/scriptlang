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

pub(crate) struct ConstParser<'a, R: ConstLookup> {
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
#[path = "const_eval/tests/mod.rs"]
mod tests;
