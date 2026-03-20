use std::collections::{BTreeMap, HashMap};

use sl_core::{
    CompiledArtifact, CompiledScript, GlobalVar, Instruction, LocalId, ScriptId, ScriptLangError,
};

use crate::lower::lower_script;
use crate::semantic::{SemanticModule, SemanticProgram};

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

pub(crate) struct ProgramAssembler {
    pub(crate) scripts: Vec<ScriptDraft>,
    pub(crate) script_refs: BTreeMap<String, ScriptId>,
    pub(crate) globals: Vec<GlobalVar>,
    pub(crate) default_entry_script_id: Option<ScriptId>,
}

#[derive(Clone)]
pub(crate) struct ScriptDraft {
    pub(crate) script_ref: String,
    pub(crate) local_names: Vec<String>,
    pub(crate) local_lookup: HashMap<String, LocalId>,
    pub(crate) instructions: Vec<Instruction>,
}

impl ProgramAssembler {
    fn collect_declarations(&mut self, modules: &[SemanticModule]) -> Result<(), ScriptLangError> {
        let mut global_short_names = HashMap::<String, String>::new();

        for module in modules {
            let _ = &module.consts;
            for var in &module.vars {
                let qualified_name = format!("{}.{}", module.name, var.name);
                if let Some(existing) =
                    global_short_names.insert(var.name.clone(), qualified_name.clone())
                {
                    return Err(ScriptLangError::message(format!(
                        "global short name `{}` is ambiguous between `{existing}` and `{qualified_name}`",
                        var.name
                    )));
                }
                self.globals.push(GlobalVar {
                    global_id: self.globals.len(),
                    qualified_name,
                    short_name: var.name.clone(),
                    initializer: var.expr.clone(),
                });
            }

            for script in &module.scripts {
                let script_ref = format!("{}.{}", module.name, script.name);
                if self.script_refs.contains_key(&script_ref) {
                    return Err(ScriptLangError::message(format!(
                        "duplicate script declaration `{script_ref}`"
                    )));
                }
                let script_id = self.scripts.len();
                self.script_refs.insert(script_ref.clone(), script_id);
                if self.default_entry_script_id.is_none() {
                    self.default_entry_script_id = Some(script_id);
                }
                self.scripts.push(ScriptDraft {
                    script_ref,
                    local_names: Vec::new(),
                    local_lookup: HashMap::new(),
                    instructions: Vec::new(),
                });
            }
        }

        Ok(())
    }

    fn lower_modules(&mut self, modules: &[SemanticModule]) -> Result<(), ScriptLangError> {
        let mut script_index = 0usize;
        for module in modules {
            for script in &module.scripts {
                let mut draft = self.scripts[script_index].clone();
                lower_script(&self.script_refs, &mut draft, &module.name, script)?;
                self.scripts[script_index] = draft;
                script_index += 1;
            }
        }
        Ok(())
    }

    fn build_boot_script(&self, default_entry_script_id: ScriptId) -> Vec<Instruction> {
        let mut instructions = Vec::with_capacity(self.globals.len() + 2);
        for global in &self.globals {
            instructions.push(Instruction::EvalGlobalInit {
                global_id: global.global_id,
                expr: global.initializer.clone(),
            });
        }
        instructions.push(Instruction::JumpScript {
            target_script_id: default_entry_script_id,
        });
        instructions
    }
}

#[cfg(test)]
mod tests {
    use sl_core::{Instruction, TextSegment, TextTemplate};

    use crate::semantic::{
        SemanticChoiceOption, SemanticModule, SemanticProgram, SemanticScript, SemanticStmt,
        SemanticVar,
    };

    use super::{ProgramAssembler, assemble_artifact};

    fn program(modules: Vec<SemanticModule>) -> SemanticProgram {
        SemanticProgram { modules }
    }

    #[test]
    fn assemble_artifact_requires_at_least_one_script() {
        let error = assemble_artifact(&program(vec![SemanticModule {
            name: "main".to_string(),
            consts: Vec::new(),
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
            consts: Vec::new(),
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
            consts: Vec::new(),
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
                consts: Vec::new(),
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
                consts: Vec::new(),
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
