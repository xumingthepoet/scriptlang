use std::collections::BTreeMap;

use crate::{GlobalId, LocalId, ScriptId};

#[derive(Clone, Debug)]
pub struct CompiledArtifact {
    pub default_entry_script_id: ScriptId,
    pub boot_script_id: ScriptId,
    pub script_refs: BTreeMap<String, ScriptId>,
    pub scripts: Vec<CompiledScript>,
    pub globals: Vec<GlobalVar>,
}

#[derive(Clone, Debug)]
pub struct CompiledScript {
    pub script_id: ScriptId,
    pub script_ref: String,
    pub local_names: Vec<String>,
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug)]
pub struct GlobalVar {
    pub global_id: GlobalId,
    pub qualified_name: String,
    pub short_name: String,
    pub runtime_name: String,
    pub initializer: String,
}

#[derive(Clone, Debug)]
pub enum Instruction {
    EvalGlobalInit {
        global_id: GlobalId,
        expr: String,
    },
    EvalTemp {
        local_id: LocalId,
        expr: String,
    },
    EvalCond {
        expr: String,
    },
    ExecCode {
        code: String,
    },
    EmitText {
        text: CompiledText,
        tag: Option<String>,
    },
    BuildChoice {
        prompt: Option<CompiledText>,
        options: Vec<ChoiceBranch>,
    },
    JumpIfFalse {
        target_pc: usize,
    },
    Jump {
        target_pc: usize,
    },
    JumpScript {
        target_script_id: ScriptId,
    },
    End,
}

#[derive(Clone, Debug)]
pub struct ChoiceBranch {
    pub text: CompiledText,
    pub target_pc: usize,
}

#[derive(Clone, Debug)]
pub struct CompiledText {
    pub parts: Vec<CompiledTextPart>,
}

#[derive(Clone, Debug)]
pub enum CompiledTextPart {
    Literal(String),
    Expr(String),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        ChoiceBranch, CompiledArtifact, CompiledScript, CompiledText, CompiledTextPart, GlobalVar,
        Instruction,
    };

    #[test]
    fn compiled_types_cover_all_instruction_variants() {
        let text = CompiledText {
            parts: vec![
                CompiledTextPart::Literal("hello".to_string()),
                CompiledTextPart::Expr("name".to_string()),
            ],
        };
        let instructions = vec![
            Instruction::EvalGlobalInit {
                global_id: 0,
                expr: "40 + 2".to_string(),
            },
            Instruction::EvalTemp {
                local_id: 0,
                expr: "1".to_string(),
            },
            Instruction::EvalCond {
                expr: "true".to_string(),
            },
            Instruction::ExecCode {
                code: "x = 1;".to_string(),
            },
            Instruction::EmitText {
                text: text.clone(),
                tag: Some("tag".to_string()),
            },
            Instruction::BuildChoice {
                prompt: Some(text.clone()),
                options: vec![ChoiceBranch {
                    text: text.clone(),
                    target_pc: 9,
                }],
            },
            Instruction::JumpIfFalse { target_pc: 10 },
            Instruction::Jump { target_pc: 11 },
            Instruction::JumpScript {
                target_script_id: 1,
            },
            Instruction::End,
        ];
        let artifact = CompiledArtifact {
            default_entry_script_id: 0,
            boot_script_id: 1,
            script_refs: BTreeMap::from([
                ("main.entry".to_string(), 0),
                ("__boot__".to_string(), 1),
            ]),
            scripts: vec![
                CompiledScript {
                    script_id: 0,
                    script_ref: "main.entry".to_string(),
                    local_names: vec!["x".to_string()],
                    instructions: instructions.clone(),
                },
                CompiledScript {
                    script_id: 1,
                    script_ref: "__boot__".to_string(),
                    local_names: Vec::new(),
                    instructions: vec![Instruction::End],
                },
            ],
            globals: vec![GlobalVar {
                global_id: 0,
                qualified_name: "main.answer".to_string(),
                short_name: "answer".to_string(),
                runtime_name: "__sl_global__main__answer".to_string(),
                initializer: "42".to_string(),
            }],
        };

        assert_eq!(artifact.default_entry_script_id, 0);
        assert_eq!(artifact.boot_script_id, 1);
        assert_eq!(artifact.scripts[0].script_id, 0);
        assert_eq!(artifact.scripts[0].script_ref, "main.entry");
        assert_eq!(artifact.scripts[0].local_names, vec!["x".to_string()]);
        assert_eq!(artifact.globals[0].qualified_name, "main.answer");
        assert_eq!(artifact.globals[0].short_name, "answer");
        assert_eq!(
            artifact.globals[0].runtime_name,
            "__sl_global__main__answer"
        );
        assert_eq!(artifact.globals[0].initializer, "42");
        assert_eq!(artifact.script_refs["main.entry"], 0);

        assert!(matches!(
            &instructions[0],
            Instruction::EvalGlobalInit { global_id, expr }
                if *global_id == 0 && expr == "40 + 2"
        ));
        assert!(matches!(
            &instructions[1],
            Instruction::EvalTemp { local_id, expr }
                if *local_id == 0 && expr == "1"
        ));
        assert!(matches!(
            &instructions[2],
            Instruction::EvalCond { expr } if expr == "true"
        ));
        assert!(matches!(
            &instructions[3],
            Instruction::ExecCode { code } if code == "x = 1;"
        ));
        assert!(matches!(
            &instructions[4],
            Instruction::EmitText {
                text: CompiledText { parts },
                tag
            } if matches!(&parts[0], CompiledTextPart::Literal(text) if text == "hello")
                && matches!(&parts[1], CompiledTextPart::Expr(expr) if expr == "name")
                && tag.as_deref() == Some("tag")
        ));
        assert!(matches!(
            &instructions[5],
            Instruction::BuildChoice {
                prompt: Some(CompiledText { parts }),
                options
            } if matches!(&parts[0], CompiledTextPart::Literal(text) if text == "hello")
                && matches!(&options[0].text.parts[1], CompiledTextPart::Expr(expr) if expr == "name")
                && options[0].target_pc == 9
        ));
        assert!(matches!(
            &instructions[6],
            Instruction::JumpIfFalse { target_pc } if *target_pc == 10
        ));
        assert!(matches!(
            &instructions[7],
            Instruction::Jump { target_pc } if *target_pc == 11
        ));
        assert!(matches!(
            &instructions[8],
            Instruction::JumpScript { target_script_id } if *target_script_id == 1
        ));
        assert!(matches!(&instructions[9], Instruction::End));
    }
}
