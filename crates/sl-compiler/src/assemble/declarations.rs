use sl_core::ScriptLangError;

use crate::semantic::SemanticModule;
use crate::semantic::types::runtime_global_name;

use super::{ProgramAssembler, types::ScriptDraft};

const DEFAULT_ENTRY_SCRIPT_REF: &str = "main.main";

impl ProgramAssembler {
    pub(super) fn collect_declarations(
        &mut self,
        modules: &[SemanticModule],
    ) -> Result<(), ScriptLangError> {
        for module in modules {
            for var in &module.vars {
                let qualified_name = format!("{}.{}", module.name, var.name);
                self.globals.push(sl_core::GlobalVar {
                    global_id: self.globals.len(),
                    runtime_name: global_runtime_name(&qualified_name),
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
                if script_ref == DEFAULT_ENTRY_SCRIPT_REF {
                    self.default_entry_script_id = Some(script_id);
                }
                self.scripts.push(ScriptDraft {
                    script_ref,
                    local_names: Vec::new(),
                    local_lookup: std::collections::HashMap::new(),
                    instructions: Vec::new(),
                });
            }
        }

        Ok(())
    }
}

fn global_runtime_name(qualified_name: &str) -> String {
    runtime_global_name(qualified_name)
}
