use std::collections::BTreeMap;

use rhai::Dynamic;
use sl_core::{CompiledArtifact, Form, ScriptLangError};

pub use sl_compiler::compile_artifact;
pub use sl_core;
pub use sl_parser::{parse_module_xml, parse_modules_from_xml_map};
pub use sl_runtime::Engine;

pub fn parse_modules_from_sources(
    sources: &BTreeMap<String, String>,
) -> Result<Vec<Form>, ScriptLangError> {
    sl_parser::parse_modules_from_xml_map(sources)
}

pub fn compile_artifact_from_xml_map(
    sources: &BTreeMap<String, String>,
) -> Result<CompiledArtifact, ScriptLangError> {
    let modules = sl_parser::parse_modules_from_xml_map(sources)?;
    sl_compiler::compile_artifact(&modules)
}

pub fn create_engine_from_xml_map(
    sources: &BTreeMap<String, String>,
    entry_script_ref: Option<&str>,
) -> Result<Engine, ScriptLangError> {
    let artifact = compile_artifact_from_xml_map(sources)?;
    let mut engine = sl_runtime::Engine::new(artifact);
    engine.start(entry_script_ref, None::<BTreeMap<String, Dynamic>>)?;
    Ok(engine)
}
