use std::collections::BTreeMap;

use crate::{GlobalId, LocalId, ScriptId};

#[derive(Clone, Debug)]
pub struct CompiledArtifact {
    pub default_entry_script_id: ScriptId,
    pub boot_script_id: ScriptId,
    pub script_refs: BTreeMap<String, ScriptId>,
    pub scripts: Vec<CompiledScript>,
    pub globals: Vec<GlobalVar>,
}

#[derive(Clone, Debug)]
pub struct CompiledScript {
    pub script_id: ScriptId,
    pub script_ref: String,
    pub local_names: Vec<String>,
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug)]
pub struct GlobalVar {
    pub global_id: GlobalId,
    pub qualified_name: String,
    pub short_name: String,
    pub initializer: String,
}

#[derive(Clone, Debug)]
pub enum Instruction {
    EvalGlobalInit {
        global_id: GlobalId,
        expr: String,
    },
    EvalTemp {
        local_id: LocalId,
        expr: String,
    },
    EvalCond {
        expr: String,
    },
    ExecCode {
        code: String,
    },
    EmitText {
        text: CompiledText,
        tag: Option<String>,
    },
    BuildChoice {
        prompt: Option<CompiledText>,
        options: Vec<ChoiceBranch>,
    },
    JumpIfFalse {
        target_pc: usize,
    },
    Jump {
        target_pc: usize,
    },
    JumpScript {
        target_script_id: ScriptId,
    },
    End,
}

#[derive(Clone, Debug)]
pub struct ChoiceBranch {
    pub text: CompiledText,
    pub target_pc: usize,
}

#[derive(Clone, Debug)]
pub struct CompiledText {
    pub parts: Vec<CompiledTextPart>,
}

#[derive(Clone, Debug)]
pub enum CompiledTextPart {
    Literal(String),
    Expr(String),
}
