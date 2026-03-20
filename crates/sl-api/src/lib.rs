use std::collections::BTreeMap;

use sl_core::{CompiledArtifact, Form, ScriptLangError};

pub use sl_compiler::compile_artifact;
pub use sl_core;
pub use sl_parser::{parse_module_xml, parse_modules_from_xml_map};
pub use sl_runtime::Engine;

const BUNDLED_LIBRARY_SOURCES: &[(&str, &str)] =
    &[("lib/kernel.xml", include_str!("../lib/kernel.xml"))];

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

pub fn create_engine_from_xml_map(
    sources: &BTreeMap<String, String>,
    entry_script_ref: Option<&str>,
) -> Result<Engine, ScriptLangError> {
    let artifact = compile_artifact_from_xml_map(sources)?;
    let mut engine = sl_runtime::Engine::new(artifact);
    engine.start(entry_script_ref)?;
    Ok(engine)
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sl_core::{Completion, StepEvent, StepResult};

    use super::{create_engine_from_xml_map, parse_modules_from_sources};

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
    fn create_engine_from_xml_map_exposes_kernel_zero_const_to_user_code() {
        let sources = BTreeMap::from([(
            "main.xml".to_string(),
            r#"
            <module name="main">
              <var name="answer">zero + 1</var>
              <script name="main">
                <text>${answer}</text>
                <end/>
              </script>
            </module>
            "#
            .to_string(),
        )]);
        let mut engine = create_engine_from_xml_map(&sources, None).expect("engine");

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
}
