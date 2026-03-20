use std::collections::BTreeMap;

use rhai::Dynamic;
use sl_core::{CompiledArtifact, Form, FormField, FormItem, FormValue, ScriptLangError};

pub use sl_compiler::compile_artifact;
pub use sl_core;
pub use sl_parser::{parse_module_xml, parse_modules_from_xml_map};
pub use sl_runtime::Engine;

const CHILDREN_FIELD: &str = "children";
const BUNDLED_LIBRARY_SOURCES: &[(&str, &str)] =
    &[("lib/kernel.xml", include_str!("../lib/kernel.xml"))];

pub fn parse_modules_from_sources(
    sources: &BTreeMap<String, String>,
) -> Result<Vec<Form>, ScriptLangError> {
    let mut modules = sl_parser::parse_modules_from_xml_map(sources)?;
    let bundled_const_items = bundled_const_items()?;
    inject_bundled_const_items(&mut modules, &bundled_const_items)?;
    Ok(modules)
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
    engine.start(entry_script_ref, None::<BTreeMap<String, Dynamic>>)?;
    Ok(engine)
}

fn bundled_const_items() -> Result<Vec<FormItem>, ScriptLangError> {
    let sources = BTreeMap::from_iter(
        BUNDLED_LIBRARY_SOURCES
            .iter()
            .map(|(name, xml)| (name.to_string(), (*xml).to_string())),
    );
    let modules = sl_parser::parse_modules_from_xml_map(&sources)?;
    let mut const_items = Vec::new();

    for module in &modules {
        for item in children_items(module)? {
            match item {
                FormItem::Form(form) if form.head == "const" => {
                    const_items.push(FormItem::Form(form.clone()));
                }
                FormItem::Text(text) if text.trim().is_empty() => {}
                FormItem::Form(form) => {
                    return Err(ScriptLangError::message(format!(
                        "bundled library currently only supports module-level <const>, found <{}>",
                        form.head
                    )));
                }
                FormItem::Text(_) => {
                    return Err(ScriptLangError::message(
                        "bundled library modules do not support text children",
                    ));
                }
            }
        }
    }

    Ok(const_items)
}

fn inject_bundled_const_items(
    modules: &mut [Form],
    bundled_const_items: &[FormItem],
) -> Result<(), ScriptLangError> {
    if bundled_const_items.is_empty() {
        return Ok(());
    }

    for module in modules {
        let existing_items = children_items(module)?.to_vec();
        let mut merged_items = bundled_const_items.to_vec();
        merged_items.extend(existing_items);
        *children_items_mut(module)? = merged_items;
    }

    Ok(())
}

fn children_items(form: &Form) -> Result<&[FormItem], ScriptLangError> {
    match form
        .fields
        .iter()
        .find(|field| field.name == CHILDREN_FIELD)
    {
        Some(FormField {
            value: FormValue::Sequence(items),
            ..
        }) => Ok(items),
        Some(FormField {
            value: FormValue::String(_),
            ..
        }) => Err(ScriptLangError::message(format!(
            "<{}> has invalid `{CHILDREN_FIELD}` field shape",
            form.head
        ))),
        None => Err(ScriptLangError::message(format!(
            "<{}> is missing `{CHILDREN_FIELD}` field",
            form.head
        ))),
    }
}

fn children_items_mut(form: &mut Form) -> Result<&mut Vec<FormItem>, ScriptLangError> {
    match form
        .fields
        .iter_mut()
        .find(|field| field.name == CHILDREN_FIELD)
    {
        Some(FormField {
            value: FormValue::Sequence(items),
            ..
        }) => Ok(items),
        Some(FormField {
            value: FormValue::String(_),
            ..
        }) => Err(ScriptLangError::message(format!(
            "<{}> has invalid `{CHILDREN_FIELD}` field shape",
            form.head
        ))),
        None => Err(ScriptLangError::message(format!(
            "<{}> is missing `{CHILDREN_FIELD}` field",
            form.head
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sl_core::{Completion, StepEvent, StepResult};

    use super::{create_engine_from_xml_map, parse_modules_from_sources};

    #[test]
    fn parse_modules_from_sources_injects_bundled_kernel_consts() {
        let modules = parse_modules_from_sources(&BTreeMap::from([(
            "main.xml".to_string(),
            "<module name=\"main\"><script name=\"entry\"><end/></script></module>".to_string(),
        )]))
        .expect("parse");

        let module = &modules[0];
        let children = match &module.fields[1].value {
            sl_core::FormValue::Sequence(items) => items,
            other => panic!("expected children sequence, got {other:?}"),
        };

        assert!(matches!(&children[0], sl_core::FormItem::Form(form) if form.head == "const"));
        assert!(matches!(
            &children[0],
            sl_core::FormItem::Form(form)
                if form.fields.iter().any(|field| field.name == "name"
                    && matches!(&field.value, sl_core::FormValue::String(value) if value == "zero"))
        ));
    }

    #[test]
    fn create_engine_from_xml_map_exposes_kernel_zero_const_to_user_code() {
        let sources = BTreeMap::from([(
            "main.xml".to_string(),
            r#"
            <module name="main">
              <var name="answer">zero + 1</var>
              <script name="entry">
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
