mod execute;
mod state;

use std::collections::HashMap;
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

    pub fn start(&mut self, entry_script_ref: Option<&str>) -> Result<(), ScriptLangError> {
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

    pub(crate) fn eval_script_key(&mut self, expr: &str) -> Result<String, ScriptLangError> {
        let value = self.eval_expression(expr)?;
        if let Some(value) = value.clone().try_cast::<String>() {
            Ok(value)
        } else {
            Err(ScriptLangError::message(
                "goto expression must evaluate to a script string",
            ))
        }
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
            if shadowed_globals.contains_key(global.runtime_name.as_str()) {
                continue;
            }
            scope.push_dynamic(
                global.runtime_name.clone(),
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
                .any(|local| local == &global.runtime_name)
            {
                continue;
            }
            if let Some(value) = scope.get_value::<Dynamic>(&global.runtime_name) {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rhai::Dynamic;
    use sl_core::{
        ChoiceBranch, CompiledArtifact, CompiledScript, CompiledText, CompiledTextPart, Completion,
        GlobalVar, Instruction, PendingChoiceOption, Snapshot, StepEvent, StepResult, Suspension,
    };

    use super::{Engine, dynamic_to_bool, dynamic_to_text};

    fn text(parts: Vec<CompiledTextPart>) -> CompiledText {
        CompiledText { parts }
    }

    fn artifact_with_scripts(
        scripts: Vec<(String, CompiledScript)>,
        globals: Vec<GlobalVar>,
        default_entry_script_id: usize,
        boot_instructions: Vec<Instruction>,
    ) -> CompiledArtifact {
        let boot_script_id = scripts.len();
        let mut script_refs = BTreeMap::from([("__boot__".to_string(), boot_script_id)]);
        let mut all_scripts = Vec::with_capacity(boot_script_id + 1);
        for (script_ref, script) in scripts {
            script_refs.insert(script_ref, script.script_id);
            all_scripts.push(script);
        }
        all_scripts.push(CompiledScript {
            script_id: boot_script_id,
            local_names: Vec::new(),
            instructions: boot_instructions,
        });

        CompiledArtifact {
            default_entry_script_id,
            boot_script_id,
            script_refs,
            scripts: all_scripts,
            globals,
        }
    }

    fn simple_engine(instructions: Vec<Instruction>) -> Engine {
        Engine::new(artifact_with_scripts(
            vec![(
                "main.entry".to_string(),
                CompiledScript {
                    script_id: 0,
                    local_names: vec!["name".to_string(), "x".to_string()],
                    instructions,
                },
            )],
            vec![GlobalVar {
                global_id: 0,
                runtime_name: "answer".to_string(),
            }],
            0,
            vec![
                Instruction::EvalGlobalInit {
                    global_id: 0,
                    expr: "40 + 2".to_string(),
                },
                Instruction::JumpScript {
                    target_script_id: 0,
                },
            ],
        ))
    }

    fn engine_with_boot(boot_instructions: Vec<Instruction>) -> Engine {
        Engine::new(artifact_with_scripts(
            vec![(
                "main.entry".to_string(),
                CompiledScript {
                    script_id: 0,
                    local_names: Vec::new(),
                    instructions: vec![Instruction::End],
                },
            )],
            Vec::new(),
            0,
            boot_instructions,
        ))
    }

    #[test]
    fn new_uses_boot_state_and_start_can_override_entry() {
        let mut engine = Engine::new(artifact_with_scripts(
            vec![
                (
                    "main.entry".to_string(),
                    CompiledScript {
                        script_id: 0,
                        local_names: Vec::new(),
                        instructions: vec![Instruction::End],
                    },
                ),
                (
                    "main.alt".to_string(),
                    CompiledScript {
                        script_id: 1,
                        local_names: Vec::new(),
                        instructions: vec![Instruction::End],
                    },
                ),
            ],
            Vec::new(),
            0,
            vec![Instruction::JumpScript {
                target_script_id: 0,
            }],
        ));

        assert_eq!(engine.current_script_id(), 2);
        assert_eq!(engine.current_pc(), 0);
        assert!(!engine.state.started);

        engine.start(Some("main.alt")).expect("start should work");
        assert_eq!(engine.current_script_id(), 2);
        assert_eq!(engine.state.entry_override, Some(1));
        assert!(engine.state.started);
    }

    #[test]
    fn start_rejects_unknown_entry_script() {
        let mut engine = simple_engine(vec![Instruction::End]);

        let error = engine.start(Some("missing")).expect_err("should fail");

        assert_eq!(error.to_string(), "unknown script `missing`");
    }

    #[test]
    fn step_auto_starts_and_executes_boot_then_entry() {
        let mut engine = simple_engine(vec![Instruction::End]);

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Progress
        ));
        assert_eq!(engine.state.globals[0].clone_cast::<i64>(), 42);
        assert_eq!(engine.current_script_id(), 1);
        assert_eq!(engine.current_pc(), 1);

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Progress
        ));
        assert_eq!(engine.current_script_id(), 0);
        assert_eq!(engine.current_pc(), 0);
        assert_eq!(engine.state.locals.len(), 2);
    }

    #[test]
    fn step_returns_completed_for_halted_or_pc_past_end() {
        let mut halted_engine = simple_engine(vec![Instruction::End]);
        halted_engine.state.started = true;
        halted_engine.state.halted = true;
        assert!(matches!(
            halted_engine.step().expect("step should work"),
            StepResult::Completed(Completion::End)
        ));

        let mut exhausted_engine = simple_engine(vec![Instruction::End]);
        exhausted_engine.start(None).expect("start should work");
        exhausted_engine.state.script_id = 0;
        exhausted_engine.state.pc = 1;
        assert!(matches!(
            exhausted_engine.step().expect("step should work"),
            StepResult::Completed(Completion::End)
        ));
        assert!(exhausted_engine.state.halted);
    }

    #[test]
    fn step_covers_instruction_variants_and_rhai_paths() {
        let mut engine = simple_engine(vec![
            Instruction::EvalTemp {
                local_id: 0,
                expr: "\"Ada\"".to_string(),
            },
            Instruction::ExecCode {
                code: "x = 1; x += answer;".to_string(),
            },
            Instruction::EmitText {
                text: text(vec![
                    CompiledTextPart::Literal("hello ".to_string()),
                    CompiledTextPart::Expr("name".to_string()),
                    CompiledTextPart::Literal(" ".to_string()),
                    CompiledTextPart::Expr("()".to_string()),
                ]),
                tag: Some("line".to_string()),
            },
            Instruction::EvalCond {
                expr: "x > 0".to_string(),
            },
            Instruction::JumpIfFalse { target_pc: 6 },
            Instruction::Jump { target_pc: 7 },
            Instruction::End,
            Instruction::JumpScriptExpr {
                expr: "\"main.entry\"".to_string(),
            },
        ]);

        engine.start(None).expect("start should work");
        engine.state.script_id = 0;
        engine.state.pc = 0;
        engine.state.locals = vec![Dynamic::UNIT, Dynamic::UNIT];
        engine.state.globals = vec![Dynamic::from(41_i64)];

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Progress
        ));
        assert_eq!(engine.state.locals[0].clone_cast::<String>(), "Ada");

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Progress
        ));
        assert_eq!(engine.state.locals[1].clone_cast::<i64>(), 42);

        let result = engine.step().expect("step should work");
        assert!(matches!(
            result,
            StepResult::Event(StepEvent::Text { text, tag })
                if text == "hello Ada " && tag.as_deref() == Some("line")
        ));

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Progress
        ));
        assert_eq!(engine.state.current_condition, Some(true));

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Progress
        ));
        assert_eq!(engine.current_pc(), 5);

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Progress
        ));
        assert_eq!(engine.current_pc(), 7);

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Progress
        ));
        assert_eq!(engine.current_pc(), 0);
    }

    #[test]
    fn jump_script_expr_requires_string_and_known_target() {
        let mut wrong_type = simple_engine(vec![Instruction::JumpScriptExpr {
            expr: "1 + 2".to_string(),
        }]);
        wrong_type.start(None).expect("start should work");
        wrong_type.state.script_id = 0;
        wrong_type.state.pc = 0;
        wrong_type.state.locals = vec![Dynamic::UNIT, Dynamic::UNIT];

        let error = wrong_type.step().expect_err("should fail");
        assert_eq!(
            error.to_string(),
            "goto expression must evaluate to a script string"
        );

        let mut missing = simple_engine(vec![Instruction::JumpScriptExpr {
            expr: "\"main.missing\"".to_string(),
        }]);
        missing.start(None).expect("start should work");
        missing.state.script_id = 0;
        missing.state.pc = 0;
        missing.state.locals = vec![Dynamic::UNIT, Dynamic::UNIT];

        let error = missing.step().expect_err("should fail");
        assert_eq!(error.to_string(), "unknown script `main.missing`");
    }

    #[test]
    fn jump_if_false_false_branch_and_end_are_executed() {
        let mut engine = simple_engine(vec![
            Instruction::EvalCond {
                expr: "false".to_string(),
            },
            Instruction::JumpIfFalse { target_pc: 3 },
            Instruction::ExecCode {
                code: "x = 999;".to_string(),
            },
            Instruction::End,
        ]);

        engine.start(None).expect("start should work");
        engine.state.script_id = 0;
        engine.state.pc = 0;
        engine.state.locals = vec![Dynamic::UNIT, Dynamic::UNIT];

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Progress
        ));
        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Progress
        ));
        assert_eq!(engine.current_pc(), 3);
        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Completed(Completion::End)
        ));
        assert!(engine.state.halted);
    }

    #[test]
    fn build_choice_suspends_and_choose_updates_pc() {
        let mut engine = simple_engine(vec![Instruction::BuildChoice {
            prompt: Some(text(vec![CompiledTextPart::Literal("pick".to_string())])),
            options: vec![
                ChoiceBranch {
                    text: text(vec![CompiledTextPart::Literal("left".to_string())]),
                    target_pc: 2,
                },
                ChoiceBranch {
                    text: text(vec![CompiledTextPart::Literal("right".to_string())]),
                    target_pc: 3,
                },
            ],
        }]);

        engine.start(None).expect("start should work");
        engine.state.script_id = 0;
        engine.state.pc = 0;

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Suspended(Suspension::Choice { prompt, items })
                if prompt.as_deref() == Some("pick") && items == vec!["left".to_string(), "right".to_string()]
        ));
        assert_eq!(engine.current_pc(), 1);
        assert_eq!(
            engine
                .snapshot()
                .pending
                .expect("pending should exist")
                .options[0]
                .text,
            "left"
        );

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Suspended(Suspension::Choice { .. })
        ));

        engine.choose(1).expect("choose should work");
        assert_eq!(engine.current_pc(), 3);
        assert!(engine.state.pending.is_none());
    }

    #[test]
    fn build_choice_without_prompt_and_jump_if_false_without_condition_error() {
        let mut engine = simple_engine(vec![
            Instruction::BuildChoice {
                prompt: None,
                options: vec![ChoiceBranch {
                    text: text(vec![CompiledTextPart::Literal("only".to_string())]),
                    target_pc: 1,
                }],
            },
            Instruction::JumpIfFalse { target_pc: 2 },
        ]);

        engine.start(None).expect("start should work");
        engine.state.script_id = 0;
        engine.state.pc = 0;
        engine.state.locals = vec![Dynamic::UNIT, Dynamic::UNIT];

        assert!(matches!(
            engine.step().expect("step should work"),
            StepResult::Suspended(Suspension::Choice { prompt, items })
                if prompt.is_none() && items == vec!["only".to_string()]
        ));

        engine.choose(0).expect("choose should work");
        let error = engine.step().expect_err("should fail");
        assert_eq!(error.to_string(), "missing condition for JumpIfFalse");
    }

    #[test]
    fn choose_rejects_missing_or_out_of_range_pending_choice() {
        let mut engine = simple_engine(vec![Instruction::End]);
        let error = engine.choose(0).expect_err("should fail");
        assert_eq!(error.to_string(), "no pending choice");

        engine.state.pending = Some(super::PendingChoiceState {
            prompt: None,
            options: vec![PendingChoiceOption {
                text: "only".to_string(),
                target_pc: 1,
            }],
        });
        let error = engine.choose(2).expect_err("should fail");
        assert_eq!(error.to_string(), "choice index out of range");
    }

    #[test]
    fn snapshot_and_resume_cover_success_and_error_paths() {
        let mut engine = simple_engine(vec![Instruction::End]);
        engine.start(None).expect("start should work");
        engine.state.script_id = 0;
        engine.state.pc = 0;
        engine.state.locals = vec![Dynamic::from("Ada"), Dynamic::from(1_i64)];
        engine.state.globals = vec![Dynamic::from(42_i64)];
        engine.state.pending = Some(super::PendingChoiceState {
            prompt: Some("pick".to_string()),
            options: vec![PendingChoiceOption {
                text: "left".to_string(),
                target_pc: 1,
            }],
        });
        engine.state.current_condition = Some(true);

        let snapshot = engine.snapshot();
        let mut resumed = simple_engine(vec![Instruction::End]);
        resumed
            .resume(snapshot.clone())
            .expect("resume should work");
        assert_eq!(resumed.current_script_id(), 0);
        assert_eq!(resumed.current_pc(), 0);
        assert_eq!(resumed.state.locals[0].clone_cast::<String>(), "Ada");
        assert_eq!(
            resumed
                .state
                .pending
                .as_ref()
                .expect("pending should exist")
                .options[0]
                .text,
            "left"
        );

        let mut unknown_script = simple_engine(vec![Instruction::End]);
        let error = unknown_script
            .resume(Snapshot {
                script_id: 9,
                ..snapshot.clone()
            })
            .expect_err("should fail");
        assert_eq!(error.to_string(), "snapshot refers to unknown script");

        let mut bad_pc = simple_engine(vec![Instruction::End]);
        let error = bad_pc
            .resume(Snapshot {
                pc: 2,
                ..snapshot.clone()
            })
            .expect_err("should fail");
        assert_eq!(
            error.to_string(),
            "snapshot refers to an instruction outside the script"
        );

        let mut bad_globals = simple_engine(vec![Instruction::End]);
        let error = bad_globals
            .resume(Snapshot {
                globals: vec![Dynamic::from(1_i64), Dynamic::from(2_i64)],
                ..snapshot.clone()
            })
            .expect_err("should fail");
        assert_eq!(error.to_string(), "snapshot global shape mismatch");

        let mut bad_locals = simple_engine(vec![Instruction::End]);
        let error = bad_locals
            .resume(Snapshot {
                locals: vec![Dynamic::from(1_i64)],
                ..snapshot
            })
            .expect_err("should fail");
        assert_eq!(error.to_string(), "snapshot local shape mismatch");
    }

    #[test]
    fn helper_methods_cover_scope_cache_and_conversion_logic() {
        let mut engine = simple_engine(vec![Instruction::End]);
        engine.start(None).expect("start should work");
        engine.state.script_id = 0;
        engine.state.globals = vec![Dynamic::from(10_i64)];
        engine.state.locals = vec![Dynamic::from(3_i64), Dynamic::from(0_i64)];

        assert_eq!(
            engine
                .resolve_script_id("main.entry")
                .expect("should resolve"),
            0
        );
        let script = CompiledScript {
            script_id: 0,
            local_names: vec!["answer".to_string()],
            instructions: vec![Instruction::End],
        };
        let artifact = artifact_with_scripts(
            vec![("main.shadow".to_string(), script)],
            vec![GlobalVar {
                global_id: 0,
                runtime_name: "answer".to_string(),
            }],
            0,
            vec![Instruction::JumpScript {
                target_script_id: 0,
            }],
        );
        let mut shadow_engine = Engine::new(artifact);
        shadow_engine.start(None).expect("start should work");
        shadow_engine.state.script_id = 0;
        shadow_engine.state.locals = vec![Dynamic::from(7_i64)];
        shadow_engine.state.globals = vec![Dynamic::from(11_i64)];
        shadow_engine
            .exec_code("answer = answer + 1;")
            .expect("exec should work");
        assert_eq!(shadow_engine.state.locals[0].clone_cast::<i64>(), 8);
        assert_eq!(shadow_engine.state.globals[0].clone_cast::<i64>(), 11);

        let first = engine
            .eval_expression("answer + x")
            .expect("eval should work");
        let second = engine
            .eval_expression("answer + x")
            .expect("eval should work");
        assert_eq!(first.clone_cast::<i64>(), 10);
        assert_eq!(second.clone_cast::<i64>(), 10);
        assert_eq!(engine.ast_cache.len(), 1);
        assert_eq!(
            engine
                .eval_script_key("\"main.entry\"")
                .expect("script key should work"),
            "main.entry"
        );

        let error = engine
            .eval_expression("answer =")
            .expect_err("parse should fail");
        assert!(error.to_string().contains("rhai parse error"));
        let error = engine
            .exec_code("unknown = missing;")
            .expect_err("eval should fail");
        assert!(error.to_string().contains("rhai eval error"));

        assert!(dynamic_to_bool(&Dynamic::from(true)).expect("bool should convert"));
        let error = dynamic_to_bool(&Dynamic::from(1_i64)).expect_err("should fail");
        assert_eq!(
            error.to_string(),
            "condition expression must evaluate to a boolean"
        );
        let error = engine
            .eval_script_key("1")
            .expect_err("script key should fail");
        assert_eq!(
            error.to_string(),
            "goto expression must evaluate to a script string"
        );
        assert_eq!(dynamic_to_text(&Dynamic::UNIT), "");
        assert_eq!(dynamic_to_text(&Dynamic::from(9_i64)), "9");
    }

    #[test]
    fn jump_script_at_boot_without_override_uses_instruction_target() {
        let mut engine = engine_with_boot(vec![Instruction::JumpScript {
            target_script_id: 0,
        }]);

        let step = engine.step().expect("step should work");

        assert!(matches!(step, StepResult::Progress));
        assert_eq!(engine.current_script_id(), 0);
        assert_eq!(engine.current_pc(), 0);
        assert_eq!(engine.state.entry_override, None);
    }
}
