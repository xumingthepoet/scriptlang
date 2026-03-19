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
