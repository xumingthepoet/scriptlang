//! Command layer: public types and command-related formatting utilities.
//!
//! These types are re-exported from the crate root (lib.rs facade).

use sl_core::StepEvent;

// ---------------------------------------------------------------------------
// Public types (re-exported from crate root)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectTarget {
    Ast,
    Semantic,
    Ir,
    Bindings,
    Modules,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadResult {
    pub modules: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmissionResult {
    ContextUpdated,
    ModuleUpdated { module_name: String },
    Executed(ExecutionResult),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionResult {
    pub events: Vec<StepEvent>,
    pub state: ExecutionState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionState {
    Ready,
    SuspendedChoice {
        prompt: Option<String>,
        items: Vec<String>,
    },
    Exited,
}

// ---------------------------------------------------------------------------
// Command parsing
// ---------------------------------------------------------------------------

pub fn split_command(input: &str) -> (&str, Option<&str>) {
    match input.split_once(char::is_whitespace) {
        Some((command, rest)) => (command, Some(rest.trim()).filter(|rest| !rest.is_empty())),
        None => (input, None),
    }
}

pub fn help_text() -> String {
    [
        ":help",
        ":load PATH",
        ":ast",
        ":semantic",
        ":ir",
        ":bindings",
        ":modules",
        ":choose INDEX",
        ":quit",
    ]
    .join("\n")
}

// ---------------------------------------------------------------------------
// Result formatters
// ---------------------------------------------------------------------------

pub fn format_load_result(result: &LoadResult) -> String {
    if result.modules.is_empty() {
        "loaded 0 modules".to_string()
    } else {
        format!("loaded {}", result.modules.join(", "))
    }
}

pub fn format_submission_result(result: &SubmissionResult) -> String {
    match result {
        SubmissionResult::ContextUpdated => "context updated".to_string(),
        SubmissionResult::ModuleUpdated { module_name } => {
            format!("module {module_name} updated")
        }
        SubmissionResult::Executed(execution) => format_execution_result(execution),
    }
}

pub fn format_execution_result(result: &ExecutionResult) -> String {
    let mut lines = result
        .events
        .iter()
        .map(|event| match event {
            StepEvent::Text { text, tag } => match tag {
                Some(tag) => format!("text[{tag}]: {text:?}"),
                None => format!("text: {text:?}"),
            },
        })
        .collect::<Vec<_>>();
    match &result.state {
        ExecutionState::Ready => lines.push("ready".to_string()),
        ExecutionState::Exited => lines.push("exited".to_string()),
        ExecutionState::SuspendedChoice { prompt, items } => lines.push(format!(
            "choice prompt={} items={items:?}",
            format_option_string(prompt.as_deref())
        )),
    }
    lines.join("\n")
}

fn format_option_string(value: Option<&str>) -> String {
    value
        .map(|text| format!("{text:?}"))
        .unwrap_or_else(|| "none".to_string())
}
