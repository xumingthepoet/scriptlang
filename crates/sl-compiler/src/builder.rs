use std::collections::{BTreeMap, HashMap};

use sl_core::{
    CompiledArtifact, CompiledScript, GlobalVar, Instruction, LocalId, ParsedModule, ScriptId,
    ScriptLangError,
};

use crate::lower::lower_script;

pub fn compile_artifact(
    parsed_modules: &[ParsedModule],
) -> Result<CompiledArtifact, ScriptLangError> {
    compile_modules(parsed_modules)
}

pub(crate) fn compile_modules(
    parsed_modules: &[ParsedModule],
) -> Result<CompiledArtifact, ScriptLangError> {
    let mut builder = ArtifactBuilder {
        scripts: Vec::new(),
        script_refs: BTreeMap::new(),
        globals: Vec::new(),
        default_entry_script_id: None,
    };

    builder.collect_declarations(parsed_modules)?;
    builder.lower_modules(parsed_modules)?;

    let default_entry_script_id = builder
        .default_entry_script_id
        .ok_or_else(|| ScriptLangError::message("no <script> declarations found"))?;

    let boot_script_id = builder.scripts.len();
    let boot_script = builder.build_boot_script(default_entry_script_id);
    let mut scripts = builder
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
        script_refs: builder.script_refs,
        scripts,
        globals: builder.globals,
    })
}

pub(crate) struct ArtifactBuilder {
    pub(crate) scripts: Vec<ScriptDraft>,
    pub(crate) script_refs: BTreeMap<String, ScriptId>,
    pub(crate) globals: Vec<GlobalVar>,
    pub(crate) default_entry_script_id: Option<ScriptId>,
}

#[derive(Clone)]
pub(crate) struct ScriptDraft {
    pub(crate) script_ref: String,
    pub(crate) module_name: String,
    pub(crate) local_names: Vec<String>,
    pub(crate) local_lookup: HashMap<String, LocalId>,
    pub(crate) instructions: Vec<Instruction>,
}

