use std::collections::{BTreeMap, HashMap};

use sl_core::{GlobalVar, Instruction, LocalId, ScriptId};

pub(crate) struct ProgramAssembler {
    pub(crate) scripts: Vec<ScriptDraft>,
    pub(crate) script_refs: BTreeMap<String, ScriptId>,
    pub(crate) globals: Vec<GlobalVar>,
    pub(crate) default_entry_script_id: Option<ScriptId>,
}

#[derive(Clone)]
pub(crate) struct ScriptDraft {
    pub(crate) script_ref: String,
    pub(crate) local_names: Vec<String>,
    pub(crate) local_lookup: HashMap<String, LocalId>,
    pub(crate) instructions: Vec<Instruction>,
}
