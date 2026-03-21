use sl_core::{CompiledArtifact, Form, ScriptLangError};

use crate::assemble::assemble_artifact;
use crate::semantic::expand_forms;

pub fn compile_artifact(forms: &[Form]) -> Result<CompiledArtifact, ScriptLangError> {
    let semantic_program = expand_forms(forms)?;
    assemble_artifact(&semantic_program)
}

#[cfg(test)]
mod tests {
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use super::compile_artifact;

    #[test]
    fn compile_artifact_pipeline_reports_expand_stage_errors() {
        let error = compile_artifact(&[Form {
            head: "module".to_string(),
            meta: FormMeta {
                source_name: Some("main.xml".to_string()),
                start: SourcePosition { row: 1, column: 1 },
                end: SourcePosition { row: 1, column: 20 },
                start_byte: 0,
                end_byte: 20,
            },
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(Vec::new()),
            }],
        }])
        .expect_err("missing module name should fail");

        assert!(error.to_string().contains("<module> requires `name`"));
    }

    #[test]
    fn compile_artifact_pipeline_reports_assemble_stage_errors() {
        let error = compile_artifact(&[Form {
            head: "module".to_string(),
            meta: FormMeta {
                source_name: Some("main.xml".to_string()),
                start: SourcePosition { row: 1, column: 1 },
                end: SourcePosition { row: 1, column: 20 },
                start_byte: 0,
                end_byte: 20,
            },
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("main".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![FormItem::Form(Form {
                        head: "script".to_string(),
                        meta: FormMeta {
                            source_name: Some("main.xml".to_string()),
                            start: SourcePosition { row: 1, column: 21 },
                            end: SourcePosition { row: 1, column: 40 },
                            start_byte: 20,
                            end_byte: 40,
                        },
                        fields: vec![
                            FormField {
                                name: "name".to_string(),
                                value: FormValue::String("entry".to_string()),
                            },
                            FormField {
                                name: "children".to_string(),
                                value: FormValue::Sequence(vec![FormItem::Form(Form {
                                    head: "end".to_string(),
                                    meta: FormMeta {
                                        source_name: Some("main.xml".to_string()),
                                        start: SourcePosition { row: 1, column: 30 },
                                        end: SourcePosition { row: 1, column: 36 },
                                        start_byte: 29,
                                        end_byte: 35,
                                    },
                                    fields: vec![FormField {
                                        name: "children".to_string(),
                                        value: FormValue::Sequence(Vec::new()),
                                    }],
                                })]),
                            },
                        ],
                    })]),
                },
            ],
        }])
        .expect_err("assemble should fail");

        assert!(
            error
                .to_string()
                .contains("default entry script `main.main`")
        );
    }
}
