use sl_core::{Instruction, ScriptId};

use std::collections::BTreeSet;

use super::{ProgramAssembler, lowering::compile_expr};

impl ProgramAssembler {
    pub(super) fn build_boot_script(&self, default_entry_script_id: ScriptId) -> Vec<Instruction> {
        let global_names = self
            .globals
            .iter()
            .map(|decl| decl.global.runtime_name.clone())
            .collect::<BTreeSet<_>>();
        let mut instructions = Vec::with_capacity(self.globals.len() + 2);
        for global in &self.globals {
            instructions.push(Instruction::EvalGlobalInit {
                global_id: global.global.global_id,
                expr: compile_expr(&global.initializer, &[], &global_names),
            });
        }
        instructions.push(Instruction::JumpScript {
            target_script_id: default_entry_script_id,
        });
        instructions
    }
}
