use sl_core::{CompiledArtifact, Form, ScriptLangError};

use crate::assemble::{assemble_artifact, assemble_artifact_with_options};
use crate::semantic::{SemanticProgram, expand_forms};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileOptions {
    pub default_entry_script_ref: String,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            default_entry_script_ref: "main.main".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompilePipeline {
    pub semantic_program: SemanticProgram,
    pub artifact: CompiledArtifact,
}

pub fn expand_to_semantic(forms: &[Form]) -> Result<SemanticProgram, ScriptLangError> {
    expand_forms(forms)
}

pub fn assemble_semantic_program(
    semantic_program: &SemanticProgram,
) -> Result<CompiledArtifact, ScriptLangError> {
    assemble_artifact(semantic_program)
}

pub fn assemble_semantic_program_with_options(
    semantic_program: &SemanticProgram,
    options: &CompileOptions,
) -> Result<CompiledArtifact, ScriptLangError> {
    assemble_artifact_with_options(semantic_program, options)
}

pub fn compile_pipeline(forms: &[Form]) -> Result<CompilePipeline, ScriptLangError> {
    compile_pipeline_with_options(forms, &CompileOptions::default())
}

pub fn compile_pipeline_with_options(
    forms: &[Form],
    options: &CompileOptions,
) -> Result<CompilePipeline, ScriptLangError> {
    let semantic_program = expand_to_semantic(forms)?;
    let artifact = assemble_semantic_program_with_options(&semantic_program, options)?;
    Ok(CompilePipeline {
        semantic_program,
        artifact,
    })
}

pub fn compile_artifact(forms: &[Form]) -> Result<CompiledArtifact, ScriptLangError> {
    compile_artifact_with_options(forms, &CompileOptions::default())
}

pub fn compile_artifact_with_options(
    forms: &[Form],
    options: &CompileOptions,
) -> Result<CompiledArtifact, ScriptLangError> {
    Ok(compile_pipeline_with_options(forms, options)?.artifact)
}

#[cfg(test)]
mod tests {
    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use super::{CompileOptions, compile_artifact, compile_artifact_with_options};

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

    #[test]
    fn compile_artifact_with_options_uses_custom_default_entry_script() {
        let artifact = compile_artifact_with_options(
            &[Form {
                head: "module".to_string(),
                meta: FormMeta {
                    source_name: Some("main.xml".to_string()),
                    start: SourcePosition { row: 1, column: 1 },
                    end: SourcePosition { row: 1, column: 40 },
                    start_byte: 0,
                    end_byte: 40,
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
                                start: SourcePosition { row: 1, column: 10 },
                                end: SourcePosition { row: 1, column: 35 },
                                start_byte: 9,
                                end_byte: 34,
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
                                            start: SourcePosition { row: 1, column: 20 },
                                            end: SourcePosition { row: 1, column: 26 },
                                            start_byte: 19,
                                            end_byte: 25,
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
            }],
            &CompileOptions {
                default_entry_script_ref: "main.entry".to_string(),
            },
        )
        .expect("custom entry should compile");

        assert_eq!(artifact.default_entry_script_id, 0);
    }
}
