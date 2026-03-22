use sl_core::{
    ChoiceBranch, CompiledText, CompiledTextPart, Instruction, LocalId, ScriptId, ScriptLangError,
    TextSegment, TextTemplate,
};

use crate::assemble::ScriptDraft;
use crate::names::lower_resolved_vars_to_runtime_names;
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
                lower_script(&self.script_refs, &mut draft, script)?;
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
    script: &SemanticScript,
) -> Result<(), ScriptLangError> {
    let mut loop_stack = Vec::new();
    lower_block(script_refs, draft, &script.body, &mut loop_stack)?;
    if !matches!(
        draft.instructions.last(),
        Some(
            Instruction::End | Instruction::JumpScript { .. } | Instruction::JumpScriptExpr { .. }
        )
    ) {
        draft.instructions.push(Instruction::End);
    }
    Ok(())
}

fn lower_block(
    script_refs: &std::collections::BTreeMap<String, ScriptId>,
    draft: &mut ScriptDraft,
    body: &[SemanticStmt],
    loop_stack: &mut Vec<LoopFrame>,
) -> Result<(), ScriptLangError> {
    for stmt in body {
        match stmt {
            SemanticStmt::Temp {
                name,
                declared_type: _,
                expr,
            } => {
                let local_id = assign_local_id(draft, name);
                draft.instructions.push(Instruction::EvalTemp {
                    local_id,
                    expr: lower_resolved_vars_to_runtime_names(expr),
                });
            }
            SemanticStmt::Code { code } => {
                draft.instructions.push(Instruction::ExecCode {
                    code: lower_resolved_vars_to_runtime_names(code),
                });
            }
            SemanticStmt::Text { template, tag } => {
                draft.instructions.push(Instruction::EmitText {
                    text: lower_text_template(template),
                    tag: tag.clone(),
                });
            }
            SemanticStmt::While {
                when,
                body,
                skip_loop_control_capture,
            } => {
                let head_pc = draft.instructions.len();
                draft.instructions.push(Instruction::EvalCond {
                    expr: lower_resolved_vars_to_runtime_names(when),
                });
                let exit_jump_index = draft.instructions.len();
                draft
                    .instructions
                    .push(Instruction::JumpIfFalse { target_pc: 0 });
                if !skip_loop_control_capture {
                    loop_stack.push(LoopFrame {
                        head_pc,
                        break_jump_indices: Vec::new(),
                    });
                }
                lower_block(script_refs, draft, body, loop_stack)?;
                draft
                    .instructions
                    .push(Instruction::Jump { target_pc: head_pc });
                let exit_pc = draft.instructions.len();
                draft.instructions[exit_jump_index] =
                    Instruction::JumpIfFalse { target_pc: exit_pc };
                if !skip_loop_control_capture {
                    let frame = loop_stack.pop().expect("loop frame should exist");
                    for jump_index in frame.break_jump_indices {
                        draft.instructions[jump_index] = Instruction::Jump { target_pc: exit_pc };
                    }
                }
            }
            SemanticStmt::Break => {
                let Some(frame) = loop_stack.last_mut() else {
                    return Err(ScriptLangError::message(
                        "<break> is only allowed inside <while>",
                    ));
                };
                let jump_index = draft.instructions.len();
                draft.instructions.push(Instruction::Jump { target_pc: 0 });
                frame.break_jump_indices.push(jump_index);
            }
            SemanticStmt::Continue => {
                let Some(frame) = loop_stack.last() else {
                    return Err(ScriptLangError::message(
                        "<continue> is only allowed inside <while>",
                    ));
                };
                draft.instructions.push(Instruction::Jump {
                    target_pc: frame.head_pc,
                });
            }
            SemanticStmt::Choice { prompt, options } => {
                lower_choice(script_refs, draft, prompt.as_ref(), options, loop_stack)?;
            }
            SemanticStmt::Goto { expr } => {
                draft.instructions.push(Instruction::JumpScriptExpr {
                    expr: lower_resolved_vars_to_runtime_names(expr),
                });
            }
            SemanticStmt::End => draft.instructions.push(Instruction::End),
        }
    }
    Ok(())
}

