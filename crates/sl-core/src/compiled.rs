use std::collections::BTreeMap;

use crate::{GlobalId, LocalId, ScriptId};

#[derive(Clone, Debug)]
pub struct CompiledArtifact {
    pub default_entry_script_id: ScriptId,
    pub boot_script_id: ScriptId,
    pub functions: BTreeMap<String, CompiledFunction>,
    pub script_refs: BTreeMap<String, ScriptId>,
    pub scripts: Vec<CompiledScript>,
    pub globals: Vec<GlobalVar>,
}

#[derive(Clone, Debug)]
pub struct CompiledScript {
    pub script_id: ScriptId,
    pub local_names: Vec<String>,
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug)]
pub struct GlobalVar {
    pub global_id: GlobalId,
    pub runtime_name: String,
}

#[derive(Clone, Debug)]
pub struct CompiledFunction {
    pub param_names: Vec<String>,
    pub body: CompiledExpr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledExpr {
    pub source: String,
    pub referenced_vars: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum Instruction {
    EvalGlobalInit {
        global_id: GlobalId,
        expr: CompiledExpr,
    },
    EvalTemp {
        local_id: LocalId,
        expr: CompiledExpr,
    },
    EvalCond {
        expr: CompiledExpr,
    },
    ExecCode {
        code: CompiledExpr,
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
    JumpScriptExpr {
        expr: CompiledExpr,
    },
    ReturnToHost,
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
    VarRef(String),
    Expr(CompiledExpr),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        ChoiceBranch, CompiledArtifact, CompiledExpr, CompiledFunction, CompiledScript,
        CompiledText, CompiledTextPart, GlobalVar, Instruction,
    };

    fn expr(source: &str, referenced_vars: &[&str]) -> CompiledExpr {
        CompiledExpr {
            source: source.to_string(),
            referenced_vars: referenced_vars
                .iter()
                .map(|name| name.to_string())
                .collect(),
        }
    }

    #[test]
    fn compiled_types_cover_all_instruction_variants() {
        let text = CompiledText {
            parts: vec![
                CompiledTextPart::Literal("hello".to_string()),
                CompiledTextPart::VarRef("name".to_string()),
                CompiledTextPart::Expr(expr("name + suffix", &["name", "suffix"])),
            ],
        };
        let instructions = vec![
            Instruction::EvalGlobalInit {
                global_id: 0,
                expr: expr("40 + 2", &[]),
            },
            Instruction::EvalTemp {
                local_id: 0,
                expr: expr("1", &[]),
            },
            Instruction::EvalCond {
                expr: expr("true", &[]),
            },
            Instruction::ExecCode {
                code: expr("x = 1;", &["x"]),
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
            Instruction::JumpScriptExpr {
                expr: expr("\"main.entry\"", &[]),
            },
            Instruction::ReturnToHost,
            Instruction::End,
        ];
        let artifact = CompiledArtifact {
            default_entry_script_id: 0,
            boot_script_id: 1,
            functions: BTreeMap::from([(
                "main.pick".to_string(),
                CompiledFunction {
                    param_names: vec!["x".to_string()],
                    body: expr("return x + 1;", &["x"]),
                },
            )]),
            script_refs: BTreeMap::from([
                ("main.entry".to_string(), 0),
                ("__boot__".to_string(), 1),
            ]),
            scripts: vec![
                CompiledScript {
                    script_id: 0,
                    local_names: vec!["x".to_string()],
                    instructions: instructions.clone(),
                },
                CompiledScript {
                    script_id: 1,
                    local_names: Vec::new(),
                    instructions: vec![Instruction::End],
                },
            ],
            globals: vec![GlobalVar {
                global_id: 0,
                runtime_name: "__sl_global__main__answer".to_string(),
            }],
        };

        assert_eq!(artifact.default_entry_script_id, 0);
        assert_eq!(artifact.boot_script_id, 1);
        assert_eq!(
            artifact.functions["main.pick"].param_names,
            vec!["x".to_string()]
        );
        assert_eq!(artifact.scripts[0].script_id, 0);
        assert_eq!(artifact.scripts[0].local_names, vec!["x".to_string()]);
        assert_eq!(
            artifact.globals[0].runtime_name,
            "__sl_global__main__answer"
        );
        assert_eq!(artifact.script_refs["main.entry"], 0);

        assert!(matches!(
            &instructions[0],
            Instruction::EvalGlobalInit { global_id, expr }
                if *global_id == 0 && expr.source == "40 + 2" && expr.referenced_vars.is_empty()
        ));
        assert!(matches!(
            &instructions[1],
            Instruction::EvalTemp { local_id, expr }
                if *local_id == 0 && expr.source == "1" && expr.referenced_vars.is_empty()
        ));
        assert!(matches!(
            &instructions[2],
            Instruction::EvalCond { expr } if expr.source == "true" && expr.referenced_vars.is_empty()
        ));
        assert!(matches!(
            &instructions[3],
            Instruction::ExecCode { code }
                if code.source == "x = 1;" && code.referenced_vars == vec!["x".to_string()]
        ));
        assert!(matches!(
            &instructions[4],
            Instruction::EmitText {
                text: CompiledText { parts },
                tag
            } if matches!(&parts[0], CompiledTextPart::Literal(text) if text == "hello")
                && matches!(&parts[1], CompiledTextPart::VarRef(name) if name == "name")
                && matches!(&parts[2], CompiledTextPart::Expr(expr) if expr.source == "name + suffix")
                && tag.as_deref() == Some("tag")
        ));
        assert!(matches!(
            &instructions[5],
            Instruction::BuildChoice {
                prompt: Some(CompiledText { parts }),
                options
            } if matches!(&parts[0], CompiledTextPart::Literal(text) if text == "hello")
                && matches!(&options[0].text.parts[1], CompiledTextPart::VarRef(name) if name == "name")
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
        assert!(matches!(
            &instructions[9],
            Instruction::JumpScriptExpr { expr } if expr.source == "\"main.entry\""
        ));
        assert!(matches!(&instructions[10], Instruction::ReturnToHost));
        assert!(matches!(&instructions[11], Instruction::End));
    }
}
