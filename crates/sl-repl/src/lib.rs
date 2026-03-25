//! REPL session management.
//!
//! This crate provides a read-eval-print loop session for ScriptLang.
//! The main entry point is [`ReplSession`].
//!
//! ## Module structure
//!
//! - [`session`] – Core `ReplSession` struct and all state management
//! - [`commands`] – Command parsing, public types, and basic result formatters

#[path = "commands.rs"]
mod commands;
#[path = "session.rs"]
mod session;

// ---------------------------------------------------------------------------
// Public re-exports
// ---------------------------------------------------------------------------

pub use commands::{ExecutionResult, ExecutionState, InspectTarget, LoadResult, SubmissionResult};
pub use session::ReplSession;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use sl_core::StepEvent;

    use super::{ExecutionState, InspectTarget, LoadResult, ReplSession, SubmissionResult};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos();
            let path = temp_dir().join(format!("sl-repl-{label}-{}-{unique}", process::id()));
            fs::create_dir_all(&path).expect("temp dir should create");
            Self { path }
        }

        fn write(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.path.join(name);
            fs::write(&path, contents).expect("temp file should write");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn text_events(result: &SubmissionResult) -> Vec<String> {
        match result {
            SubmissionResult::Executed(execution) => execution
                .events
                .iter()
                .map(|event| match event {
                    StepEvent::Text { text, .. } => text.clone(),
                })
                .collect(),
            other => panic!("expected execution result, got {other:?}"),
        }
    }

    fn execution_state(result: SubmissionResult) -> ExecutionState {
        match result {
            SubmissionResult::Executed(execution) => execution.state,
            other => panic!("expected execution result, got {other:?}"),
        }
    }

    #[test]
    fn text_submission_prints_and_returns_to_prompt() {
        let mut repl = ReplSession::new().expect("repl");

        let result = repl
            .submit_xml("<text>hello</text>")
            .expect("text should run");

        assert_eq!(text_events(&result), vec!["hello".to_string()]);
        assert_eq!(execution_state(result), ExecutionState::Ready);
        assert!(!repl.is_exited());
    }

    #[test]
    fn temp_bindings_persist_and_can_be_mutated() {
        let mut repl = ReplSession::new().expect("repl");

        assert!(matches!(
            repl.submit_xml(r#"<temp name="x" type="int">1</temp>"#)
                .expect("temp should run"),
            SubmissionResult::Executed(_)
        ));
        assert!(matches!(
            repl.submit_xml("<code>x = x + 2;</code>")
                .expect("code should run"),
            SubmissionResult::Executed(_)
        ));

        let result = repl
            .submit_xml("<text>${x}</text>")
            .expect("text should run");
        assert_eq!(text_events(&result), vec!["3".to_string()]);
        assert_eq!(repl.inspect(InspectTarget::Bindings), "temp x: int = 3");
    }

    #[test]
    fn choice_suspends_and_choose_resumes_to_prompt() {
        let mut repl = ReplSession::new().expect("repl");

        let state = execution_state(
            repl.submit_xml(
                r#"
                <choice text="pick">
                  <option text="left">
                    <text>L</text>
                  </option>
                  <option text="right">
                    <text>R</text>
                  </option>
                </choice>
                "#,
            )
            .expect("choice should suspend"),
        );
        assert_eq!(
            state,
            ExecutionState::SuspendedChoice {
                prompt: Some("pick".to_string()),
                items: vec!["left".to_string(), "right".to_string()],
            }
        );
        let resumed = repl.choose(1).expect("choose should work");
        assert_eq!(
            resumed.events,
            vec![StepEvent::Text {
                text: "R".to_string(),
                tag: None,
            }]
        );
        assert_eq!(resumed.state, ExecutionState::Ready);
    }

    #[test]
    fn end_exits_the_repl() {
        let mut repl = ReplSession::new().expect("repl");

        let result = repl.submit_xml("<end/>").expect("end should run");

        assert_eq!(execution_state(result), ExecutionState::Exited);
        assert!(repl.is_exited());
    }

    #[test]
    fn goto_into_loaded_script_exits_after_target_end() {
        let dir = TestDir::new("goto");
        let file = dir.write(
            "helper.xml",
            r#"
            <module name="helper">
              <script name="target">
                <text>from target</text>
                <end/>
              </script>
            </module>
            "#,
        );
        let mut repl = ReplSession::new().expect("repl");
        let load = repl.load_path(&file).expect("file load should work");
        assert_eq!(
            load,
            LoadResult {
                modules: vec!["helper".to_string()]
            }
        );

        let result = repl
            .submit_xml(r#"<goto script="@helper.target"/>"#)
            .expect("goto should run");

        assert_eq!(text_events(&result), vec!["from target".to_string()]);
        assert_eq!(execution_state(result), ExecutionState::Exited);
        assert!(repl.is_exited());
    }

    #[test]
    fn kernel_if_macro_works_at_top_level() {
        let mut repl = ReplSession::new().expect("repl");

        let result = repl
            .submit_xml(
                r#"
                <if when="true">
                  <text>inside</text>
                </if>
                "#,
            )
            .expect("if should run");

        assert_eq!(text_events(&result), vec!["inside".to_string()]);
    }

    #[test]
    fn require_enables_repl_defined_macro() {
        let mut repl = ReplSession::new().expect("repl");

        assert!(matches!(
            repl.submit_xml(
                r#"
                <module name="helper">
                  <macro name="mk">
                    <quote>
                      <text>hello</text>
                    </quote>
                  </macro>
                </module>
                "#,
            )
            .expect("module should define"),
            SubmissionResult::ModuleUpdated { .. }
        ));
        assert!(matches!(
            repl.submit_xml(r#"<require name="helper"/>"#)
                .expect("require should work"),
            SubmissionResult::ContextUpdated
        ));

        let result = repl.submit_xml("<mk/>").expect("macro should run");
        assert_eq!(text_events(&result), vec!["hello".to_string()]);
    }

    #[test]
    fn repl_defined_module_replaces_same_named_module() {
        let mut repl = ReplSession::new().expect("repl");

        repl.submit_xml(
            r#"
            <module name="helper">
              <macro name="mk">
                <quote><text>old</text></quote>
              </macro>
            </module>
            "#,
        )
        .expect("module should define");
        repl.submit_xml(r#"<require name="helper"/>"#)
            .expect("require should work");
        let first = repl.submit_xml("<mk/>").expect("first macro should run");
        assert_eq!(text_events(&first), vec!["old".to_string()]);

        repl.submit_xml(
            r#"
            <module name="helper">
              <macro name="mk">
                <quote><text>new</text></quote>
              </macro>
            </module>
            "#,
        )
        .expect("replacement module should define");
        let second = repl.submit_xml("<mk/>").expect("second macro should run");
        assert_eq!(text_events(&second), vec!["new".to_string()]);
    }

    #[test]
    fn load_directory_and_imported_var_work_without_autorun() {
        let dir = TestDir::new("dir-load");
        dir.write(
            "a.xml",
            r#"
            <module name="main">
              <script name="main">
                <text>should not autorun</text>
                <end/>
              </script>
            </module>
            "#,
        );
        dir.write(
            "b.xml",
            r#"
            <module name="helper">
              <var name="value" type="int">7</var>
            </module>
            "#,
        );
        let mut repl = ReplSession::new().expect("repl");
        let load = repl.load_path(&dir.path).expect("dir load should work");
        assert_eq!(
            load,
            LoadResult {
                modules: vec!["main".to_string(), "helper".to_string()]
            }
        );
        assert_eq!(repl.inspect(InspectTarget::Bindings), "no bindings");

        repl.submit_xml(r#"<import name="helper"/>"#)
            .expect("import should work");
        let result = repl
            .submit_xml("<text>${value}</text>")
            .expect("text should run");
        assert_eq!(text_events(&result), vec!["7".to_string()]);
    }

    #[test]
    fn runtime_failures_do_not_mutate_persisted_state() {
        let mut repl = ReplSession::new().expect("repl");

        repl.submit_xml(r#"<temp name="x" type="int">1</temp>"#)
            .expect("temp should run");
        let error = repl
            .submit_xml("<code>x = missing;</code>")
            .expect_err("runtime error should fail");
        assert!(error.to_string().contains("rhai eval error"));

        let result = repl
            .submit_xml("<text>${x}</text>")
            .expect("temp should still persist");
        assert_eq!(text_events(&result), vec!["1".to_string()]);
    }

    #[test]
    fn repl_defined_module_can_define_script_and_function_and_run_them() {
        let mut repl = ReplSession::new().expect("repl");

        assert!(matches!(
            repl.submit_xml(
                r#"
                <module name="helper">
                  <function name="pick" args="" return_type="string">return "picked";</function>
                  <script name="main">
                    <text>${invoke(#pick, [])}</text>
                    <end/>
                  </script>
                </module>
                "#,
            )
            .expect("module with script/function should define"),
            SubmissionResult::ModuleUpdated { .. }
        ));

        let result = repl
            .submit_xml(r#"<goto script="@helper.main"/>"#)
            .expect("goto should run repl-defined script");
        assert_eq!(text_events(&result), vec!["picked".to_string()]);
        assert_eq!(execution_state(result), ExecutionState::Exited);
    }

    #[test]
    fn top_level_xml_submission_can_mix_module_and_executable_forms() {
        let mut repl = ReplSession::new().expect("repl");

        let results = repl
            .submit_top_level_xml(
                r#"
                <module name="demo">
                  <script name="run">
                    <text>inside</text>
                    <end/>
                  </script>
                </module>
                <text>before</text>
                <goto script="@demo.run"/>
                "#,
            )
            .expect("top-level xml should submit");

        assert_eq!(results.len(), 3);
        assert_eq!(text_events(&results[1]), vec!["before".to_string()]);
        assert_eq!(text_events(&results[2]), vec!["inside".to_string()]);
        assert!(repl.is_exited());
    }

    #[test]
    fn failed_top_level_xml_submission_does_not_mutate_session() {
        let mut repl = ReplSession::new().expect("repl");

        let error = repl
            .submit_top_level_xml(
                r#"
                <module name="demo">
                  <script name="run">
                    <text>inside</text>
                    <end/>
                  </script>
                </module>
                <goto script="@missing.run"/>
                "#,
            )
            .expect_err("submission should fail transactionally");
        assert!(error.to_string().contains("unknown script `missing.run`"));

        assert_eq!(repl.inspect(InspectTarget::Modules), "kernel");
    }
}
