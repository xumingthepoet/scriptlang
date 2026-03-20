use sl_core::ScriptLangError;

use crate::semantic::SemanticModule;

use super::{ProgramAssembler, types::ScriptDraft};

impl ProgramAssembler {
    pub(super) fn collect_declarations(
        &mut self,
        modules: &[SemanticModule],
    ) -> Result<(), ScriptLangError> {
        let mut global_short_names = std::collections::HashMap::<String, String>::new();

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
                self.globals.push(sl_core::GlobalVar {
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
                    local_lookup: std::collections::HashMap::new(),
                    instructions: Vec::new(),
                });
            }
        }

        Ok(())
    }
}
