#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExprKind {
    Rhai,
    TemplateHole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpecialTokenKind {
    FunctionLiteral,
    ScriptLiteral,
    IdentRef,
    QualifiedRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpecialToken {
    pub(crate) kind: SpecialTokenKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExprSource {
    pub(crate) raw: String,
    pub(crate) kind: ExprKind,
    pub(crate) tokens: Vec<SpecialToken>,
}

impl ExprSource {
    pub(crate) fn rhai(raw: impl Into<String>) -> Self {
        Self {
            raw: raw.into(),
            kind: ExprKind::Rhai,
            tokens: Vec::new(),
        }
    }

    pub(crate) fn template_hole(raw: impl Into<String>) -> Self {
        Self {
            raw: raw.into(),
            kind: ExprKind::TemplateHole,
            tokens: Vec::new(),
        }
    }

    pub(crate) fn with_tokens(mut self, tokens: Vec<SpecialToken>) -> Self {
        self.tokens = tokens;
        self
    }
}