impl ArtifactBuilder {
    fn collect_declarations(&mut self, modules: &[ParsedModule]) -> Result<(), ScriptLangError> {
        let mut global_short_names = HashMap::<String, String>::new();

        for module in modules {
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
                    module_name: module.name.clone(),
                    local_names: Vec::new(),
                    local_lookup: HashMap::new(),
                    instructions: Vec::new(),
                });
            }
        }

        Ok(())
    }

    fn lower_modules(&mut self, modules: &[ParsedModule]) -> Result<(), ScriptLangError> {
        let mut script_index = 0;
        for module in modules {
            for script in &module.scripts {
                lower_script(self, script_index, &module.name, script)?;
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
    use sl_core::{ParsedScript, ParsedStmt, ParsedVar, TextSegment, TextTemplate};

    use super::{ArtifactBuilder, Instruction, compile_artifact, compile_modules};

    fn module(
        name: &str,
        vars: Vec<(&str, &str)>,
        scripts: Vec<(&str, Vec<ParsedStmt>)>,
    ) -> sl_core::ParsedModule {
        sl_core::ParsedModule {
            name: name.to_string(),
            vars: vars
                .into_iter()
                .map(|(name, expr)| ParsedVar {
                    name: name.to_string(),
                    expr: expr.to_string(),
                })
                .collect(),
            scripts: scripts
                .into_iter()
                .map(|(name, body)| ParsedScript {
                    name: name.to_string(),
                    body,
                })
                .collect(),
        }
    }

    fn text_template() -> TextTemplate {
        TextTemplate {
            segments: vec![
                TextSegment::Literal("hello ".to_string()),
                TextSegment::Expr("name".to_string()),
            ],
        }
    }

    #[test]
    fn compile_artifact_requires_at_least_one_script() {
        let error = compile_artifact(&[module("main", vec![], vec![])]).expect_err("should fail");

        assert_eq!(error.to_string(), "no <script> declarations found");
    }

    #[test]
    fn collect_declarations_rejects_ambiguous_global_short_names() {
        let error = compile_modules(&[
            module(
                "a",
                vec![("name", "1")],
                vec![("entry", vec![ParsedStmt::End])],
            ),
            module(
                "b",
                vec![("name", "2")],
                vec![("entry", vec![ParsedStmt::End])],
            ),
        ])
        .expect_err("should fail");

        assert_eq!(
            error.to_string(),
            "global short name `name` is ambiguous between `a.name` and `b.name`"
        );
    }

    #[test]
    fn collect_declarations_rejects_duplicate_script_refs() {
        let error = compile_modules(&[module(
            "main",
            vec![],
            vec![
                ("entry", vec![ParsedStmt::End]),
                ("entry", vec![ParsedStmt::End]),
            ],
        )])
        .expect_err("should fail");

        assert_eq!(
            error.to_string(),
            "duplicate script declaration `main.entry`"
        );
    }

    #[test]
    fn compile_artifact_sets_default_entry_and_boot_script_order() {
        let artifact = compile_artifact(&[module(
            "main",
            vec![("answer", "40 + 2")],
            vec![
                ("entry", vec![ParsedStmt::End]),
                ("other", vec![ParsedStmt::End]),
            ],
        )])
        .expect("artifact should compile");

        assert_eq!(artifact.default_entry_script_id, 0);
        assert_eq!(artifact.boot_script_id, 2);
        assert_eq!(artifact.script_refs["main.entry"], 0);
        assert_eq!(artifact.script_refs["main.other"], 1);
        assert_eq!(artifact.scripts[2].script_ref, "__boot__");
        assert!(matches!(
            &artifact.scripts[2].instructions[..],
            [
                Instruction::EvalGlobalInit { global_id, expr },
                Instruction::JumpScript { target_script_id }
            ] if *global_id == 0 && expr == "40 + 2" && *target_script_id == 0
        ));
    }

    #[test]
    fn compile_artifact_lowers_statements_into_expected_instructions() {
        let artifact = compile_artifact(&[
            module(
                "main",
                vec![("shared", "10")],
                vec![
                    (
                        "entry",
                        vec![
                            ParsedStmt::Temp {
                                name: "x".to_string(),
                                expr: "1".to_string(),
                            },
                            ParsedStmt::Temp {
                                name: "x".to_string(),
                                expr: "2".to_string(),
                            },
                            ParsedStmt::Code {
                                code: "x += 1;".to_string(),
                            },
                            ParsedStmt::Text {
                                template: text_template(),
                                tag: Some("line".to_string()),
                            },
                            ParsedStmt::If {
                                when: "x > 0".to_string(),
                                body: vec![ParsedStmt::Text {
                                    template: TextTemplate {
                                        segments: vec![TextSegment::Literal("inside".to_string())],
                                    },
                                    tag: None,
                                }],
                            },
                            ParsedStmt::Choice {
                                prompt: Some(TextTemplate {
                                    segments: vec![TextSegment::Literal("pick".to_string())],
                                }),
                                options: vec![
                                    sl_core::ParsedChoiceOption {
                                        text: TextTemplate {
                                            segments: vec![TextSegment::Literal(
                                                "left".to_string(),
                                            )],
                                        },
                                        body: vec![ParsedStmt::Goto {
                                            target_script_ref: "target".to_string(),
                                        }],
                                    },
                                    sl_core::ParsedChoiceOption {
                                        text: TextTemplate {
                                            segments: vec![TextSegment::Literal(
                                                "right".to_string(),
                                            )],
                                        },
                                        body: vec![ParsedStmt::End],
                                    },
                                ],
                            },
                        ],
                    ),
                    ("target", vec![ParsedStmt::End]),
                    (
                        "jump_end",
                        vec![ParsedStmt::Goto {
                            target_script_ref: "main.target".to_string(),
                        }],
                    ),
                ],
            ),
            module("other", vec![], vec![("remote", vec![ParsedStmt::End])]),
        ])
        .expect("artifact should compile");

        let instructions = &artifact.scripts[0].instructions;
        assert!(matches!(
            &instructions[0],
            Instruction::EvalTemp { local_id, expr } if *local_id == 0 && expr == "1"
        ));
        assert!(matches!(
            &instructions[1],
            Instruction::EvalTemp { local_id, expr } if *local_id == 0 && expr == "2"
        ));
        assert!(matches!(
            &instructions[2],
            Instruction::ExecCode { code } if code == "x += 1;"
        ));
        assert!(matches!(
            &instructions[3],
            Instruction::EmitText { tag, .. } if tag.as_deref() == Some("line")
        ));
        assert!(matches!(
            &instructions[4],
            Instruction::EvalCond { expr } if expr == "x > 0"
        ));
        assert!(matches!(
            &instructions[5],
            Instruction::JumpIfFalse { target_pc } if *target_pc > 5
        ));
        assert!(matches!(&instructions[6], Instruction::EmitText { .. }));
        assert!(matches!(
            &instructions[7],
            Instruction::BuildChoice { prompt, options }
                if prompt.is_some() && options.len() == 2
        ));
        assert!(matches!(
            &instructions[8],
            Instruction::JumpScript { target_script_id } if *target_script_id == artifact.script_refs["main.target"]
        ));
        assert!(matches!(
            &instructions[9],
            Instruction::Jump { target_pc } if *target_pc == 12
        ));
        assert!(matches!(&instructions[10], Instruction::End));
        assert!(matches!(
            &instructions[11],
            Instruction::Jump { target_pc } if *target_pc == 12
        ));
        assert!(matches!(&instructions[12], Instruction::End));
        assert_eq!(artifact.scripts[0].local_names, vec!["x".to_string()]);
        assert!(matches!(
            artifact.scripts[2].instructions.as_slice(),
            [Instruction::JumpScript { .. }]
        ));
        assert!(matches!(
            artifact.scripts[1].instructions.as_slice(),
            [Instruction::End]
        ));
    }

    #[test]
    fn compile_artifact_resolves_goto_variants_and_rejects_unknown_targets() {
        let artifact = compile_artifact(&[
            module(
                "main",
                vec![],
                vec![
                    (
                        "entry",
                        vec![
                            ParsedStmt::Goto {
                                target_script_ref: "next".to_string(),
                            },
                            ParsedStmt::Goto {
                                target_script_ref: "@next".to_string(),
                            },
                            ParsedStmt::Goto {
                                target_script_ref: "other.remote".to_string(),
                            },
                        ],
                    ),
                    ("next", vec![ParsedStmt::End]),
                ],
            ),
            module("other", vec![], vec![("remote", vec![ParsedStmt::End])]),
        ])
        .expect("artifact should compile");
        let target_next = artifact.script_refs["main.next"];
        let target_remote = artifact.script_refs["other.remote"];

        assert!(matches!(
            &artifact.scripts[0].instructions[..],
            [
                Instruction::JumpScript { target_script_id: first },
                Instruction::JumpScript { target_script_id: second },
                Instruction::JumpScript { target_script_id: third },
            ] if *first == target_next && *second == target_next && *third == target_remote
        ));

        let error = compile_artifact(&[module(
            "main",
            vec![],
            vec![(
                "entry",
                vec![ParsedStmt::Goto {
                    target_script_ref: "missing".to_string(),
                }],
            )],
        )])
        .expect_err("should fail");

        assert_eq!(
            error.to_string(),
            "script `main.missing` referenced by <goto> does not exist"
        );
    }

    #[test]
    fn build_boot_script_is_empty_except_for_jump_when_no_globals_exist() {
        let builder = ArtifactBuilder {
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
