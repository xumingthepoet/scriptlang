use std::collections::BTreeMap;

use sl_compiler::{CompileOptions, compile_artifact_with_options};
use sl_core::{
    CompiledArtifact, Form, FormField, FormItem, FormMeta, FormValue, ScriptLangError,
    SourcePosition,
};

pub use sl_compiler::compile_artifact;
pub use sl_core;
pub use sl_parser::{parse_module_xml, parse_modules_from_xml_map};
pub use sl_runtime::Engine;

const BUNDLED_LIBRARY_SOURCES: &[(&str, &str)] =
    &[("lib/kernel.xml", include_str!("../lib/kernel.xml"))];
const API_SESSION_MODULE: &str = "__sl_api__";
const API_SESSION_SCRIPT: &str = "__entry__";
const API_SESSION_SCRIPT_REF: &str = "__sl_api__.__entry__";

pub fn parse_modules_from_sources(
    sources: &BTreeMap<String, String>,
) -> Result<Vec<Form>, ScriptLangError> {
    sl_parser::parse_modules_from_xml_map(&merged_sources(sources))
}

pub fn compile_artifact_from_xml_map(
    sources: &BTreeMap<String, String>,
) -> Result<CompiledArtifact, ScriptLangError> {
    let modules = parse_modules_from_sources(sources)?;
    sl_compiler::compile_artifact(&modules)
}

pub fn start_runtime_session_from_xml_map(
    sources: &BTreeMap<String, String>,
    entry_script_ref: Option<&str>,
) -> Result<Engine, ScriptLangError> {
    let entry_script_ref = entry_script_ref.unwrap_or("main.main");
    let mut modules = parse_modules_from_sources(sources)?;
    reject_reserved_session_module(&modules)?;
    modules.push(build_entry_session_module(entry_script_ref));
    let artifact = compile_artifact_with_options(
        &modules,
        &CompileOptions {
            default_entry_script_ref: API_SESSION_SCRIPT_REF.to_string(),
        },
    )?;
    let mut engine = sl_runtime::Engine::new(artifact);
    engine.start(None)?;
    Ok(engine)
}

pub fn create_engine_from_xml_map(
    sources: &BTreeMap<String, String>,
    entry_script_ref: Option<&str>,
) -> Result<Engine, ScriptLangError> {
    start_runtime_session_from_xml_map(sources, entry_script_ref)
}

fn merged_sources(user_sources: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut sources = BTreeMap::from_iter(
        BUNDLED_LIBRARY_SOURCES
            .iter()
            .map(|(name, xml)| (name.to_string(), (*xml).to_string())),
    );
    sources.extend(user_sources.clone());
    sources
}

fn reject_reserved_session_module(forms: &[Form]) -> Result<(), ScriptLangError> {
    if forms.iter().any(|form| {
        form.head == "module" && module_name(form).is_ok_and(|name| name == API_SESSION_MODULE)
    }) {
        return Err(ScriptLangError::message(format!(
            "module name `{API_SESSION_MODULE}` is reserved for api runtime entry"
        )));
    }
    Ok(())
}

fn build_entry_session_module(entry_script_ref: &str) -> Form {
    build_form(
        "module",
        vec![("name", API_SESSION_MODULE.to_string())],
        vec![FormItem::Form(build_form(
            "script",
            vec![("name", API_SESSION_SCRIPT.to_string())],
            vec![FormItem::Form(build_form(
                "goto",
                vec![("script", format!("'{entry_script_ref}'"))],
                Vec::new(),
            ))],
        ))],
    )
}

fn build_form(head: &str, attrs: Vec<(&str, String)>, children: Vec<FormItem>) -> Form {
    let mut fields = attrs
        .into_iter()
        .map(|(name, value)| FormField {
            name: name.to_string(),
            value: FormValue::String(value),
        })
        .collect::<Vec<_>>();
    fields.push(FormField {
        name: "children".to_string(),
        value: FormValue::Sequence(children),
    });
    Form {
        head: head.to_string(),
        meta: synthetic_meta(),
        fields,
    }
}

fn synthetic_meta() -> FormMeta {
    FormMeta {
        source_name: Some("<sl-api>".to_string()),
        start: SourcePosition { row: 1, column: 1 },
        end: SourcePosition { row: 1, column: 1 },
        start_byte: 0,
        end_byte: 0,
    }
}

