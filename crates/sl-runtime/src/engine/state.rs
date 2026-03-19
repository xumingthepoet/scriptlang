use rhai::Dynamic;
use sl_core::{CompiledArtifact, PendingChoiceOption, ScriptId};

#[derive(Clone, Debug)]
pub(crate) struct EngineState {
    pub(crate) script_id: ScriptId,
    pub(crate) pc: usize,
    pub(crate) globals: Vec<Dynamic>,
    pub(crate) locals: Vec<Dynamic>,
    pub(crate) pending: Option<PendingChoiceState>,
    pub(crate) current_condition: Option<bool>,
    pub(crate) started: bool,
    pub(crate) halted: bool,
    pub(crate) entry_override: Option<ScriptId>,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingChoiceState {
    pub(crate) prompt: Option<String>,
    pub(crate) options: Vec<PendingChoiceOption>,
}

impl EngineState {
    pub(crate) fn for_boot(artifact: &CompiledArtifact) -> Self {
        Self {
            script_id: artifact.boot_script_id,
            pc: 0,
            globals: vec![Dynamic::UNIT; artifact.globals.len()],
            locals: artifact.scripts[artifact.boot_script_id]
                .local_names
                .iter()
                .map(|_| Dynamic::UNIT)
                .collect(),
            pending: None,
            current_condition: None,
            started: false,
            halted: false,
            entry_override: None,
        }
    }

    pub(crate) fn started(artifact: &CompiledArtifact, entry_override: Option<ScriptId>) -> Self {
        Self {
            script_id: artifact.boot_script_id,
            pc: 0,
            globals: vec![Dynamic::UNIT; artifact.globals.len()],
            locals: artifact.scripts[artifact.boot_script_id]
                .local_names
                .iter()
                .map(|_| Dynamic::UNIT)
                .collect(),
            pending: None,
            current_condition: None,
            started: true,
            halted: false,
            entry_override,
        }
    }
}
