use sl_core::{Completion, Instruction, ScriptLangError, StepEvent, StepResult, Suspension};

use super::{Engine, PendingChoiceOption, PendingChoiceState, dynamic_to_bool};

impl Engine {
    pub fn step(&mut self) -> Result<StepResult, ScriptLangError> {
        if !self.state.started {
            self.start(None)?;
        }
        if self.state.halted {
            return Ok(StepResult::Completed(Completion::End));
        }
        if let Some(pending) = &self.state.pending {
            return Ok(StepResult::Suspended(Suspension::Choice {
                prompt: pending.prompt.clone(),
                items: pending
                    .options
                    .iter()
                    .map(|item| item.text.clone())
                    .collect(),
            }));
        }

        let script = self.current_script();
        if self.state.pc >= script.instructions.len() {
            self.state.halted = true;
            return Ok(StepResult::Completed(Completion::End));
        }

        let instruction = script.instructions[self.state.pc].clone();
        let current_script_id = self.state.script_id;
        let current_pc = self.state.pc;

        match instruction {
            Instruction::EvalGlobalInit { global_id, expr } => {
                let value = self.eval_expression(&expr)?;
                self.state.globals[global_id] = value;
                self.state.pc += 1;
                Ok(StepResult::Progress)
            }
            Instruction::EvalTemp { local_id, expr } => {
                let value = self.eval_expression(&expr)?;
                self.state.locals[local_id] = value;
                self.state.pc += 1;
                Ok(StepResult::Progress)
            }
            Instruction::EvalCond { expr } => {
                let value = self.eval_expression(&expr)?;
                self.state.current_condition = Some(dynamic_to_bool(&value)?);
                self.state.pc += 1;
                Ok(StepResult::Progress)
            }
            Instruction::ExecCode { code } => {
                self.exec_code(&code)?;
                self.state.pc += 1;
                Ok(StepResult::Progress)
            }
            Instruction::EmitText { text, tag } => {
                let text = self.render_text(&text)?;
                self.state.pc += 1;
                Ok(StepResult::Event(StepEvent::Text { text, tag }))
            }
            Instruction::BuildChoice { prompt, options } => {
                let rendered_prompt = match prompt {
                    Some(text) => Some(self.render_text(&text)?),
                    None => None,
                };
                let rendered_options = options
                    .into_iter()
                    .map(|option| {
                        Ok(PendingChoiceOption {
                            text: self.render_text(&option.text)?,
                            target_pc: option.target_pc,
                        })
                    })
                    .collect::<Result<Vec<_>, ScriptLangError>>()?;
                self.state.pending = Some(PendingChoiceState {
                    prompt: rendered_prompt.clone(),
                    options: rendered_options.clone(),
                });
                self.state.pc += 1;
                Ok(StepResult::Suspended(Suspension::Choice {
                    prompt: rendered_prompt,
                    items: rendered_options.into_iter().map(|item| item.text).collect(),
                }))
            }
            Instruction::JumpIfFalse { target_pc } => {
                let condition =
                    self.state.current_condition.take().ok_or_else(|| {
                        ScriptLangError::message("missing condition for JumpIfFalse")
                    })?;
                self.state.pc = if condition {
                    self.state.pc + 1
                } else {
                    target_pc
                };
                Ok(StepResult::Progress)
            }
            Instruction::Jump { target_pc } => {
                self.state.pc = target_pc;
                Ok(StepResult::Progress)
            }
            Instruction::JumpScript { target_script_id } => {
                let next_script_id = if current_script_id == self.artifact.boot_script_id
                    && current_pc + 1 == self.current_script().instructions.len()
                {
                    self.state.entry_override.take().unwrap_or(target_script_id)
                } else {
                    target_script_id
                };
                self.jump_to_script(next_script_id);
                Ok(StepResult::Progress)
            }
            Instruction::JumpScriptExpr { expr } => {
                let script_key = self.eval_script_key(&expr)?;
                let next_script_id = self.resolve_script_id(&script_key)?;
                self.jump_to_script(next_script_id);
                Ok(StepResult::Progress)
            }
            Instruction::ReturnToHost => {
                self.state.halted = true;
                Ok(StepResult::Completed(Completion::ReturnToHost))
            }
            Instruction::End => {
                self.state.halted = true;
                Ok(StepResult::Completed(Completion::End))
            }
        }
    }
}
