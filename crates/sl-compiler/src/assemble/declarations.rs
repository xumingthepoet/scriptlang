use std::collections::BTreeSet;

use sl_core::ScriptLangError;

use crate::names::{qualified_member_name, runtime_global_name};
use crate::semantic::SemanticModule;

use super::{
    ProgramAssembler,
    lowering::compile_expr,
    types::{GlobalDecl, ScriptDraft},
};

impl ProgramAssembler {
    pub(super) fn collect_declarations(
        &mut self,
        modules: &[SemanticModule],
    ) -> Result<(), ScriptLangError> {
        let global_names = modules
            .iter()
            .flat_map(|module| {
                module
                    .vars
                    .iter()
                    .map(|var| runtime_global_name(&qualified_member_name(&module.name, &var.name)))
            })
            .collect::<BTreeSet<_>>();
        for module in modules {
            for var in &module.vars {
                let qualified_name = qualified_member_name(&module.name, &var.name);
                self.globals.push(GlobalDecl {
                    global: sl_core::GlobalVar {
                        global_id: self.globals.len(),
                        runtime_name: runtime_global_name(&qualified_name),
                    },
                    initializer: var.expr.clone(),
                });
            }

            for function in &module.functions {
                let function_ref = qualified_member_name(&module.name, &function.name);
                if self.functions.contains_key(&function_ref) {
                    return Err(ScriptLangError::message(format!(
                        "duplicate function declaration `{function_ref}`"
                    )));
                }
                self.functions.insert(
                    function_ref,
                    sl_core::CompiledFunction {
                        param_names: function.param_names.clone(),
                        body: compile_expr(&function.body, &function.param_names, &global_names),
                    },
                );
            }

            for script in &module.scripts {
                let script_ref = qualified_member_name(&module.name, &script.name);
                if self.script_refs.contains_key(&script_ref) {
                    return Err(ScriptLangError::message(format!(
                        "duplicate script declaration `{script_ref}`"
                    )));
                }
                let script_id = self.scripts.len();
                self.script_refs.insert(script_ref.clone(), script_id);
                self.scripts.push(ScriptDraft {
                    local_names: Vec::new(),
                    local_lookup: std::collections::HashMap::new(),
                    instructions: Vec::new(),
                });
            }
        }

        Ok(())
    }
}
