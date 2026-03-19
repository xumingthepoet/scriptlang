#[derive(Clone, Debug)]
pub struct ParsedModule {
    pub name: String,
    pub vars: Vec<ParsedVar>,
    pub scripts: Vec<ParsedScript>,
}

#[derive(Clone, Debug)]
pub struct ParsedVar {
    pub name: String,
    pub expr: String,
}

#[derive(Clone, Debug)]
pub struct ParsedScript {
    pub name: String,
    pub body: Vec<ParsedStmt>,
}

#[derive(Clone, Debug)]
pub enum ParsedStmt {
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
        body: Vec<ParsedStmt>,
    },
    Choice {
        prompt: Option<TextTemplate>,
        options: Vec<ParsedChoiceOption>,
    },
    Goto {
        target_script_ref: String,
    },
    End,
}

#[derive(Clone, Debug)]
pub struct ParsedChoiceOption {
    pub text: TextTemplate,
    pub body: Vec<ParsedStmt>,
}

#[derive(Clone, Debug)]
pub struct TextTemplate {
    pub segments: Vec<TextSegment>,
}

#[derive(Clone, Debug)]
pub enum TextSegment {
    Literal(String),
    Expr(String),
}
