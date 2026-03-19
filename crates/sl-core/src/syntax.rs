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

#[cfg(test)]
mod tests {
    use super::{
        ParsedChoiceOption, ParsedModule, ParsedScript, ParsedStmt, ParsedVar, TextSegment,
        TextTemplate,
    };

    #[test]
    fn syntax_types_cover_all_public_variants() {
        let template = TextTemplate {
            segments: vec![
                TextSegment::Literal("hello ".to_string()),
                TextSegment::Expr("name".to_string()),
            ],
        };
        let script = ParsedScript {
            name: "entry".to_string(),
            body: vec![
                ParsedStmt::Temp {
                    name: "tmp".to_string(),
                    expr: "1".to_string(),
                },
                ParsedStmt::Code {
                    code: "tmp = 2;".to_string(),
                },
                ParsedStmt::Text {
                    template: template.clone(),
                    tag: Some("tag".to_string()),
                },
                ParsedStmt::If {
                    when: "true".to_string(),
                    body: vec![ParsedStmt::End],
                },
                ParsedStmt::Choice {
                    prompt: Some(template.clone()),
                    options: vec![ParsedChoiceOption {
                        text: template.clone(),
                        body: vec![ParsedStmt::Goto {
                            target_script_ref: "main.next".to_string(),
                        }],
                    }],
                },
                ParsedStmt::Goto {
                    target_script_ref: "main.done".to_string(),
                },
                ParsedStmt::End,
            ],
        };
        let module = ParsedModule {
            name: "main".to_string(),
            vars: vec![ParsedVar {
                name: "answer".to_string(),
                expr: "42".to_string(),
            }],
            scripts: vec![script],
        };

        assert_eq!(module.name, "main");
        assert_eq!(module.vars[0].name, "answer");
        assert_eq!(module.vars[0].expr, "42");
        assert_eq!(module.scripts[0].name, "entry");
        assert!(matches!(
            &module.scripts[0].body[0],
            ParsedStmt::Temp { name, expr } if name == "tmp" && expr == "1"
        ));
        assert!(matches!(
            &module.scripts[0].body[1],
            ParsedStmt::Code { code } if code == "tmp = 2;"
        ));
        assert!(matches!(
            &module.scripts[0].body[2],
            ParsedStmt::Text {
                template: TextTemplate { segments },
                tag
            } if matches!(&segments[0], TextSegment::Literal(text) if text == "hello ")
                && matches!(&segments[1], TextSegment::Expr(expr) if expr == "name")
                && tag.as_deref() == Some("tag")
        ));
        assert!(matches!(
            &module.scripts[0].body[3],
            ParsedStmt::If { when, body } if when == "true" && matches!(body[0], ParsedStmt::End)
        ));
        assert!(matches!(
            &module.scripts[0].body[4],
            ParsedStmt::Choice { prompt: Some(TextTemplate { segments }), options }
                if matches!(&segments[1], TextSegment::Expr(expr) if expr == "name")
                    && matches!(&options[0].body[0], ParsedStmt::Goto { target_script_ref } if target_script_ref == "main.next")
        ));
        assert!(matches!(
            &module.scripts[0].body[5],
            ParsedStmt::Goto { target_script_ref } if target_script_ref == "main.done"
        ));
        assert!(matches!(&module.scripts[0].body[6], ParsedStmt::End));
    }
}
