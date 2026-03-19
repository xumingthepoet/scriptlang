mod execute;
mod state;

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use rhai::{AST, Dynamic, Engine as RhaiEngine, Scope};
use sl_core::{
    CompiledArtifact, CompiledScript, CompiledText, CompiledTextPart, PendingChoiceOption,
    PendingChoiceSnapshot, ScriptId, ScriptLangError, Snapshot,
};

use self::state::{EngineState, PendingChoiceState};

pub struct Engine {
    pub(crate) artifact: Arc<CompiledArtifact>,
    pub(crate) state: EngineState,
    pub(crate) rhai: RhaiEngine,
    pub(crate) ast_cache: HashMap<String, AST>,
}

impl Engine {
    pub fn new(artifact: CompiledArtifact) -> Self {
        let artifact = Arc::new(artifact);
        let state = EngineState::for_boot(&artifact);
        Self {
            artifact,
            state,
            rhai: RhaiEngine::new(),
            ast_cache: HashMap::new(),
        }
    }

    pub fn start(
        &mut self,
        entry_script_ref: Option<&str>,
        _args: Option<BTreeMap<String, Dynamic>>,
    ) -> Result<(), ScriptLangError> {
        let entry_override = match entry_script_ref {
            Some(script_ref) => Some(self.resolve_script_id(script_ref)?),
            None => None,
        };
        self.state = EngineState::started(&self.artifact, entry_override);
        Ok(())
    }

