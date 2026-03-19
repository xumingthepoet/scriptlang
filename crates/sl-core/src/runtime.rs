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

#[cfg(test)]
mod tests {
    use rhai::Dynamic;

    use super::{
        Completion, PendingChoiceOption, PendingChoiceSnapshot, Snapshot, StepEvent, StepResult,
        Suspension,
    };

    #[test]
    fn runtime_types_cover_all_public_variants() {
        let pending = PendingChoiceSnapshot {
            prompt: Some("pick".to_string()),
            options: vec![PendingChoiceOption {
                text: "a".to_string(),
                target_pc: 7,
            }],
        };
        let snapshot = Snapshot {
            script_id: 2,
            pc: 3,
            globals: vec![Dynamic::from(1_i64)],
            locals: vec![Dynamic::from("x")],
            pending: Some(pending.clone()),
            current_condition: Some(true),
            started: true,
            halted: false,
            entry_override: Some(9),
        };
        let event = StepResult::Event(StepEvent::Text {
            text: "hello".to_string(),
            tag: Some("line".to_string()),
        });
        let suspended = StepResult::Suspended(Suspension::Choice {
            prompt: Some("pick".to_string()),
            items: vec!["a".to_string(), "b".to_string()],
        });
        let completed = StepResult::Completed(Completion::End);

        assert!(matches!(StepResult::Progress, StepResult::Progress));
        assert!(matches!(
            event,
            StepResult::Event(StepEvent::Text { text, tag })
                if text == "hello" && tag.as_deref() == Some("line")
        ));
        assert!(matches!(
            suspended,
            StepResult::Suspended(Suspension::Choice { prompt, items })
                if prompt.as_deref() == Some("pick") && items == vec!["a".to_string(), "b".to_string()]
        ));
        assert!(matches!(completed, StepResult::Completed(Completion::End)));

        assert_eq!(snapshot.script_id, 2);
        assert_eq!(snapshot.pc, 3);
        assert_eq!(snapshot.globals[0].clone_cast::<i64>(), 1);
        assert_eq!(snapshot.locals[0].clone_cast::<String>(), "x");
        assert_eq!(snapshot.current_condition, Some(true));
        assert!(snapshot.started);
        assert!(!snapshot.halted);
        assert_eq!(snapshot.entry_override, Some(9));
        assert_eq!(
            snapshot.pending.expect("pending should exist").options[0].target_pc,
            pending.options[0].target_pc
        );
    }
}
