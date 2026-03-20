use std::collections::{BTreeMap, BTreeSet};

use sl_core::{ScriptLangError, TextSegment, TextTemplate};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConstValue {
    Bool(bool),
    Integer(i64),
    String(String),
    Array(Vec<ConstValue>),
    Object(BTreeMap<String, ConstValue>),
}

pub(crate) type ConstEnv = BTreeMap<String, ConstValue>;

pub(crate) fn parse_const_value(
    source: &str,
    env: &ConstEnv,
    blocked_names: &BTreeSet<String>,
) -> Result<ConstValue, ScriptLangError> {
    let mut parser = ConstParser {
        source,
        cursor: 0,
        env,
        blocked_names,
    };
    let value = parser.parse_value()?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(ScriptLangError::message(format!(
            "unexpected trailing tokens in const expression `{source}`"
        )));
    }
    Ok(value)
}

pub(crate) fn rewrite_expr_with_consts(
    source: &str,
    env: &ConstEnv,
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
            let start = cursor;
            cursor += 1;
            while cursor < bytes.len() && is_ident_continue(bytes[cursor] as char) {
                cursor += 1;
            }
            let ident = &source[start..cursor];
            if is_property_access(bytes, start)
                || is_map_key(source, cursor)
                || shadowed_names.contains(ident)
            {
                rewritten.push_str(ident);
                continue;
            }
            if blocked_names.contains(ident) {
                return Err(ScriptLangError::message(format!(
                    "const `{ident}` cannot be referenced before it is defined"
                )));
            }
            if let Some(value) = env.get(ident) {
                rewritten.push_str(&value.to_rhai_literal());
            } else {
                rewritten.push_str(ident);
            }
            continue;
        }
        rewritten.push(ch);
        cursor += ch.len_utf8();
    }

    Ok(rewritten)
}

pub(crate) fn rewrite_template_with_consts(
    template: TextTemplate,
    env: &ConstEnv,
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
                env,
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

struct ConstParser<'a> {
    source: &'a str,
    cursor: usize,
    env: &'a ConstEnv,
    blocked_names: &'a BTreeSet<String>,
}

impl<'a> ConstParser<'a> {
    fn parse_value(&mut self) -> Result<ConstValue, ScriptLangError> {
        self.skip_ws();
        let Some(ch) = self.peek_char() else {
            return Err(ScriptLangError::message("empty const expression"));
        };
        match ch {
            '"' | '\'' => Ok(ConstValue::String(self.parse_string()?)),
            '[' => self.parse_array(),
            '#' => self.parse_object(),
            '-' | '0'..='9' => self.parse_number(),
            _ if is_ident_start(ch) => self.parse_identifier_value(),
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
            Some(ch) if is_ident_start(ch) => self.parse_identifier(),
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

    fn parse_identifier_value(&mut self) -> Result<ConstValue, ScriptLangError> {
        let ident = self.parse_identifier()?;
        match ident.as_str() {
            "true" => Ok(ConstValue::Bool(true)),
            "false" => Ok(ConstValue::Bool(false)),
            other => {
                if self.blocked_names.contains(other) {
                    return Err(ScriptLangError::message(format!(
                        "const `{other}` cannot be referenced before it is defined"
                    )));
                }
                self.env.get(other).cloned().ok_or_else(|| {
                    ScriptLangError::message(format!(
                        "unsupported const reference `{other}`; only previously defined const values are allowed"
                    ))
                })
            }
        }
    }

    fn parse_identifier(&mut self) -> Result<String, ScriptLangError> {
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

    use sl_core::{TextSegment, TextTemplate};

    use super::{
        ConstValue, parse_const_value, rewrite_expr_with_consts, rewrite_template_with_consts,
    };

    #[test]
    fn parse_const_value_supports_literals_containers_and_const_refs() {
        let env = BTreeMap::from([("answer".to_string(), ConstValue::Integer(42))]);

        assert_eq!(
            parse_const_value(r#"[answer, true, "ok"]"#, &env, &BTreeSet::new()).expect("parse"),
            ConstValue::Array(vec![
                ConstValue::Integer(42),
                ConstValue::Bool(true),
                ConstValue::String("ok".to_string()),
            ])
        );
        assert_eq!(
            parse_const_value("#{foo: answer}", &env, &BTreeSet::new()).expect("parse"),
            ConstValue::Object(BTreeMap::from([(
                "foo".to_string(),
                ConstValue::Integer(42)
            )]))
        );
    }

    #[test]
    fn parse_const_value_rejects_forward_refs_and_unsupported_shapes() {
        let blocked = BTreeSet::from(["later".to_string()]);
        assert!(
            parse_const_value("later", &BTreeMap::new(), &blocked)
                .expect_err("forward ref")
                .to_string()
                .contains("cannot be referenced before it is defined")
        );
        assert!(
            parse_const_value("call()", &BTreeMap::new(), &BTreeSet::new())
                .expect_err("call")
                .to_string()
                .contains("unsupported const reference `call`")
        );
        assert!(
            parse_const_value("1.5", &BTreeMap::new(), &BTreeSet::new())
                .expect_err("float")
                .to_string()
                .contains("float const literals are not supported")
        );
    }

    #[test]
    fn rewrite_helpers_replace_only_identifier_refs() {
        let env = BTreeMap::from([
            ("answer".to_string(), ConstValue::Integer(42)),
            ("name".to_string(), ConstValue::String("neo".to_string())),
        ]);
        let rewritten = rewrite_expr_with_consts(
            r##"answer + answer_more + obj.answer + "#{answer}" + #{answer: answer} + name"##,
            &env,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .expect("rewrite");
        assert_eq!(
            rewritten,
            r##"42 + answer_more + obj.answer + "#{answer}" + #{answer: 42} + "neo""##
        );

        let template = rewrite_template_with_consts(
            TextTemplate {
                segments: vec![
                    TextSegment::Literal("x=".to_string()),
                    TextSegment::Expr("answer".to_string()),
                ],
            },
            &env,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .expect("template");
        assert!(matches!(
            &template.segments[1],
            TextSegment::Expr(expr) if expr == "42"
        ));
    }
}
