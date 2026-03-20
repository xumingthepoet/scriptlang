use sl_core::{
    ChoiceBranch, CompiledText, CompiledTextPart, Instruction, LocalId, ScriptId, ScriptLangError,
    TextSegment, TextTemplate,
};

use crate::assemble::ScriptDraft;
use crate::semantic::{SemanticChoiceOption, SemanticModule, SemanticScript, SemanticStmt};

use super::ProgramAssembler;

impl ProgramAssembler {
    pub(super) fn lower_modules(
        &mut self,
        modules: &[SemanticModule],
    ) -> Result<(), ScriptLangError> {
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
}

pub(crate) fn lower_script(
    script_refs: &std::collections::BTreeMap<String, ScriptId>,
    draft: &mut ScriptDraft,
    module_name: &str,
    script: &SemanticScript,
) -> Result<(), ScriptLangError> {
    lower_block(script_refs, draft, &script.body, module_name)?;
    if !matches!(
        draft.instructions.last(),
        Some(Instruction::End | Instruction::JumpScript { .. })
    ) {
        draft.instructions.push(Instruction::End);
    }
    Ok(())
}

fn lower_block(
    script_refs: &std::collections::BTreeMap<String, ScriptId>,
    draft: &mut ScriptDraft,
    body: &[SemanticStmt],
    module_name: &str,
) -> Result<(), ScriptLangError> {
    for stmt in body {
        match stmt {
            SemanticStmt::Temp { name, expr } => {
                let local_id = assign_local_id(draft, name);
                draft.instructions.push(Instruction::EvalTemp {
                    local_id,
                    expr: expr.clone(),
                });
            }
            SemanticStmt::Code { code } => {
                draft
                    .instructions
                    .push(Instruction::ExecCode { code: code.clone() });
            }
            SemanticStmt::Text { template, tag } => {
                draft.instructions.push(Instruction::EmitText {
                    text: lower_text_template(template),
                    tag: tag.clone(),
                });
            }
            SemanticStmt::If { when, body } => {
                draft
                    .instructions
                    .push(Instruction::EvalCond { expr: when.clone() });
                let jump_index = draft.instructions.len();
                draft
                    .instructions
                    .push(Instruction::JumpIfFalse { target_pc: 0 });
                lower_block(script_refs, draft, body, module_name)?;
                let after_body = draft.instructions.len();
                draft.instructions[jump_index] = Instruction::JumpIfFalse {
                    target_pc: after_body,
                };
            }
            SemanticStmt::Choice { prompt, options } => {
                lower_choice(script_refs, draft, module_name, prompt.as_ref(), options)?;
            }
            SemanticStmt::Goto { target_script_ref } => {
                let resolved = resolve_script_ref(module_name, target_script_ref);
                let target_script_id = script_refs.get(&resolved).copied().ok_or_else(|| {
                    ScriptLangError::message(format!(
                        "script `{resolved}` referenced by <goto> does not exist"
                    ))
                })?;
                draft
                    .instructions
                    .push(Instruction::JumpScript { target_script_id });
            }
            SemanticStmt::End => draft.instructions.push(Instruction::End),
        }
    }
    Ok(())
}

fn lower_choice(
    script_refs: &std::collections::BTreeMap<String, ScriptId>,
    draft: &mut ScriptDraft,
    module_name: &str,
    prompt: Option<&sl_core::TextTemplate>,
    options: &[SemanticChoiceOption],
) -> Result<(), ScriptLangError> {
    let build_index = draft.instructions.len();
    draft.instructions.push(Instruction::BuildChoice {
        prompt: prompt.map(lower_text_template),
        options: Vec::new(),
    });
    let mut branches = Vec::with_capacity(options.len());
    let mut branch_jump_indices = Vec::with_capacity(options.len());

    for option in options {
        let target_pc = draft.instructions.len();
        branches.push(ChoiceBranch {
            text: lower_text_template(&option.text),
            target_pc,
        });
        lower_block(script_refs, draft, &option.body, module_name)?;
        let jump_index = draft.instructions.len();
        draft.instructions.push(Instruction::Jump { target_pc: 0 });
        branch_jump_indices.push(jump_index);
    }

    let join_pc = draft.instructions.len();
    for jump_index in branch_jump_indices {
        draft.instructions[jump_index] = Instruction::Jump { target_pc: join_pc };
    }
    draft.instructions[build_index] = Instruction::BuildChoice {
        prompt: prompt.map(lower_text_template),
        options: branches,
    };
    Ok(())
}

fn lower_text_template(template: &TextTemplate) -> CompiledText {
    CompiledText {
        parts: template
            .segments
            .iter()
            .map(|segment| match segment {
                TextSegment::Literal(text) => CompiledTextPart::Literal(text.clone()),
                TextSegment::Expr(expr) => CompiledTextPart::Expr(expr.clone()),
            })
            .collect(),
    }
}

fn assign_local_id(draft: &mut ScriptDraft, name: &str) -> LocalId {
    if let Some(local_id) = draft.local_lookup.get(name) {
        *local_id
    } else {
        let local_id = draft.local_names.len();
        draft.local_names.push(name.to_string());
        draft.local_lookup.insert(name.to_string(), local_id);
        local_id
    }
}

fn resolve_script_ref(module_name: &str, raw: &str) -> String {
    let raw = raw.strip_prefix('@').unwrap_or(raw);
    if raw.contains('.') {
        raw.to_string()
    } else {
        format!("{module_name}.{raw}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use sl_core::{CompiledTextPart, Instruction, TextSegment, TextTemplate};

    use crate::assemble::ScriptDraft;
    use crate::semantic::{SemanticChoiceOption, SemanticScript, SemanticStmt};

    use super::{lower_script, lower_text_template};

    fn draft() -> ScriptDraft {
        ScriptDraft {
            script_ref: "main.entry".to_string(),
            local_names: Vec::new(),
            local_lookup: HashMap::new(),
            instructions: Vec::new(),
        }
    }

    #[test]
    fn lower_script_emits_expected_instructions_from_semantic_script() {
        let mut draft = draft();
        let script_refs = BTreeMap::from([("main.target".to_string(), 1usize)]);
        let script = SemanticScript {
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
                    options: vec![SemanticChoiceOption {
                        text: TextTemplate {
                            segments: vec![TextSegment::Literal("left".to_string())],
                        },
                        body: vec![SemanticStmt::Goto {
                            target_script_ref: "target".to_string(),
                        }],
                    }],
                },
            ],
        };

        lower_script(&script_refs, &mut draft, "main", &script).expect("lower");

        assert!(matches!(
            &draft.instructions[0],
            Instruction::EvalTemp { local_id, expr } if *local_id == 0 && expr == "1"
        ));
        assert!(matches!(
            &draft.instructions[1],
            Instruction::BuildChoice { prompt, options }
                if prompt.is_some() && options.len() == 1
        ));
        assert!(matches!(
            &draft.instructions[2],
            Instruction::JumpScript { target_script_id } if *target_script_id == 1
        ));
    }

    #[test]
    fn lower_script_rejects_missing_goto_target() {
        let mut draft = draft();
        let error = lower_script(
            &BTreeMap::new(),
            &mut draft,
            "main",
            &SemanticScript {
                name: "entry".to_string(),
                body: vec![SemanticStmt::Goto {
                    target_script_ref: "missing".to_string(),
                }],
            },
        )
        .expect_err("missing target should fail");

        assert!(
            error
                .to_string()
                .contains("script `main.missing` referenced by <goto> does not exist")
        );
    }

    #[test]
    fn lower_text_template_preserves_literal_and_expression_segments() {
        let lowered = lower_text_template(&TextTemplate {
            segments: vec![
                TextSegment::Literal("hello ".to_string()),
                TextSegment::Expr("name".to_string()),
            ],
        });

        assert!(matches!(
            lowered.parts.as_slice(),
            [CompiledTextPart::Literal(text), CompiledTextPart::Expr(expr)]
                if text == "hello " && expr == "name"
        ));
    }
}
