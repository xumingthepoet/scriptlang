mod boot;
mod declarations;
mod lowering;
mod types;

use std::collections::BTreeMap;

use crate::semantic::SemanticProgram;
use sl_core::{CompiledArtifact, CompiledScript, ScriptLangError};

use self::types::ProgramAssembler;

pub(crate) use self::types::ScriptDraft;

pub(crate) fn assemble_artifact(
    program: &SemanticProgram,
) -> Result<CompiledArtifact, ScriptLangError> {
    let mut assembler = ProgramAssembler {
        scripts: Vec::new(),
        script_refs: BTreeMap::new(),
        globals: Vec::new(),
        default_entry_script_id: None,
    };

    assembler.collect_declarations(&program.modules)?;
    assembler.lower_modules(&program.modules)?;

    let default_entry_script_id = assembler
        .default_entry_script_id
        .ok_or_else(|| ScriptLangError::message("no <script> declarations found"))?;

    let boot_script_id = assembler.scripts.len();
    let boot_script = assembler.build_boot_script(default_entry_script_id);
    let mut scripts = assembler
        .scripts
        .into_iter()
        .enumerate()
        .map(|(script_id, draft)| CompiledScript {
            script_id,
            script_ref: draft.script_ref,
            local_names: draft.local_names,
            instructions: draft.instructions,
        })
        .collect::<Vec<_>>();
    scripts.push(CompiledScript {
        script_id: boot_script_id,
        script_ref: "__boot__".to_string(),
        local_names: Vec::new(),
        instructions: boot_script,
    });

    Ok(CompiledArtifact {
        default_entry_script_id,
        boot_script_id,
        script_refs: assembler.script_refs,
        scripts,
        globals: assembler.globals,
    })
}

#[cfg(test)]
mod tests {
    use sl_core::{Instruction, TextSegment, TextTemplate};

    use crate::semantic::types::SemanticVar;
    use crate::semantic::{
        SemanticChoiceOption, SemanticModule, SemanticProgram, SemanticScript, SemanticStmt,
    };

    use super::{ProgramAssembler, assemble_artifact};

    fn program(modules: Vec<SemanticModule>) -> SemanticProgram {
        SemanticProgram { modules }
    }

    #[test]
    fn assemble_artifact_requires_at_least_one_script() {
        let error = assemble_artifact(&program(vec![SemanticModule {
            name: "main".to_string(),
            vars: Vec::new(),
            scripts: Vec::new(),
        }]))
        .expect_err("should fail");

        assert_eq!(error.to_string(), "no <script> declarations found");
    }

    #[test]
    fn assemble_artifact_collects_globals_and_lowers_scripts() {
        let artifact = assemble_artifact(&program(vec![SemanticModule {
            name: "main".to_string(),
            vars: vec![SemanticVar {
                name: "answer".to_string(),
                expr: "40 + 2".to_string(),
            }],
            scripts: vec![
                SemanticScript {
                    name: "entry".to_string(),
                    body: vec![
                        SemanticStmt::Temp {
                            name: "x".to_string(),
                            expr: "1".to_string(),
                        },
                        SemanticStmt::Choice {
                            prompt: Some(TextTemplate {
                                segments: vec![TextSegment::Literal("pick".to_string())],
                            }),
                            options: vec![
                                SemanticChoiceOption {
                                    text: TextTemplate {
                                        segments: vec![TextSegment::Literal("left".to_string())],
                                    },
                                    body: vec![SemanticStmt::Goto {
                                        target_script_ref: "target".to_string(),
                                    }],
                                },
                                SemanticChoiceOption {
                                    text: TextTemplate {
                                        segments: vec![TextSegment::Literal("right".to_string())],
                                    },
                                    body: vec![SemanticStmt::End],
                                },
                            ],
                        },
                    ],
                },
                SemanticScript {
                    name: "target".to_string(),
                    body: vec![SemanticStmt::End],
                },
            ],
        }]))
        .expect("lower");

        assert_eq!(artifact.default_entry_script_id, 0);
        assert_eq!(artifact.boot_script_id, 2);
        assert_eq!(artifact.globals[0].qualified_name, "main.answer");
        assert!(matches!(
            &artifact.scripts[0].instructions[0],
            Instruction::EvalTemp { local_id, expr } if *local_id == 0 && expr == "1"
        ));
        assert!(matches!(
            &artifact.scripts[0].instructions[1],
            Instruction::BuildChoice { prompt, options }
                if prompt.is_some() && options.len() == 2
        ));
    }

    #[test]
    fn assemble_artifact_rejects_duplicate_script_refs_and_ambiguous_globals() {
        let duplicate_script = assemble_artifact(&program(vec![SemanticModule {
            name: "main".to_string(),
            vars: Vec::new(),
            scripts: vec![
                SemanticScript {
                    name: "entry".to_string(),
                    body: vec![SemanticStmt::End],
                },
                SemanticScript {
                    name: "entry".to_string(),
                    body: vec![SemanticStmt::End],
                },
            ],
        }]))
        .expect_err("duplicate script should fail");
        assert_eq!(
            duplicate_script.to_string(),
            "duplicate script declaration `main.entry`"
        );

        let ambiguous_global = assemble_artifact(&program(vec![
            SemanticModule {
                name: "a".to_string(),
                vars: vec![SemanticVar {
                    name: "name".to_string(),
                    expr: "1".to_string(),
                }],
                scripts: vec![SemanticScript {
                    name: "entry".to_string(),
                    body: vec![SemanticStmt::End],
                }],
            },
            SemanticModule {
                name: "b".to_string(),
                vars: vec![SemanticVar {
                    name: "name".to_string(),
                    expr: "2".to_string(),
                }],
                scripts: vec![SemanticScript {
                    name: "entry".to_string(),
                    body: vec![SemanticStmt::End],
                }],
            },
        ]))
        .expect_err("ambiguous globals should fail");
        assert_eq!(
            ambiguous_global.to_string(),
            "global short name `name` is ambiguous between `a.name` and `b.name`"
        );
    }

    #[test]
    fn build_boot_script_is_empty_except_for_jump_when_no_globals_exist() {
        let builder = ProgramAssembler {
            scripts: Vec::new(),
            script_refs: Default::default(),
            globals: Vec::new(),
            default_entry_script_id: Some(0),
        };

        assert!(matches!(
            builder.build_boot_script(3).as_slice(),
            [Instruction::JumpScript { target_script_id }] if *target_script_id == 3
        ));
    }
}
