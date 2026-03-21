use std::collections::{BTreeMap, HashMap};

use sl_core::{CompiledFunction, GlobalVar, Instruction, LocalId, ScriptId};

pub(crate) struct ProgramAssembler {
    pub(crate) functions: BTreeMap<String, CompiledFunction>,
    pub(crate) scripts: Vec<ScriptDraft>,
    pub(crate) script_refs: BTreeMap<String, ScriptId>,
    pub(crate) globals: Vec<GlobalDecl>,
    pub(crate) default_entry_script_id: Option<ScriptId>,
}

pub(crate) struct GlobalDecl {
    pub(crate) global: GlobalVar,
    pub(crate) initializer: String,
}

#[derive(Clone)]
pub(crate) struct ScriptDraft {
    pub(crate) local_names: Vec<String>,
    pub(crate) local_lookup: HashMap<String, LocalId>,
    pub(crate) instructions: Vec<Instruction>,
}
