use sl_core::{CompiledText, CompiledTextPart, TextSegment, TextTemplate};

pub(crate) fn lower_text_template(template: &TextTemplate) -> CompiledText {
    CompiledText {
        parts: template
            .segments
            .iter()
            .map(|segment| match segment {
                TextSegment::Literal(text) => CompiledTextPart::Literal(text.clone()),
                TextSegment::Expr(expr) => CompiledTextPart::Expr(expr.clone()),
            })
            .collect(),
    }
}

pub(crate) fn parse_text_template(source: &str) -> TextTemplate {
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
        segments.push(TextSegment::Expr(
            source[expr_start..expr_end].trim().to_string(),
        ));
        cursor = expr_end + 1;
    }

    if cursor < source.len() {
        segments.push(TextSegment::Literal(source[cursor..].to_string()));
    }
    if segments.is_empty() {
        segments.push(TextSegment::Literal(source.to_string()));
    }

    TextTemplate { segments }
}

#[cfg(test)]
mod tests {
    use sl_core::{CompiledTextPart, TextSegment, TextTemplate};

    use super::{lower_text_template, parse_text_template};

    #[test]
    fn lower_text_template_preserves_literal_and_expression_segments() {
        let lowered = lower_text_template(&TextTemplate {
            segments: vec![
                TextSegment::Literal("hello ".to_string()),
                TextSegment::Expr("name".to_string()),
            ],
        });

        assert!(matches!(
            lowered.parts.as_slice(),
            [CompiledTextPart::Literal(text), CompiledTextPart::Expr(expr)]
                if text == "hello " && expr == "name"
        ));
    }

    #[test]
    fn parse_text_template_covers_literal_and_expression_shapes() {
        let empty = parse_text_template("");
        assert_eq!(empty.segments.len(), 1);
        assert!(matches!(&empty.segments[0], TextSegment::Literal(text) if text.is_empty()));

        let literal = parse_text_template("hello");
        assert!(matches!(&literal.segments[..], [TextSegment::Literal(text)] if text == "hello"));

        let expr_only = parse_text_template("${ value }");
        assert!(matches!(&expr_only.segments[..], [TextSegment::Expr(text)] if text == "value"));

        let unclosed = parse_text_template("hello ${name");
        assert!(
            matches!(&unclosed.segments[..], [TextSegment::Literal(text)] if text == "hello ${name")
        );

        let mixed = parse_text_template("a ${left} b ${ } c");
        assert_eq!(mixed.segments.len(), 5);
        assert!(matches!(&mixed.segments[0], TextSegment::Literal(text) if text == "a "));
        assert!(matches!(&mixed.segments[1], TextSegment::Expr(text) if text == "left"));
        assert!(matches!(&mixed.segments[2], TextSegment::Literal(text) if text == " b "));
        assert!(matches!(&mixed.segments[3], TextSegment::Expr(text) if text.is_empty()));
        assert!(matches!(&mixed.segments[4], TextSegment::Literal(text) if text == " c"));
    }
}
