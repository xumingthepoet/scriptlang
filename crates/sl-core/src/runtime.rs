use rhai::Dynamic;

use crate::ScriptId;

#[derive(Clone, Debug)]
pub enum StepResult {
    Progress,
    Event(StepEvent),
    Suspended(Suspension),
    Completed(Completion),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepEvent {
    Text { text: String, tag: Option<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Suspension {
    Choice {
        prompt: Option<String>,
        items: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Completion {
    End,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub script_id: ScriptId,
    pub pc: usize,
    pub globals: Vec<Dynamic>,
    pub locals: Vec<Dynamic>,
    pub pending: Option<PendingChoiceSnapshot>,
    pub current_condition: Option<bool>,
    pub started: bool,
    pub halted: bool,
    pub entry_override: Option<ScriptId>,
}

#[derive(Clone, Debug)]
pub struct PendingChoiceSnapshot {
    pub prompt: Option<String>,
    pub options: Vec<PendingChoiceOption>,
}

#[derive(Clone, Debug)]
pub struct PendingChoiceOption {
    pub text: String,
    pub target_pc: usize,
}