fn lower_choice(
    script_refs: &std::collections::BTreeMap<String, ScriptId>,
    draft: &mut ScriptDraft,
    prompt: Option<&sl_core::TextTemplate>,
    options: &[SemanticChoiceOption],
    loop_stack: &mut Vec<LoopFrame>,
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
        lower_block(script_refs, draft, &option.body, loop_stack)?;
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

struct LoopFrame {
    head_pc: usize,
    break_jump_indices: Vec<usize>,
}

fn lower_text_template(template: &TextTemplate) -> CompiledText {
    CompiledText {
        parts: template
            .segments
            .iter()
            .map(|segment| match segment {
                TextSegment::Literal(text) => CompiledTextPart::Literal(text.clone()),
                TextSegment::Expr(expr) => {
                    CompiledTextPart::Expr(lower_resolved_vars_to_runtime_names(expr))
                }
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use sl_core::{CompiledTextPart, Instruction, TextSegment, TextTemplate};

    use crate::assemble::ScriptDraft;
    use crate::semantic::types::DeclaredType;
    use crate::semantic::{SemanticChoiceOption, SemanticScript, SemanticStmt};

    use super::{lower_script, lower_text_template};

    fn draft() -> ScriptDraft {
        ScriptDraft {
            local_names: Vec::new(),
            local_lookup: HashMap::new(),
            instructions: Vec::new(),
        }
    }

    #[test]
    fn lower_script_emits_expected_instructions_from_semantic_script() {
        let mut draft = draft();
        let script = SemanticScript {
            name: "entry".to_string(),
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
                    options: vec![SemanticChoiceOption {
                        text: TextTemplate {
                            segments: vec![TextSegment::Literal("left".to_string())],
                        },
                        body: vec![SemanticStmt::Goto {
                            expr: "\"main.target\"".to_string(),
                        }],
                    }],
                },
            ],
        };

        lower_script(&BTreeMap::new(), &mut draft, &script).expect("lower");

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
            Instruction::JumpScriptExpr { expr } if expr == "\"main.target\""
        ));
    }

    #[test]
    fn lower_script_rewrites_runtime_globals_inside_goto_expression() {
        let mut draft = draft();
        let script = SemanticScript {
            name: "entry".to_string(),
            body: vec![SemanticStmt::Goto {
                expr: "__sl_var__(main.next)".to_string(),
            }],
        };

        lower_script(&BTreeMap::new(), &mut draft, &script).expect("lower");

        assert!(matches!(
            &draft.instructions[0],
            Instruction::JumpScriptExpr { expr } if expr == "__sl_globalmain_next"
        ));
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

    #[test]
    fn lower_script_covers_code_text_if_end_and_reused_local_ids() {
        let mut draft = draft();
        let script = SemanticScript {
            name: "entry".to_string(),
            body: vec![
                SemanticStmt::Temp {
                    name: "x".to_string(),
                    declared_type: DeclaredType::Int,
                    expr: "1".to_string(),
                },
                SemanticStmt::Code {
                    code: "x = x + 1;".to_string(),
                },
                SemanticStmt::Text {
                    template: TextTemplate {
                        segments: vec![
                            TextSegment::Literal("value=".to_string()),
                            TextSegment::Expr("x".to_string()),
                        ],
                    },
                    tag: Some("note".to_string()),
                },
                SemanticStmt::While {
                    when: "x > 1".to_string(),
                    skip_loop_control_capture: true,
                    body: vec![
                        SemanticStmt::Temp {
                            name: "x".to_string(),
                            declared_type: DeclaredType::Int,
                            expr: "2".to_string(),
                        },
                        SemanticStmt::End,
                    ],
                },
            ],
        };

        lower_script(&BTreeMap::new(), &mut draft, &script).expect("lower");

        assert_eq!(draft.local_names, vec!["x".to_string()]);
        assert!(matches!(
            &draft.instructions[0],
            Instruction::EvalTemp { local_id, expr } if *local_id == 0 && expr == "1"
        ));
        assert!(matches!(
            &draft.instructions[1],
            Instruction::ExecCode { code } if code == "x = x + 1;"
        ));
        assert!(matches!(
            &draft.instructions[2],
            Instruction::EmitText { text, tag }
                if matches!(text.parts.as_slice(),
                    [CompiledTextPart::Literal(prefix), CompiledTextPart::Expr(expr)]
                    if prefix == "value=" && expr == "x")
                && tag.as_deref() == Some("note")
        ));
        assert!(matches!(
            &draft.instructions[3],
            Instruction::EvalCond { expr } if expr == "x > 1"
        ));
        assert!(matches!(
            &draft.instructions[4],
            Instruction::JumpIfFalse { target_pc } if *target_pc == 8
        ));
        assert!(matches!(
            &draft.instructions[5],
            Instruction::EvalTemp { local_id, expr } if *local_id == 0 && expr == "2"
        ));
        assert!(matches!(&draft.instructions[6], Instruction::End));
        assert!(matches!(
            &draft.instructions[7],
            Instruction::Jump { target_pc } if *target_pc == 3
        ));
    }

    #[test]
    fn lower_script_covers_while_break_and_continue() {
        let mut draft = draft();
        let script = SemanticScript {
            name: "entry".to_string(),
            body: vec![SemanticStmt::While {
                when: "flag".to_string(),
                skip_loop_control_capture: false,
                body: vec![
                    SemanticStmt::Continue,
                    SemanticStmt::Break,
                    SemanticStmt::Text {
                        template: TextTemplate {
                            segments: vec![TextSegment::Literal("never".to_string())],
                        },
                        tag: None,
                    },
                ],
            }],
        };

        lower_script(&BTreeMap::new(), &mut draft, &script).expect("lower");

        assert!(matches!(
            &draft.instructions[0],
            Instruction::EvalCond { expr } if expr == "flag"
        ));
        assert!(matches!(
            &draft.instructions[1],
            Instruction::JumpIfFalse { target_pc } if *target_pc == 6
        ));
        assert!(matches!(
            &draft.instructions[2],
            Instruction::Jump { target_pc } if *target_pc == 0
        ));
        assert!(matches!(
            &draft.instructions[3],
            Instruction::Jump { target_pc } if *target_pc == 6
        ));
        assert!(matches!(
            &draft.instructions[4],
            Instruction::EmitText { .. }
        ));
        assert!(matches!(
            &draft.instructions[5],
            Instruction::Jump { target_pc } if *target_pc == 0
        ));
    }

    #[test]
    fn lower_script_rejects_break_and_continue_outside_loop() {
        let break_error = lower_script(
            &BTreeMap::new(),
            &mut draft(),
            &SemanticScript {
                name: "entry".to_string(),
                body: vec![SemanticStmt::Break],
            },
        )
        .expect_err("break outside loop");
        assert!(
            break_error
                .to_string()
                .contains("only allowed inside <while>")
        );

        let continue_error = lower_script(
            &BTreeMap::new(),
            &mut draft(),
            &SemanticScript {
                name: "entry".to_string(),
                body: vec![SemanticStmt::Continue],
            },
        )
        .expect_err("continue outside loop");
        assert!(
            continue_error
                .to_string()
                .contains("only allowed inside <while>")
        );
    }

    #[test]
    fn lower_script_non_capturing_while_does_not_accept_break_or_continue() {
        let break_error = lower_script(
            &BTreeMap::new(),
            &mut draft(),
            &SemanticScript {
                name: "entry".to_string(),
                body: vec![SemanticStmt::While {
                    when: "true".to_string(),
                    skip_loop_control_capture: true,
                    body: vec![SemanticStmt::Break],
                }],
            },
        )
        .expect_err("break should not target synthetic loop");
        assert!(
            break_error
                .to_string()
                .contains("only allowed inside <while>")
        );

        let continue_error = lower_script(
            &BTreeMap::new(),
            &mut draft(),
            &SemanticScript {
                name: "entry".to_string(),
                body: vec![SemanticStmt::While {
                    when: "true".to_string(),
                    skip_loop_control_capture: true,
                    body: vec![SemanticStmt::Continue],
                }],
            },
        )
        .expect_err("continue should not target synthetic loop");
        assert!(
            continue_error
                .to_string()
                .contains("only allowed inside <while>")
        );
    }
}
