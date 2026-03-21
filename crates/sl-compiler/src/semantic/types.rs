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
    pub(crate) declared_type: DeclaredType,
    pub(crate) expr: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ModulePath(pub(crate) String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DeclaredType {
    Array,
    Bool,
    Function,
    Int,
    Object,
    Script,
    String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MemberKind {
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
        declared_type: DeclaredType,
        expr: String,
    },
    Code {
        code: String,
    },
    Text {
        template: TextTemplate,
        tag: Option<String>,
    },
    While {
        when: String,
        body: Vec<SemanticStmt>,
        captures_loop_control: bool,
    },
    Break,
    Continue,
    Choice {
        prompt: Option<TextTemplate>,
        options: Vec<SemanticChoiceOption>,
    },
    Goto {
        expr: String,
    },
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticChoiceOption {
    pub(crate) text: TextTemplate,
    pub(crate) body: Vec<SemanticStmt>,
}