fn module_name(form: &Form) -> Result<&str, ScriptLangError> {
    form.fields
        .iter()
        .find(|field| field.name == "name")
        .ok_or_else(|| ScriptLangError::message("expected a <module> form with `name`"))
        .and_then(|field| match &field.value {
            FormValue::String(value) => Ok(value.as_str()),
            FormValue::Sequence(_) => {
                Err(ScriptLangError::message("<module>.name must be a string"))
            }
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sl_core::{Completion, StepEvent, StepResult};

    use super::{parse_modules_from_sources, start_runtime_session_from_xml_map};

    #[test]
    fn parse_modules_from_sources_includes_bundled_library_modules() {
        let modules = parse_modules_from_sources(&BTreeMap::from([(
            "main.xml".to_string(),
            "<module name=\"main\"><script name=\"main\"><end/></script></module>".to_string(),
        )]))
        .expect("parse");

        assert_eq!(modules.len(), 2);
        assert_eq!(
            modules[0].meta.source_name.as_deref(),
            Some("lib/kernel.xml")
        );
        assert_eq!(modules[1].meta.source_name.as_deref(), Some("main.xml"));
    }

    #[test]
    fn start_runtime_session_from_xml_map_uses_user_defined_zero_const() {
        let sources = BTreeMap::from([(
            "main.xml".to_string(),
            r#"
            <module name="main">
              <const name="zero" type="int">0</const>
              <var name="answer" type="int">zero + 1</var>
              <script name="main">
                <text>${answer}</text>
                <end/>
              </script>
            </module>
            "#
            .to_string(),
        )]);
        let mut engine = start_runtime_session_from_xml_map(&sources, None).expect("engine");

        loop {
            match engine.step().expect("step") {
                StepResult::Progress => continue,
                StepResult::Event(StepEvent::Text { text, .. }) => {
                    assert_eq!(text, "1");
                    break;
                }
                other => panic!("expected text event, got {other:?}"),
            }
        }

        assert!(matches!(
            engine.step().expect("step"),
            StepResult::Completed(Completion::End)
        ));
    }

    #[test]
    fn start_runtime_session_from_xml_map_expands_kernel_statement_macros() {
        let sources = BTreeMap::from([(
            "main.xml".to_string(),
            r#"
            <module name="main">
              <script name="main">
                <if when="true">
                  <text>hello</text>
                </if>
                <unless when="false">
                  <text>inside</text>
                </unless>
                <end/>
              </script>
            </module>
            "#
            .to_string(),
        )]);
        let mut engine = start_runtime_session_from_xml_map(&sources, None).expect("engine");
        let mut events = Vec::new();
        loop {
            match engine.step().expect("step") {
                StepResult::Progress => continue,
                StepResult::Event(StepEvent::Text { text, .. }) => events.push(text),
                StepResult::Completed(Completion::End) => break,
                other => panic!("unexpected step result: {other:?}"),
            }
        }

        assert_eq!(events, vec!["hello".to_string(), "inside".to_string()]);
    }

    #[test]
    fn start_runtime_session_from_xml_map_uses_hidden_session_goto_for_custom_entry() {
        let sources = BTreeMap::from([(
            "main.xml".to_string(),
            r#"
            <module name="main">
              <script name="main">
                <text>default</text>
                <end/>
              </script>
              <script name="alt">
                <text>alt</text>
                <end/>
              </script>
            </module>
            "#
            .to_string(),
        )]);
        let mut engine =
            start_runtime_session_from_xml_map(&sources, Some("main.alt")).expect("engine");
        match engine.step().expect("step") {
            StepResult::Progress => {}
            other => panic!("expected initial progress, got {other:?}"),
        }
        loop {
            match engine.step().expect("step") {
                StepResult::Progress => continue,
                StepResult::Event(StepEvent::Text { text, .. }) => {
                    assert_eq!(text, "alt");
                    break;
                }
                other => panic!("expected text event, got {other:?}"),
            }
        }
    }
}

#[cfg(test)]
mod hidden_helper_test {
    #[test]
    fn test_hidden_helper_no_conflict() {
        use super::*;
        use std::collections::BTreeMap;
        let sources = BTreeMap::from([
            (
                "helper.xml".to_string(),
                r#"
<module name="helper">
  <macro name="__using__" params="keyword:opts">
    <quote>
      <script name="helper" hidden="true">
        <text>hidden</text>
        <end/>
      </script>
    </quote>
  </macro>
</module>
"#
                .to_string(),
            ),
            (
                "main.xml".to_string(),
                r#"
<module name="main">
  <script name="helper">
    <text>caller</text>
    <end/>
  </script>
  <use module="helper"/>
  <script name="main">
    <goto script="@main.helper"/>
    <end/>
  </script>
</module>
"#
                .to_string(),
            ),
        ]);
        let engine = start_runtime_session_from_xml_map(&sources, None);
        match &engine {
            Ok(_) => println!("OK"),
            Err(e) => println!("ERROR: {}", e),
        }
        engine.expect("engine");
    }
}
