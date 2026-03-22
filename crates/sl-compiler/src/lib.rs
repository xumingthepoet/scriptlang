mod assemble;
mod names;
mod pipeline;
mod semantic;

pub use pipeline::{
    CompileOptions, CompilePipeline, assemble_semantic_program,
    assemble_semantic_program_with_options, compile_artifact, compile_artifact_with_options,
    compile_pipeline, compile_pipeline_with_options, expand_to_semantic,
};
pub use semantic::types::{
    DeclaredType, SemanticChoiceOption, SemanticFunction, SemanticModule, SemanticProgram,
    SemanticScript, SemanticStmt, SemanticVar,
};
