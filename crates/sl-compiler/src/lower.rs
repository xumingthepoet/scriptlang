use sl_core::{ChoiceBranch, Instruction, ParsedScript, ParsedStmt, ScriptId, ScriptLangError};

use crate::builder::{ArtifactBuilder, ScriptDraft};
use crate::text::lower_text_template;

pub(crate) fn lower_script(
    builder: &mut ArtifactBuilder,
    script_id: ScriptId,
    module_name: &str,
    script: &ParsedScript,
) -> Result<(), ScriptLangError> {
    let mut draft = builder.scripts[script_id].clone();
    draft.module_name = module_name.to_string();
    lower_block(builder, &mut draft, &script.body, module_name)?;
    if !matches!(
        draft.instructions.last(),
        Some(Instruction::End | Instruction::JumpScript { .. })
    ) {
        draft.instructions.push(Instruction::End);
    }
    builder.scripts[script_id] = draft;
    Ok(())
}

fn lower_block(
    builder: &ArtifactBuilder,
    draft: &mut ScriptDraft,
    body: &[ParsedStmt],
    module_name: &str,
) -> Result<(), ScriptLangError> {
    for stmt in body {
        match stmt {
            ParsedStmt::Temp { name, expr } => {
                let local_id = if let Some(local_id) = draft.local_lookup.get(name) {
                    *local_id
                } else {
                    let local_id = draft.local_names.len();
                    draft.local_names.push(name.clone());
                    draft.local_lookup.insert(name.clone(), local_id);
                    local_id
                };
                draft.instructions.push(Instruction::EvalTemp {
                    local_id,
                    expr: expr.clone(),
                });
            }
            ParsedStmt::Code { code } => {
                draft
                    .instructions
                    .push(Instruction::ExecCode { code: code.clone() });
            }
            ParsedStmt::Text { template, tag } => {
                draft.instructions.push(Instruction::EmitText {
                    text: lower_text_template(template),
                    tag: tag.clone(),
                });
            }
            ParsedStmt::If { when, body } => {
                draft
                    .instructions
                    .push(Instruction::EvalCond { expr: when.clone() });
                let jump_index = draft.instructions.len();
                draft
                    .instructions
                    .push(Instruction::JumpIfFalse { target_pc: 0 });
                lower_block(builder, draft, body, module_name)?;
                let after_body = draft.instructions.len();
                draft.instructions[jump_index] = Instruction::JumpIfFalse {
                    target_pc: after_body,
                };
            }
            ParsedStmt::Choice { prompt, options } => {
                let build_index = draft.instructions.len();
                draft.instructions.push(Instruction::BuildChoice {
                    prompt: prompt.as_ref().map(lower_text_template),
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
                    lower_block(builder, draft, &option.body, module_name)?;
                    let jump_index = draft.instructions.len();
                    draft.instructions.push(Instruction::Jump { target_pc: 0 });
                    branch_jump_indices.push(jump_index);
                }
                let join_pc = draft.instructions.len();
                for jump_index in branch_jump_indices {
                    draft.instructions[jump_index] = Instruction::Jump { target_pc: join_pc };
                }
                draft.instructions[build_index] = Instruction::BuildChoice {
                    prompt: prompt.as_ref().map(lower_text_template),
                    options: branches,
                };
            }
            ParsedStmt::Goto { target_script_ref } => {
                let resolved = resolve_script_ref(module_name, target_script_ref);
                let target_script_id =
                    builder.script_refs.get(&resolved).copied().ok_or_else(|| {
                        ScriptLangError::message(format!(
                            "script `{resolved}` referenced by <goto> does not exist"
                        ))
                    })?;
                draft
                    .instructions
                    .push(Instruction::JumpScript { target_script_id });
            }
            ParsedStmt::End => draft.instructions.push(Instruction::End),
        }
    }
    Ok(())
}

fn resolve_script_ref(module_name: &str, raw: &str) -> String {
    let raw = raw.strip_prefix('@').unwrap_or(raw);
    if raw.contains('.') {
        raw.to_string()
    } else {
        format!("{module_name}.{raw}")
    }
}
