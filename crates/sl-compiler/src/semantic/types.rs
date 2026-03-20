use sl_core::TextTemplate;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticProgram {
    pub(crate) modules: Vec<SemanticModule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticModule {
    pub(crate) name: String,
    pub(crate) vars: Vec<SemanticVar>,
    pub(crate) scripts: Vec<SemanticScript>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticVar {
    pub(crate) name: String,
    pub(crate) expr: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticScript {
    pub(crate) name: String,
    pub(crate) body: Vec<SemanticStmt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SemanticStmt {
    Temp {
        name: String,
        expr: String,
    },
    Code {
        code: String,
    },
    Text {
        template: TextTemplate,
        tag: Option<String>,
    },
    If {
        when: String,
        body: Vec<SemanticStmt>,
    },
    Choice {
        prompt: Option<TextTemplate>,
        options: Vec<SemanticChoiceOption>,
    },
    Goto {
        target_script_ref: String,
    },
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticChoiceOption {
    pub(crate) text: TextTemplate,
    pub(crate) body: Vec<SemanticStmt>,
}
