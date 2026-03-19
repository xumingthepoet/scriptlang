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

#[cfg(test)]
mod tests {
    use sl_core::{CompiledTextPart, TextSegment, TextTemplate};

    use super::lower_text_template;

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
}
