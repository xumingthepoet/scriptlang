use sl_core::{ScriptLangError, TextSegment, TextTemplate};

use super::types::{ExprKind, ExprSource};
use super::{normalize_expr_escapes, scan::scan_expr_source};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TemplateExprHole {
    pub(crate) expr: ExprSource,
}

impl TemplateExprHole {
    pub(crate) fn new(expr: ExprSource) -> Self {
        Self { expr }
    }
}

pub(crate) fn parse_text_template(source: &str) -> Result<TextTemplate, ScriptLangError> {
    let mut segments = Vec::new();
    let mut cursor = 0usize;

    while let Some(start_offset) = source[cursor..].find("${") {
        let start = cursor + start_offset;
        if start > cursor {
            segments.push(TextSegment::Literal(source[cursor..start].to_string()));
        }

        let expr_start = start + 2;
        let Some(end_offset) = source[expr_start..].find('}') else {
            if let Some(TextSegment::Literal(prefix)) = segments.last_mut() {
                prefix.push_str(&source[start..]);
            } else {
                segments.push(TextSegment::Literal(source[start..].to_string()));
            }
            cursor = source.len();
            break;
        };
        let expr_end = expr_start + end_offset;
        let normalized = normalize_expr_escapes(source[expr_start..expr_end].trim())?;
        let hole = TemplateExprHole::new(scan_expr_source(&normalized, ExprKind::TemplateHole)?);
        segments.push(TextSegment::Expr(hole.expr.raw));
        cursor = expr_end + 1;
    }

    if cursor < source.len() {
        segments.push(TextSegment::Literal(source[cursor..].to_string()));
    }
    if segments.is_empty() {
        segments.push(TextSegment::Literal(source.to_string()));
    }

    Ok(TextTemplate { segments })
}

#[cfg(test)]
mod tests {
    use sl_core::TextSegment;

    use super::parse_text_template;

    #[test]
    fn parse_text_template_covers_literal_and_expression_shapes() {
        let mixed = parse_text_template("a ${left} b").expect("template");
        assert_eq!(
            mixed.segments,
            vec![
                TextSegment::Literal("a ".to_string()),
                TextSegment::Expr("left".to_string()),
                TextSegment::Literal(" b".to_string()),
            ]
        );

        let plain = parse_text_template("hello").expect("plain");
        assert_eq!(
            plain.segments,
            vec![TextSegment::Literal("hello".to_string())]
        );

        let dangling = parse_text_template("start ${unterminated").expect("dangling");
        assert_eq!(
            dangling.segments,
            vec![TextSegment::Literal("start ${unterminated".to_string())]
        );

        let normalized = parse_text_template("a ${hp LTE 1 AND ready} b").expect("normalized");
        assert_eq!(
            normalized.segments,
            vec![
                TextSegment::Literal("a ".to_string()),
                TextSegment::Expr("hp <= 1 && ready".to_string()),
                TextSegment::Literal(" b".to_string()),
            ]
        );
    }
}
