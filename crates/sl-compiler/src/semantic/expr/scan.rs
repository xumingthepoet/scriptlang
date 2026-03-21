use sl_core::ScriptLangError;

use super::types::{ExprKind, ExprSource, SpecialToken, SpecialTokenKind};

pub(crate) fn scan_expr_source(
    source: &str,
    kind: ExprKind,
) -> Result<ExprSource, ScriptLangError> {
    let bytes = source.as_bytes();
    let mut cursor = 0usize;
    let mut tokens = Vec::new();

    while cursor < bytes.len() {
        let ch = bytes[cursor] as char;
        if ch == '"' || ch == '\'' {
            cursor = scan_quoted(bytes, cursor)?;
            continue;
        }
        if ch == '@' {
            let start = cursor;
            cursor += 1;
            if cursor >= bytes.len() || !is_ident_start(bytes[cursor] as char) {
                return Err(ScriptLangError::message(format!(
                    "invalid script literal `{}`",
                    &source[start..cursor]
                )));
            }
            while cursor < bytes.len() {
                let current = bytes[cursor] as char;
                if is_ident_continue(current) || current == '.' {
                    cursor += 1;
                } else {
                    break;
                }
            }
            tokens.push(SpecialToken {
                kind: SpecialTokenKind::ScriptLiteral,
                start,
                end: cursor,
            });
            continue;
        }
        if is_ident_start(ch) {
            let start = cursor;
            let (end, segments) = scan_reference_path(source, cursor);
            let kind = if segments.len() == 1 {
                SpecialTokenKind::IdentRef
            } else {
                SpecialTokenKind::QualifiedRef
            };
            tokens.push(SpecialToken { kind, start, end });
            cursor = end;
            continue;
        }
        cursor += ch.len_utf8();
    }

    let expr = match kind {
        ExprKind::Rhai => ExprSource::rhai(source),
        ExprKind::TemplateHole => ExprSource::template_hole(source),
    };
    Ok(expr.with_tokens(tokens))
}

pub(crate) fn scan_quoted(bytes: &[u8], start: usize) -> Result<usize, ScriptLangError> {
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

pub(crate) fn scan_reference_path(source: &str, start: usize) -> (usize, Vec<String>) {
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

pub(crate) fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

pub(crate) fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::scan_expr_source;
    use crate::semantic::expr::{ExprKind, SpecialTokenKind};

    #[test]
    fn scan_expr_source_tracks_script_literals_and_refs() {
        let expr = scan_expr_source(
            r#"target + @main.loop + user.name + "skip @quoted""#,
            ExprKind::Rhai,
        )
        .expect("scan");

        let kinds = expr
            .tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                SpecialTokenKind::IdentRef,
                SpecialTokenKind::ScriptLiteral,
                SpecialTokenKind::QualifiedRef,
            ]
        );
    }
}
