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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ModulePath(pub(crate) String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MemberKind {
    Script,
    Var,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedRef {
    pub(crate) module_path: ModulePath,
    pub(crate) member_name: String,
    pub(crate) member_kind: MemberKind,
}

impl ResolvedRef {
    pub(crate) fn new(
        module_path: impl Into<String>,
        member_name: impl Into<String>,
        member_kind: MemberKind,
    ) -> Self {
        Self {
            module_path: ModulePath(module_path.into()),
            member_name: member_name.into(),
            member_kind,
        }
    }

    pub(crate) fn script(module_path: impl Into<String>, member_name: impl Into<String>) -> Self {
        Self::new(module_path, member_name, MemberKind::Script)
    }

    pub(crate) fn qualified_name(&self) -> String {
        format!("{}.{}", self.module_path.0, self.member_name)
    }
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
        target: ResolvedRef,
    },
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticChoiceOption {
    pub(crate) text: TextTemplate,
    pub(crate) body: Vec<SemanticStmt>,
}
