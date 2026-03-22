use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sl_api::{Engine, compile_artifact_from_xml_map, start_runtime_session_from_xml_map};
use sl_core::{Completion, ScriptLangError, Snapshot, StepEvent, StepResult, Suspension};

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    SnapshotProgress(usize),
    SnapshotOnChoice,
    ResumeSnapshot,
    Choose(usize),
}

pub fn run_all_examples() {
    let mut cases = list_example_cases();
    cases.sort();

    for case in cases {
        run_case(&case);
    }
}

fn run_case(case_dir: &Path) {
    let xml_dir = case_dir.join("xml");
    let runs_dir = case_dir.join("runs");
    let sources = read_xml_dir(&xml_dir);
    let mut runs = fs::read_dir(&runs_dir)
        .unwrap_or_else(|err| panic!("failed to read runs dir {}: {err}", runs_dir.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("failed to list runs dir {}: {err}", runs_dir.display()));
    runs.sort_by_key(|entry| entry.path());

    for run in runs {
        let run_dir = run.path();
        if !run_dir.is_dir() {
            continue;
        }
        let label = format!(
            "{}/{}",
            case_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<invalid-case>"),
            run_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<invalid-run>")
        );

        let error_path = run_dir.join("error.txt");
        if error_path.exists() {
            let expected_error = read_lines(&error_path).join("\n");
            let error = compile_artifact_from_xml_map(&sources)
                .expect_err("compile failure scenario should fail");
            assert!(
                error.to_string().contains(expected_error.trim()),
                "case {label}: expected compile error containing {:?}, got {:?}",
                expected_error.trim(),
                error.to_string()
            );
            continue;
        }

        let actions = read_actions(&run_dir.join("actions.txt"));
        let expected = read_lines(&run_dir.join("results.txt"));
        let actual = execute_run(&sources, &actions)
            .unwrap_or_else(|err| panic!("case {label}: run failed: {err}"));
        assert_eq!(actual, expected, "case {label} results mismatch");
    }
}

fn execute_run(
    sources: &BTreeMap<String, String>,
    actions: &[Action],
) -> Result<Vec<String>, ScriptLangError> {
    let mut engine = start_runtime_session_from_xml_map(sources, None)?;
    let mut actual = Vec::new();
    let mut snapshot = None::<Snapshot>;
    let mut action_index = 0usize;
    let mut resume_pending_choice = false;

    loop {
        while let Some(action) = actions.get(action_index) {
            match action {
                Action::SnapshotProgress(count) => {
                    advance_progress_steps(&mut engine, *count)?;
                    snapshot = Some(engine.snapshot());
                    action_index += 1;
                }
                Action::ResumeSnapshot => {
                    let saved = snapshot.clone().ok_or_else(|| {
                        ScriptLangError::message("resume requested without snapshot")
                    })?;
                    let mut resumed = start_runtime_session_from_xml_map(sources, None)?;
                    resumed.resume(saved)?;
                    engine = resumed;
                    resume_pending_choice = matches!(
                        actions.get(action_index.saturating_sub(1)),
                        Some(Action::SnapshotOnChoice)
                    );
                    action_index += 1;
                }
                Action::Choose(index) if resume_pending_choice => {
                    engine.choose(*index)?;
                    resume_pending_choice = false;
                    action_index += 1;
                }
                _ => break,
            }
        }

        let result = next_visible(&mut engine)?;
        match result {
            StepResult::Event(StepEvent::Text { text, .. }) => actual.push(format!("text {text}")),
            StepResult::Suspended(Suspension::Choice { prompt, items }) => {
                let prompt = prompt.unwrap_or_default();
                let mut rendered = String::from("choice ");
                rendered.push_str(&prompt);
                for item in items {
                    rendered.push_str(" | ");
                    rendered.push_str(&item);
                }
                actual.push(rendered);

                if let Some(Action::SnapshotOnChoice) = actions.get(action_index) {
                    snapshot = Some(engine.snapshot());
                    action_index += 1;
                }
                if let Some(Action::Choose(index)) = actions.get(action_index) {
                    engine.choose(*index)?;
                    action_index += 1;
                }
            }
            StepResult::Completed(Completion::End) => {
                actual.push("end".to_string());
                break;
            }
            StepResult::Completed(Completion::ReturnToHost) => {
                return Err(ScriptLangError::message(
                    "integration test engine unexpectedly returned to host",
                ));
            }
            StepResult::Progress => unreachable!("next_visible filters progress"),
        }
    }

    if action_index != actions.len() {
        return Err(ScriptLangError::message(format!(
            "unused actions remain: {:?}",
            &actions[action_index..]
        )));
    }

    Ok(actual)
}

fn advance_progress_steps(engine: &mut Engine, count: usize) -> Result<(), ScriptLangError> {
    let mut progressed = 0usize;
    while progressed < count {
        match engine.step()? {
            StepResult::Progress => progressed += 1,
            other => {
                return Err(ScriptLangError::message(format!(
                    "expected progress step while snapshotting, got {other:?}"
                )));
            }
        }
    }
    Ok(())
}

fn next_visible(engine: &mut Engine) -> Result<StepResult, ScriptLangError> {
    loop {
        match engine.step()? {
            StepResult::Progress => continue,
            other => return Ok(other),
        }
    }
}

fn list_example_cases() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut entries = fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("failed to read examples dir {}: {err}", root.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("failed to list examples dir {}: {err}", root.display()));
    entries.sort_by_key(|entry| entry.path());
    entries
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

fn read_xml_dir(dir: &Path) -> BTreeMap<String, String> {
    let mut entries = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read xml dir {}: {err}", dir.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("failed to list xml dir {}: {err}", dir.display()));
    entries.sort_by_key(|entry| entry.path());

    let mut sources = BTreeMap::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("xml") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| panic!("invalid xml file name: {}", path.display()));
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read xml file {}: {err}", path.display()));
        sources.insert(name.to_string(), contents);
    }
    assert!(!sources.is_empty(), "xml dir {} is empty", dir.display());
    sources
}

fn read_actions(path: &Path) -> Vec<Action> {
    if !path.exists() {
        return Vec::new();
    }
    read_lines(path)
        .into_iter()
        .map(|line| parse_action(&line))
        .collect()
}

fn read_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_action(line: &str) -> Action {
    if let Some(rest) = line.strip_prefix("choose ") {
        return Action::Choose(
            rest.parse::<usize>()
                .unwrap_or_else(|err| panic!("invalid choose action `{line}`: {err}")),
        );
    }
    if let Some(rest) = line.strip_prefix("snapshot-progress ") {
        return Action::SnapshotProgress(
            rest.parse::<usize>()
                .unwrap_or_else(|err| panic!("invalid snapshot-progress action `{line}`: {err}")),
        );
    }
    if line == "snapshot-on-choice" {
        return Action::SnapshotOnChoice;
    }
    if line == "resume-snapshot" {
        return Action::ResumeSnapshot;
    }
    panic!("unknown action `{line}`");
}
