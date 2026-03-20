use sl_core::{Instruction, ScriptId};

use crate::names::lower_resolved_vars_to_runtime_names;

use super::ProgramAssembler;

impl ProgramAssembler {
    pub(super) fn build_boot_script(&self, default_entry_script_id: ScriptId) -> Vec<Instruction> {
        let mut instructions = Vec::with_capacity(self.globals.len() + 2);
        for global in &self.globals {
            instructions.push(Instruction::EvalGlobalInit {
                global_id: global.global.global_id,
                expr: lower_resolved_vars_to_runtime_names(&global.initializer),
            });
        }
        instructions.push(Instruction::JumpScript {
            target_script_id: default_entry_script_id,
        });
        instructions
    }
}
