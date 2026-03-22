mod boot;
mod declarations;
mod lowering;
mod types;

use std::collections::BTreeMap;

use crate::pipeline::CompileOptions;
use crate::semantic::SemanticProgram;
use sl_core::{CompiledArtifact, CompiledScript, ScriptLangError};

use self::types::ProgramAssembler;

pub(crate) use self::types::ScriptDraft;

pub(crate) fn assemble_artifact(
    program: &SemanticProgram,
) -> Result<CompiledArtifact, ScriptLangError> {
    assemble_artifact_with_options(program, &CompileOptions::default())
}

pub(crate) fn assemble_artifact_with_options(
    program: &SemanticProgram,
    options: &CompileOptions,
) -> Result<CompiledArtifact, ScriptLangError> {
    let mut assembler = ProgramAssembler {
        functions: BTreeMap::new(),
        scripts: Vec::new(),
        script_refs: BTreeMap::new(),
        globals: Vec::new(),
    };

    assembler.collect_declarations(&program.modules)?;
    assembler.lower_modules(&program.modules)?;

    let default_entry_script_id = assembler
        .script_refs
        .get(options.default_entry_script_ref.as_str())
        .copied()
        .ok_or_else(|| {
            if assembler.scripts.is_empty() {
                ScriptLangError::message("no <script> declarations found")
            } else {
                ScriptLangError::message(format!(
                    "default entry script `{}` does not exist",
                    options.default_entry_script_ref
                ))
            }
        })?;

    let boot_script_id = assembler.scripts.len();
    let boot_script = assembler.build_boot_script(default_entry_script_id)?;
    let mut scripts = assembler
        .scripts
        .into_iter()
        .enumerate()
        .map(|(script_id, draft)| CompiledScript {
            script_id,
            local_names: draft.local_names,
            instructions: draft.instructions,
        })
        .collect::<Vec<_>>();
    scripts.push(CompiledScript {
        script_id: boot_script_id,
        local_names: Vec::new(),
        instructions: boot_script,
    });

    Ok(CompiledArtifact {
        default_entry_script_id,
        boot_script_id,
        functions: assembler.functions,
        script_refs: assembler.script_refs,
        scripts,
        globals: assembler
            .globals
            .into_iter()
            .map(|decl| decl.global)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use sl_core::{Instruction, TextSegment, TextTemplate};

    use crate::semantic::types::{DeclaredType, SemanticVar};
    use crate::semantic::{
        SemanticChoiceOption, SemanticModule, SemanticProgram, SemanticScript, SemanticStmt,
    };

    use crate::pipeline::CompileOptions;

    use super::{ProgramAssembler, assemble_artifact, assemble_artifact_with_options};

    fn program(modules: Vec<SemanticModule>) -> SemanticProgram {
        SemanticProgram { modules }
    }

    #[test]
    fn assemble_artifact_requires_at_least_one_script() {
        let error = assemble_artifact(&program(vec![SemanticModule {
            name: "main".to_string(),
            functions: Vec::new(),
            vars: Vec::new(),
            scripts: Vec::new(),
        }]))
        .expect_err("should fail");

        assert_eq!(error.to_string(), "no <script> declarations found");
    }

    #[test]
    fn assemble_artifact_requires_main_main_as_default_entry() {
        let error = assemble_artifact(&program(vec![SemanticModule {
            name: "main".to_string(),
            functions: Vec::new(),
            vars: Vec::new(),
            scripts: vec![SemanticScript {
                name: "entry".to_string(),
                body: vec![SemanticStmt::End],
            }],
        }]))
        .expect_err("missing main.main should fail");

        assert_eq!(
            error.to_string(),
            "default entry script `main.main` does not exist"
        );
    }

    #[test]
    fn assemble_artifact_allows_custom_default_entry_script() {
        let artifact = assemble_artifact_with_options(
            &program(vec![SemanticModule {
                name: "main".to_string(),
                functions: Vec::new(),
                vars: Vec::new(),
                scripts: vec![SemanticScript {
                    name: "entry".to_string(),
                    body: vec![SemanticStmt::End],
                }],
            }]),
            &CompileOptions {
                default_entry_script_ref: "main.entry".to_string(),
            },
        )
        .expect("custom entry should compile");

        assert_eq!(artifact.default_entry_script_id, 0);
    }

    #[test]
    fn assemble_artifact_collects_globals_and_lowers_scripts() {
        let artifact = assemble_artifact(&program(vec![SemanticModule {
            name: "main".to_string(),
            functions: Vec::new(),
            vars: vec![SemanticVar {
                name: "answer".to_string(),
                declared_type: DeclaredType::Int,
                expr: "40 + 2".to_string(),
            }],
            scripts: vec![
                SemanticScript {
                    name: "main".to_string(),
                    body: vec![
                        SemanticStmt::Temp {
                            name: "x".to_string(),
                            declared_type: DeclaredType::Int,
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
                                        expr: "\"main.target\"".to_string(),
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
        assert_eq!(artifact.globals[0].runtime_name, "__sl_globalmain_answer");
        assert!(matches!(
            &artifact.scripts[0].instructions[0],
            Instruction::EvalTemp { local_id, expr }
                if *local_id == 0 && expr.source == "1" && expr.referenced_vars.is_empty()
        ));
        assert!(matches!(
            &artifact.scripts[0].instructions[1],
            Instruction::BuildChoice { prompt, options }
                if prompt.is_some() && options.len() == 2
        ));
    }

    #[test]
    fn assemble_artifact_rejects_duplicate_script_refs_and_allows_same_named_globals() {
        let duplicate_script = assemble_artifact(&program(vec![SemanticModule {
            name: "main".to_string(),
            functions: Vec::new(),
            vars: Vec::new(),
            scripts: vec![
                SemanticScript {
                    name: "main".to_string(),
                    body: vec![SemanticStmt::End],
                },
                SemanticScript {
                    name: "main".to_string(),
                    body: vec![SemanticStmt::End],
                },
            ],
        }]))
        .expect_err("duplicate script should fail");
        assert_eq!(
            duplicate_script.to_string(),
            "duplicate script declaration `main.main`"
        );

        let artifact = assemble_artifact(&program(vec![
            SemanticModule {
                name: "main".to_string(),
                functions: Vec::new(),
                vars: vec![SemanticVar {
                    name: "name".to_string(),
                    declared_type: DeclaredType::Int,
                    expr: "1".to_string(),
                }],
                scripts: vec![SemanticScript {
                    name: "main".to_string(),
                    body: vec![SemanticStmt::End],
                }],
            },
            SemanticModule {
                name: "b".to_string(),
                functions: Vec::new(),
                vars: vec![SemanticVar {
                    name: "name".to_string(),
                    declared_type: DeclaredType::Int,
                    expr: "2".to_string(),
                }],
                scripts: vec![SemanticScript {
                    name: "main".to_string(),
                    body: vec![SemanticStmt::End],
                }],
            },
        ]))
        .expect("same short globals should be allowed");
        assert_eq!(artifact.globals.len(), 2);
        assert_eq!(artifact.globals[0].runtime_name, "__sl_globalmain_name");
        assert_eq!(artifact.globals[1].runtime_name, "__sl_globalb_name");
    }

    #[test]
    fn build_boot_script_is_empty_except_for_jump_when_no_globals_exist() {
        let builder = ProgramAssembler {
            functions: Default::default(),
            scripts: Vec::new(),
            script_refs: Default::default(),
            globals: Vec::new(),
        };

        assert!(matches!(
            builder.build_boot_script(3).expect("boot").as_slice(),
            [Instruction::JumpScript { target_script_id }] if *target_script_id == 3
        ));
    }
}
