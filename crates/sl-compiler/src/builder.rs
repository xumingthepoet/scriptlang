use std::collections::{BTreeMap, HashMap};

use sl_core::{
    CompiledArtifact, CompiledScript, GlobalVar, Instruction, LocalId, ParsedModule, ScriptId,
    ScriptLangError,
};

use crate::lower::lower_script;

pub fn compile_artifact(
    parsed_modules: &[ParsedModule],
) -> Result<CompiledArtifact, ScriptLangError> {
    compile_modules(parsed_modules)
}

pub(crate) fn compile_modules(
    parsed_modules: &[ParsedModule],
) -> Result<CompiledArtifact, ScriptLangError> {
    let mut builder = ArtifactBuilder {
        scripts: Vec::new(),
        script_refs: BTreeMap::new(),
        globals: Vec::new(),
        default_entry_script_id: None,
    };

    builder.collect_declarations(parsed_modules)?;
    builder.lower_modules(parsed_modules)?;

    let default_entry_script_id = builder
        .default_entry_script_id
        .ok_or_else(|| ScriptLangError::message("no <script> declarations found"))?;

    let boot_script_id = builder.scripts.len();
    let boot_script = builder.build_boot_script(default_entry_script_id);
    let mut scripts = builder
        .scripts
        .into_iter()
        .enumerate()
        .map(|(script_id, draft)| CompiledScript {
            script_id,
            script_ref: draft.script_ref,
            local_names: draft.local_names,
            instructions: draft.instructions,
        })
        .collect::<Vec<_>>();
    scripts.push(CompiledScript {
        script_id: boot_script_id,
        script_ref: "__boot__".to_string(),
        local_names: Vec::new(),
        instructions: boot_script,
    });

    Ok(CompiledArtifact {
        default_entry_script_id,
        boot_script_id,
        script_refs: builder.script_refs,
        scripts,
        globals: builder.globals,
    })
}

pub(crate) struct ArtifactBuilder {
    pub(crate) scripts: Vec<ScriptDraft>,
    pub(crate) script_refs: BTreeMap<String, ScriptId>,
    pub(crate) globals: Vec<GlobalVar>,
    pub(crate) default_entry_script_id: Option<ScriptId>,
}

#[derive(Clone)]
pub(crate) struct ScriptDraft {
    pub(crate) script_ref: String,
    pub(crate) module_name: String,
    pub(crate) local_names: Vec<String>,
    pub(crate) local_lookup: HashMap<String, LocalId>,
    pub(crate) instructions: Vec<Instruction>,
}

impl ArtifactBuilder {
    fn collect_declarations(&mut self, modules: &[ParsedModule]) -> Result<(), ScriptLangError> {
        let mut global_short_names = HashMap::<String, String>::new();

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
                self.globals.push(GlobalVar {
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
                    module_name: module.name.clone(),
                    local_names: Vec::new(),
                    local_lookup: HashMap::new(),
                    instructions: Vec::new(),
                });
            }
        }

        Ok(())
    }

    fn lower_modules(&mut self, modules: &[ParsedModule]) -> Result<(), ScriptLangError> {
        let mut script_index = 0;
        for module in modules {
            for script in &module.scripts {
                lower_script(self, script_index, &module.name, script)?;
                script_index += 1;
            }
        }
        Ok(())
    }

    fn build_boot_script(&self, default_entry_script_id: ScriptId) -> Vec<Instruction> {
        let mut instructions = Vec::with_capacity(self.globals.len() + 2);
        for global in &self.globals {
            instructions.push(Instruction::EvalGlobalInit {
                global_id: global.global_id,
                expr: global.initializer.clone(),
            });
        }
        instructions.push(Instruction::JumpScript {
            target_script_id: default_entry_script_id,
        });
        instructions
    }
}
