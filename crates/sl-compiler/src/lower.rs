use sl_core::{ChoiceBranch, Instruction, ScriptId, ScriptLangError, XmlForm};

use crate::builder::{ArtifactBuilder, ScriptDraft};
use crate::text::{lower_text_template, parse_text_template};
use crate::xml::{attr, child_elements, error_at, required_attr, trimmed_text_content};

pub(crate) fn lower_script(
    builder: &mut ArtifactBuilder,
    script_id: ScriptId,
    module_name: &str,
    script: &XmlForm,
) -> Result<(), ScriptLangError> {
    let mut draft = builder.scripts[script_id].clone();
    draft.module_name = module_name.to_string();
    lower_block(builder, &mut draft, &child_elements(script)?, module_name)?;
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
    body: &[&XmlForm],
    module_name: &str,
) -> Result<(), ScriptLangError> {
    for stmt in body {
        match stmt.tag.as_str() {
            "temp" => {
                let name = required_attr(stmt, "name")?;
                let expr = trimmed_text_content(stmt)?;
                let local_id = if let Some(local_id) = draft.local_lookup.get(name) {
                    *local_id
                } else {
                    let local_id = draft.local_names.len();
                    draft.local_names.push(name.to_string());
                    draft.local_lookup.insert(name.to_string(), local_id);
                    local_id
                };
                draft
                    .instructions
                    .push(Instruction::EvalTemp { local_id, expr });
            }
            "code" => {
                draft.instructions.push(Instruction::ExecCode {
                    code: trimmed_text_content(stmt)?,
                });
            }
            "text" => {
                draft.instructions.push(Instruction::EmitText {
                    text: lower_text_template(&parse_text_template(&trimmed_text_content(stmt)?)),
                    tag: attr(stmt, "tag").map(str::to_string),
                });
            }
            "if" => {
                draft.instructions.push(Instruction::EvalCond {
                    expr: required_attr(stmt, "when")?.to_string(),
                });
                let jump_index = draft.instructions.len();
                draft
                    .instructions
                    .push(Instruction::JumpIfFalse { target_pc: 0 });
                let body = child_elements(stmt)?;
                lower_block(builder, draft, &body, module_name)?;
                let after_body = draft.instructions.len();
                draft.instructions[jump_index] = Instruction::JumpIfFalse {
                    target_pc: after_body,
                };
            }
            "choice" => {
                let prompt = attr(stmt, "text").map(parse_text_template);
                let build_index = draft.instructions.len();
                draft.instructions.push(Instruction::BuildChoice {
                    prompt: prompt.as_ref().map(lower_text_template),
                    options: Vec::new(),
                });
                let mut branches = Vec::new();
                let mut branch_jump_indices = Vec::new();
                for option in child_elements(stmt)? {
                    if option.tag != "option" {
                        return Err(error_at(
                            option,
                            format!(
                                "<choice> only supports <option> children in MVP, got <{}>",
                                option.tag
                            ),
                        ));
                    }
                    let target_pc = draft.instructions.len();
                    branches.push(ChoiceBranch {
                        text: lower_text_template(&parse_text_template(required_attr(
                            option, "text",
                        )?)),
                        target_pc,
                    });
                    let body = child_elements(option)?;
                    lower_block(builder, draft, &body, module_name)?;
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
            "goto" => {
                let resolved = resolve_script_ref(module_name, required_attr(stmt, "script")?);
                let target_script_id =
                    builder.script_refs.get(&resolved).copied().ok_or_else(|| {
                        error_at(
                            stmt,
                            format!("script `{resolved}` referenced by <goto> does not exist"),
                        )
                    })?;
                draft
                    .instructions
                    .push(Instruction::JumpScript { target_script_id });
            }
            "end" => draft.instructions.push(Instruction::End),
            other => {
                return Err(error_at(
                    stmt,
                    format!("unsupported statement <{other}> in MVP"),
                ));
            }
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