    pub fn choose(&mut self, index: usize) -> Result<(), ScriptLangError> {
        let pending = self
            .state
            .pending
            .take()
            .ok_or_else(|| ScriptLangError::message("no pending choice"))?;
        let selected = pending
            .options
            .get(index)
            .ok_or_else(|| ScriptLangError::message("choice index out of range"))?;
        self.state.pc = selected.target_pc;
        Ok(())
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            script_id: self.state.script_id,
            pc: self.state.pc,
            globals: self.state.globals.clone(),
            locals: self.state.locals.clone(),
            pending: self
                .state
                .pending
                .as_ref()
                .map(|pending| PendingChoiceSnapshot {
                    prompt: pending.prompt.clone(),
                    options: pending.options.clone(),
                }),
            current_condition: self.state.current_condition,
            started: self.state.started,
            halted: self.state.halted,
            entry_override: self.state.entry_override,
        }
    }

    pub fn resume(&mut self, snapshot: Snapshot) -> Result<(), ScriptLangError> {
        let script = self
            .artifact
            .scripts
            .get(snapshot.script_id)
            .ok_or_else(|| ScriptLangError::message("snapshot refers to unknown script"))?;
        if snapshot.pc > script.instructions.len() {
            return Err(ScriptLangError::message(
                "snapshot refers to an instruction outside the script",
            ));
        }
        if snapshot.globals.len() != self.artifact.globals.len() {
            return Err(ScriptLangError::message("snapshot global shape mismatch"));
        }
        if snapshot.locals.len() != script.local_names.len() {
            return Err(ScriptLangError::message("snapshot local shape mismatch"));
        }
        self.state = EngineState {
            script_id: snapshot.script_id,
            pc: snapshot.pc,
            globals: snapshot.globals,
            locals: snapshot.locals,
            pending: snapshot.pending.map(|pending| PendingChoiceState {
                prompt: pending.prompt,
                options: pending.options,
            }),
            current_condition: snapshot.current_condition,
            started: snapshot.started,
            halted: snapshot.halted,
            entry_override: snapshot.entry_override,
        };
        Ok(())
    }

    pub fn current_script_id(&self) -> ScriptId {
        self.state.script_id
    }

    pub fn current_pc(&self) -> usize {
        self.state.pc
    }

    pub(crate) fn resolve_script_id(&self, script_ref: &str) -> Result<ScriptId, ScriptLangError> {
        self.artifact
            .script_refs
            .get(script_ref)
            .copied()
            .ok_or_else(|| ScriptLangError::message(format!("unknown script `{script_ref}`")))
    }

    pub(crate) fn current_script(&self) -> &CompiledScript {
        &self.artifact.scripts[self.state.script_id]
    }

    pub(crate) fn jump_to_script(&mut self, script_id: ScriptId) {
        self.state.script_id = script_id;
        self.state.pc = 0;
        self.state.locals = self.artifact.scripts[script_id]
            .local_names
            .iter()
            .map(|_| Dynamic::UNIT)
            .collect();
        self.state.current_condition = None;
        self.state.pending = None;
    }

    pub(crate) fn render_text(&mut self, text: &CompiledText) -> Result<String, ScriptLangError> {
        let mut rendered = String::new();
        for part in &text.parts {
            match part {
                CompiledTextPart::Literal(text) => rendered.push_str(text),
                CompiledTextPart::Expr(expr) => {
                    let value = self.eval_expression(expr)?;
                    rendered.push_str(&dynamic_to_text(&value));
                }
            }
        }
        Ok(rendered)
    }

    pub(crate) fn eval_expression(&mut self, expr: &str) -> Result<Dynamic, ScriptLangError> {
        let ast = self.get_or_compile_ast(expr)?;
        let mut scope = self.build_scope();
        Ok(self.rhai.eval_ast_with_scope::<Dynamic>(&mut scope, &ast)?)
    }

    pub(crate) fn exec_code(&mut self, code: &str) -> Result<(), ScriptLangError> {
        let ast = self.get_or_compile_ast(code)?;
        let mut scope = self.build_scope();
        let _ = self.rhai.eval_ast_with_scope::<Dynamic>(&mut scope, &ast)?;
        self.write_scope_back(&scope);
        Ok(())
    }

    fn get_or_compile_ast(&mut self, source: &str) -> Result<AST, ScriptLangError> {
        if !self.ast_cache.contains_key(source) {
            let ast = self.rhai.compile(source)?;
            self.ast_cache.insert(source.to_string(), ast);
        }
        Ok(self
            .ast_cache
            .get(source)
            .expect("AST must exist after insertion")
            .clone())
    }

    fn build_scope(&self) -> Scope<'static> {
        let mut scope = Scope::new();
        let script = self.current_script();
        let mut shadowed_globals = HashMap::<&str, ()>::new();

        for (local_id, name) in script.local_names.iter().enumerate() {
            shadowed_globals.insert(name.as_str(), ());
            scope.push_dynamic(name.to_string(), self.state.locals[local_id].clone());
        }

        for global in &self.artifact.globals {
            if shadowed_globals.contains_key(global.short_name.as_str()) {
                continue;
            }
            scope.push_dynamic(
                global.short_name.clone(),
                self.state.globals[global.global_id].clone(),
            );
        }

        scope
    }

    fn write_scope_back(&mut self, scope: &Scope<'_>) {
        let script = self.current_script().clone();
        for (local_id, name) in script.local_names.iter().enumerate() {
            if let Some(value) = scope.get_value::<Dynamic>(name) {
                self.state.locals[local_id] = value;
            }
        }

        for global in &self.artifact.globals {
            if script
                .local_names
                .iter()
                .any(|local| local == &global.short_name)
            {
                continue;
            }
            if let Some(value) = scope.get_value::<Dynamic>(&global.short_name) {
                self.state.globals[global.global_id] = value;
            }
        }
    }
}

fn dynamic_to_bool(value: &Dynamic) -> Result<bool, ScriptLangError> {
    if let Some(value) = value.clone().try_cast::<bool>() {
        Ok(value)
    } else {
        Err(ScriptLangError::message(
            "condition expression must evaluate to a boolean",
        ))
    }
}

fn dynamic_to_text(value: &Dynamic) -> String {
    if value.is_unit() {
        String::new()
    } else {
        value.to_string()
    }
}
