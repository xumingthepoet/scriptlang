use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use rhai::Dynamic;
use sl_compiler::{
    CompileOptions, CompilePipeline, DeclaredType, SemanticChoiceOption, SemanticModule,
    SemanticProgram, SemanticScript, SemanticStmt, compile_pipeline_with_options,
};
use sl_core::{
    CompiledArtifact, CompiledText, CompiledTextPart, Completion, Form, FormField, FormItem,
    FormMeta, FormValue, Instruction, ScriptLangError, Snapshot, SourcePosition, StepEvent,
    StepResult, Suspension, TextSegment,
};
use sl_parser::{parse_modules_from_xml_map, parse_xml_fragment};
use sl_runtime::Engine;

const KERNEL_SOURCE_NAME: &str = "lib/kernel.xml";
const KERNEL_SOURCE_XML: &str = include_str!("../../sl-api/lib/kernel.xml");
const RESERVED_SESSION_MODULE: &str = "__repl__";
const RESERVED_SESSION_SCRIPT: &str = "__session__";
const SESSION_SCRIPT_REF: &str = "__repl__.__session__";
const SENTINEL_CODE_EXPR: &str = "()";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectTarget {
    Ast,
    Semantic,
    Ir,
    Bindings,
    Modules,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadResult {
    pub modules: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmissionResult {
    ContextUpdated,
    ModuleUpdated { module_name: String },
    Executed(ExecutionResult),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionResult {
    pub events: Vec<StepEvent>,
    pub state: ExecutionState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionState {
    Ready,
    SuspendedChoice {
        prompt: Option<String>,
        items: Vec<String>,
    },
    Exited,
}

#[derive(Clone)]
struct BuildOutput {
    forms: Vec<Form>,
    pipeline: CompilePipeline,
    prelude_temp_count: usize,
}

#[derive(Clone)]
struct PersistedTemp {
    name: String,
    declared_type: DeclaredType,
    value: Dynamic,
}

#[derive(Clone)]
struct CaptureBinding {
    name: String,
    declared_type: DeclaredType,
    existed_before: bool,
}

struct PendingExecution {
    build: BuildOutput,
    capture_bindings: Vec<CaptureBinding>,
    engine: Engine,
}

pub struct ReplSession {
    kernel_form: Form,
    modules: BTreeMap<String, Form>,
    session_context: Vec<Form>,
    persistent_temps: Vec<PersistedTemp>,
    persisted_globals: BTreeMap<String, Dynamic>,
    base_build: BuildOutput,
    last_build: BuildOutput,
    pending: Option<PendingExecution>,
    exited: bool,
}

impl ReplSession {
    pub fn new() -> Result<Self, ScriptLangError> {
        let mut kernel_forms = parse_modules_from_xml_map(&BTreeMap::from([(
            KERNEL_SOURCE_NAME.to_string(),
            KERNEL_SOURCE_XML.to_string(),
        )]))?;
        let kernel_form = kernel_forms
            .drain(..)
            .next()
            .ok_or_else(|| ScriptLangError::message("kernel module is missing"))?;

        let placeholder = BuildOutput {
            forms: Vec::new(),
            pipeline: CompilePipeline {
                semantic_program: SemanticProgram {
                    modules: Vec::new(),
                },
                artifact: CompiledArtifact {
                    default_entry_script_id: 0,
                    boot_script_id: 0,
                    functions: BTreeMap::new(),
                    script_refs: BTreeMap::new(),
                    scripts: Vec::new(),
                    globals: Vec::new(),
                },
            },
            prelude_temp_count: 0,
        };

        let mut session = Self {
            kernel_form,
            modules: BTreeMap::new(),
            session_context: Vec::new(),
            persistent_temps: Vec::new(),
            persisted_globals: BTreeMap::new(),
            base_build: placeholder.clone(),
            last_build: placeholder,
            pending: None,
            exited: false,
        };
        let base_build = session.build_program_with(
            &session.modules,
            &session.session_context,
            &session.persistent_temps,
            None,
        )?;
        session.base_build = base_build.clone();
        session.last_build = base_build;
        Ok(session)
    }

    pub fn is_exited(&self) -> bool {
        self.exited
    }

    pub fn has_pending_choice(&self) -> bool {
        self.pending.is_some()
    }

    pub fn quit(&mut self) {
        self.exited = true;
        self.pending = None;
    }

    pub fn forms(&self) -> &[Form] {
        &self.last_build.forms
    }

    pub fn semantic_program(&self) -> &SemanticProgram {
        &self.last_build.pipeline.semantic_program
    }

    pub fn artifact(&self) -> &CompiledArtifact {
        &self.last_build.pipeline.artifact
    }

    pub fn load_path<P: AsRef<Path>>(&mut self, path: P) -> Result<LoadResult, ScriptLangError> {
        self.ensure_ready_for_mutation()?;

        let loaded_forms = load_modules_from_path(path.as_ref())?;
        let mut candidate_modules = self.modules.clone();
        let mut loaded_names = Vec::new();
        for form in loaded_forms {
            validate_loaded_module(&form)?;
            let name = module_name(&form)?.to_string();
            if !loaded_names.iter().any(|loaded| loaded == &name) {
                loaded_names.push(name.clone());
            }
            candidate_modules.insert(name, form);
        }

        let build = self.build_program_with(
            &candidate_modules,
            &self.session_context,
            &self.persistent_temps,
            None,
        )?;
        self.modules = candidate_modules;
        self.base_build = build.clone();
        self.last_build = build;
        Ok(LoadResult {
            modules: loaded_names,
        })
    }

    pub fn submit_xml(&mut self, xml: &str) -> Result<SubmissionResult, ScriptLangError> {
        self.ensure_ready_for_mutation()?;
        let trimmed = xml.trim();
        if trimmed.is_empty() {
            return Err(ScriptLangError::message("empty repl xml input"));
        }
        let form = parse_xml_fragment(trimmed)?;
        self.submit_form(form)
    }

    pub fn choose(&mut self, index: usize) -> Result<ExecutionResult, ScriptLangError> {
        self.ensure_not_exited()?;
        let mut pending = self
            .pending
            .take()
            .ok_or_else(|| ScriptLangError::message("no pending choice"))?;

        if let Err(error) = pending.engine.choose(index) {
            self.pending = Some(pending);
            return Err(error);
        }

        match run_engine_until_boundary(pending.engine)? {
            ExecutionBoundary::Ready {
                engine,
                execution,
                snapshot,
            } => {
                self.commit_successful_execution(
                    &pending.build,
                    &snapshot,
                    &pending.capture_bindings,
                )?;
                self.last_build = pending.build;
                let _ = engine;
                Ok(execution)
            }
            ExecutionBoundary::Suspended { engine, execution } => {
                pending.engine = engine;
                self.pending = Some(pending);
                Ok(execution)
            }
            ExecutionBoundary::Exited { execution } => {
                self.exited = true;
                self.last_build = pending.build;
                Ok(execution)
            }
        }
    }

    pub fn inspect(&self, target: InspectTarget) -> String {
        match target {
            InspectTarget::Ast => format_forms(self.forms()),
            InspectTarget::Semantic => format_semantic_program(self.semantic_program()),
            InspectTarget::Ir => format_artifact(self.artifact()),
            InspectTarget::Bindings => {
                if let Some(pending) = &self.pending {
                    format_live_bindings(
                        &pending.build.pipeline.artifact,
                        &pending.engine.snapshot(),
                    )
                } else {
                    format_persisted_bindings(&self.persistent_temps, &self.persisted_globals)
                }
            }
            InspectTarget::Modules => format_modules(&self.modules),
        }
    }

    pub fn eval_command(&mut self, input: &str) -> Result<String, ScriptLangError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ScriptLangError::message("empty repl command"));
        }
        if !trimmed.starts_with(':') {
            return Ok(format_submission_result(&self.submit_xml(trimmed)?));
        }

        let (command, arg) = split_command(trimmed);
        match command {
            ":help" => Ok(help_text()),
            ":load" => {
                let path =
                    arg.ok_or_else(|| ScriptLangError::message("`:load` requires a path"))?;
                Ok(format_load_result(&self.load_path(path)?))
            }
            ":ast" => Ok(self.inspect(InspectTarget::Ast)),
            ":semantic" => Ok(self.inspect(InspectTarget::Semantic)),
            ":ir" => Ok(self.inspect(InspectTarget::Ir)),
            ":bindings" => Ok(self.inspect(InspectTarget::Bindings)),
            ":modules" => Ok(self.inspect(InspectTarget::Modules)),
            ":choose" => {
                let raw_index =
                    arg.ok_or_else(|| ScriptLangError::message("`:choose` requires an index"))?;
                let index = raw_index.parse::<usize>().map_err(|_| {
                    ScriptLangError::message(format!("invalid choice index `{raw_index}`"))
                })?;
                Ok(format_execution_result(&self.choose(index)?))
            }
            ":quit" => {
                self.quit();
                Ok("bye".to_string())
            }
            other => Err(ScriptLangError::message(format!(
                "unknown repl command `{other}`"
            ))),
        }
    }

    fn submit_form(&mut self, form: Form) -> Result<SubmissionResult, ScriptLangError> {
        match form.head.as_str() {
            "module" => self.submit_module_definition(form),
            "import" | "require" | "alias" => self.submit_session_context(form),
            "const" | "var" | "macro" | "script" | "function" => Err(ScriptLangError::message(
                format!("<{}> is not allowed at repl top level", form.head),
            )),
            _ => self.submit_statement(form),
        }
    }

    fn submit_module_definition(
        &mut self,
        form: Form,
    ) -> Result<SubmissionResult, ScriptLangError> {
        validate_repl_module(&form)?;
        let module_name = module_name(&form)?.to_string();
        let mut candidate_modules = self.modules.clone();
        candidate_modules.insert(module_name.clone(), form);
        let build = self.build_program_with(
            &candidate_modules,
            &self.session_context,
            &self.persistent_temps,
            None,
        )?;
        self.modules = candidate_modules;
        self.base_build = build.clone();
        self.last_build = build;
        Ok(SubmissionResult::ModuleUpdated { module_name })
    }

    fn submit_session_context(&mut self, form: Form) -> Result<SubmissionResult, ScriptLangError> {
        let mut candidate_context = self.session_context.clone();
        candidate_context.push(form);
        let build = self.build_program_with(
            &self.modules,
            &candidate_context,
            &self.persistent_temps,
            None,
        )?;
        self.session_context = candidate_context;
        self.base_build = build.clone();
        self.last_build = build;
        Ok(SubmissionResult::ContextUpdated)
    }

    fn submit_statement(&mut self, form: Form) -> Result<SubmissionResult, ScriptLangError> {
        let capture_bindings = build_capture_bindings(&self.persistent_temps, &form)?;
        let build = self.build_program_with(
            &self.modules,
            &self.session_context,
            &self.persistent_temps,
            Some(&form),
        )?;
        self.last_build = build.clone();

        let mut engine = Engine::new(build.pipeline.artifact.clone());
        self.prepare_engine_for_statement(&build, &mut engine)?;

        match run_engine_until_boundary(engine)? {
            ExecutionBoundary::Ready {
                engine: _,
                execution,
                snapshot,
            } => {
                self.commit_successful_execution(&build, &snapshot, &capture_bindings)?;
                self.last_build = build;
                Ok(SubmissionResult::Executed(execution))
            }
            ExecutionBoundary::Suspended { engine, execution } => {
                self.pending = Some(PendingExecution {
                    build: build.clone(),
                    capture_bindings,
                    engine,
                });
                Ok(SubmissionResult::Executed(execution))
            }
            ExecutionBoundary::Exited { execution } => {
                self.exited = true;
                self.last_build = build;
                Ok(SubmissionResult::Executed(execution))
            }
        }
    }

    fn prepare_engine_for_statement(
        &self,
        build: &BuildOutput,
        engine: &mut Engine,
    ) -> Result<(), ScriptLangError> {
        engine.start(Some(SESSION_SCRIPT_REF))?;
        while engine.current_script_id() == build.pipeline.artifact.boot_script_id {
            expect_progress(engine.step()?, "starting repl session")?;
        }
        self.restore_globals(build, engine)?;
        for _ in 0..build.prelude_temp_count {
            expect_progress(engine.step()?, "executing repl temp prelude")?;
        }
        self.restore_temps(build, engine)
    }

    fn restore_globals(
        &self,
        build: &BuildOutput,
        engine: &mut Engine,
    ) -> Result<(), ScriptLangError> {
        if self.persisted_globals.is_empty() {
            return Ok(());
        }
        let mut snapshot = engine.snapshot();
        for global in &build.pipeline.artifact.globals {
            if let Some(value) = self.persisted_globals.get(&global.runtime_name) {
                snapshot.globals[global.global_id] = value.clone();
            }
        }
        engine.resume(snapshot)
    }

    fn restore_temps(
        &self,
        build: &BuildOutput,
        engine: &mut Engine,
    ) -> Result<(), ScriptLangError> {
        if self.persistent_temps.is_empty() {
            return Ok(());
        }
        let mut snapshot = engine.snapshot();
        let session_script_id = session_script_id(&build.pipeline.artifact)?;
        if snapshot.script_id != session_script_id {
            return Err(ScriptLangError::message(
                "repl entry did not reach the hidden session script",
            ));
        }
        let local_lookup = local_lookup(&build.pipeline.artifact.scripts[session_script_id]);
        for temp in &self.persistent_temps {
            if let Some(local_id) = local_lookup.get(temp.name.as_str()) {
                snapshot.locals[*local_id] = temp.value.clone();
            }
        }
        engine.resume(snapshot)
    }

    fn commit_successful_execution(
        &mut self,
        build: &BuildOutput,
        snapshot: &Snapshot,
        capture_bindings: &[CaptureBinding],
    ) -> Result<(), ScriptLangError> {
        let session_script_id = session_script_id(&build.pipeline.artifact)?;
        if snapshot.script_id != session_script_id {
            return Err(ScriptLangError::message(
                "repl host return must complete inside the hidden session script",
            ));
        }

        let candidate_globals = build
            .pipeline
            .artifact
            .globals
            .iter()
            .map(|global| {
                (
                    global.runtime_name.clone(),
                    snapshot.globals[global.global_id].clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let locals = local_lookup(&build.pipeline.artifact.scripts[session_script_id]);
        let candidate_temps = capture_bindings
            .iter()
            .filter_map(|binding| {
                let local_id = locals.get(binding.name.as_str())?;
                let value = snapshot.locals[*local_id].clone();
                if !binding.existed_before && value.is_unit() {
                    None
                } else {
                    Some(PersistedTemp {
                        name: binding.name.clone(),
                        declared_type: binding.declared_type.clone(),
                        value,
                    })
                }
            })
            .collect::<Vec<_>>();

        let candidate_base =
            self.build_program_with(&self.modules, &self.session_context, &candidate_temps, None)?;
        self.persisted_globals = candidate_globals;
        self.persistent_temps = candidate_temps;
        self.base_build = candidate_base;
        Ok(())
    }

    fn build_program_with(
        &self,
        modules: &BTreeMap<String, Form>,
        session_context: &[Form],
        persistent_temps: &[PersistedTemp],
        statement: Option<&Form>,
    ) -> Result<BuildOutput, ScriptLangError> {
        let mut forms = Vec::with_capacity(modules.len() + 2);
        forms.push(self.kernel_form.clone());
        forms.extend(modules.values().cloned());
        forms.push(build_session_module(
            session_context,
            persistent_temps,
            statement,
        ));

        let mut pipeline = compile_pipeline_with_options(
            &forms,
            &CompileOptions {
                default_entry_script_ref: SESSION_SCRIPT_REF.to_string(),
            },
        )?;
        append_host_return(&mut pipeline.artifact)?;
        Ok(BuildOutput {
            forms,
            pipeline,
            prelude_temp_count: persistent_temps.len(),
        })
    }

    fn ensure_ready_for_mutation(&self) -> Result<(), ScriptLangError> {
        self.ensure_not_exited()?;
        if self.pending.is_some() {
            return Err(ScriptLangError::message(
                "a choice is pending; resolve it before submitting more xml",
            ));
        }
        Ok(())
    }

    fn ensure_not_exited(&self) -> Result<(), ScriptLangError> {
        if self.exited {
            Err(ScriptLangError::message("repl session has exited"))
        } else {
            Ok(())
        }
    }
}

enum ExecutionBoundary {
    Ready {
        engine: Engine,
        execution: ExecutionResult,
        snapshot: Snapshot,
    },
    Suspended {
        engine: Engine,
        execution: ExecutionResult,
    },
    Exited {
        execution: ExecutionResult,
    },
}

fn run_engine_until_boundary(mut engine: Engine) -> Result<ExecutionBoundary, ScriptLangError> {
    let mut events = Vec::new();
    loop {
        match engine.step()? {
            StepResult::Progress => {}
            StepResult::Event(event) => events.push(event),
            StepResult::Suspended(Suspension::Choice { prompt, items }) => {
                return Ok(ExecutionBoundary::Suspended {
                    engine,
                    execution: ExecutionResult {
                        events,
                        state: ExecutionState::SuspendedChoice { prompt, items },
                    },
                });
            }
            StepResult::Completed(Completion::ReturnToHost) => {
                let snapshot = engine.snapshot();
                return Ok(ExecutionBoundary::Ready {
                    engine,
                    execution: ExecutionResult {
                        events,
                        state: ExecutionState::Ready,
                    },
                    snapshot,
                });
            }
            StepResult::Completed(Completion::End) => {
                return Ok(ExecutionBoundary::Exited {
                    execution: ExecutionResult {
                        events,
                        state: ExecutionState::Exited,
                    },
                });
            }
        }
    }
}

fn expect_progress(result: StepResult, phase: &str) -> Result<(), ScriptLangError> {
    if matches!(result, StepResult::Progress) {
        Ok(())
    } else {
        Err(ScriptLangError::message(format!(
            "unexpected runtime boundary while {phase}: {result:?}"
        )))
    }
}

fn build_capture_bindings(
    persistent_temps: &[PersistedTemp],
    statement: &Form,
) -> Result<Vec<CaptureBinding>, ScriptLangError> {
    let mut bindings = persistent_temps
        .iter()
        .map(|temp| CaptureBinding {
            name: temp.name.clone(),
            declared_type: temp.declared_type.clone(),
            existed_before: true,
        })
        .collect::<Vec<_>>();
    let mut index_by_name = bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| (binding.name.clone(), index))
        .collect::<HashMap<_, _>>();
    let mut discovered = Vec::new();
    collect_temp_decls(statement, &mut discovered)?;
    for (name, declared_type) in discovered {
        if let Some(index) = index_by_name.get(name.as_str()) {
            bindings[*index].declared_type = declared_type;
        } else {
            let index = bindings.len();
            bindings.push(CaptureBinding {
                name: name.clone(),
                declared_type,
                existed_before: false,
            });
            index_by_name.insert(name, index);
        }
    }
    Ok(bindings)
}

fn collect_temp_decls(
    form: &Form,
    output: &mut Vec<(String, DeclaredType)>,
) -> Result<(), ScriptLangError> {
    if form.head == "temp" {
        output.push((
            string_attr(form, "name")?.to_string(),
            parse_declared_type(form)?,
        ));
    }
    for child in child_forms(form) {
        collect_temp_decls(child, output)?;
    }
    Ok(())
}

fn build_session_module(
    session_context: &[Form],
    persistent_temps: &[PersistedTemp],
    statement: Option<&Form>,
) -> Form {
    let mut module_children = session_context
        .iter()
        .cloned()
        .map(FormItem::Form)
        .collect::<Vec<_>>();
    let mut script_children = persistent_temps
        .iter()
        .map(|temp| {
            FormItem::Form(build_form(
                "temp",
                vec![
                    ("name", temp.name.clone()),
                    ("type", declared_type_name(&temp.declared_type).to_string()),
                ],
                vec![FormItem::Text(
                    default_value_expr(&temp.declared_type).to_string(),
                )],
            ))
        })
        .collect::<Vec<_>>();
    if let Some(statement) = statement {
        script_children.push(FormItem::Form(statement.clone()));
    }
    script_children.push(FormItem::Form(build_form(
        "code",
        Vec::new(),
        vec![FormItem::Text(SENTINEL_CODE_EXPR.to_string())],
    )));
    module_children.push(FormItem::Form(build_form(
        "script",
        vec![("name", RESERVED_SESSION_SCRIPT.to_string())],
        script_children,
    )));
    build_form(
        "module",
        vec![("name", RESERVED_SESSION_MODULE.to_string())],
        module_children,
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
        source_name: Some("<repl>".to_string()),
        start: SourcePosition { row: 1, column: 1 },
        end: SourcePosition { row: 1, column: 1 },
        start_byte: 0,
        end_byte: 0,
    }
}

fn default_value_expr(declared_type: &DeclaredType) -> &'static str {
    match declared_type {
        DeclaredType::Array => "[]",
        DeclaredType::Bool => "false",
        DeclaredType::Function | DeclaredType::Script | DeclaredType::String => "\"\"",
        DeclaredType::Int => "0",
        DeclaredType::Object => "#{}",
    }
}

fn append_host_return(artifact: &mut CompiledArtifact) -> Result<(), ScriptLangError> {
    let script_id = session_script_id(artifact)?;
    let script = artifact
        .scripts
        .get_mut(script_id)
        .ok_or_else(|| ScriptLangError::message("missing hidden repl session script"))?;
    match script.instructions.last_mut() {
        Some(last @ Instruction::End) => *last = Instruction::ReturnToHost,
        _ => script.instructions.push(Instruction::ReturnToHost),
    }
    Ok(())
}

fn session_script_id(artifact: &CompiledArtifact) -> Result<usize, ScriptLangError> {
    artifact
        .script_refs
        .get(SESSION_SCRIPT_REF)
        .copied()
        .ok_or_else(|| ScriptLangError::message("missing hidden repl session script"))
}

fn local_lookup(script: &sl_core::CompiledScript) -> HashMap<&str, usize> {
    script
        .local_names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect()
}

fn load_modules_from_path(path: &Path) -> Result<Vec<Form>, ScriptLangError> {
    if path.is_file() {
        return parse_modules_from_xml_map(&BTreeMap::from([(
            path.display().to_string(),
            read_file(path)?,
        )]));
    }
    if path.is_dir() {
        let mut paths = fs::read_dir(path)
            .map_err(|error| {
                ScriptLangError::message(format!(
                    "failed to read directory `{}`: {error}",
                    path.display()
                ))
            })?
            .map(|entry| {
                entry
                    .map_err(|error| {
                        ScriptLangError::message(format!(
                            "failed to read directory entry in `{}`: {error}",
                            path.display()
                        ))
                    })
                    .map(|entry| entry.path())
            })
            .collect::<Result<Vec<_>, _>>()?;
        paths.retain(|entry| entry.is_file() && entry.extension().is_some_and(|ext| ext == "xml"));
        paths.sort();
        if paths.is_empty() {
            return Err(ScriptLangError::message(format!(
                "directory `{}` does not contain any `.xml` files",
                path.display()
            )));
        }
        let sources = paths
            .into_iter()
            .map(|entry| Ok((entry.display().to_string(), read_file(&entry)?)))
            .collect::<Result<BTreeMap<_, _>, ScriptLangError>>()?;
        return parse_modules_from_xml_map(&sources);
    }
    Err(ScriptLangError::message(format!(
        "path `{}` does not exist",
        path.display()
    )))
}

fn read_file(path: &Path) -> Result<String, ScriptLangError> {
    fs::read_to_string(path).map_err(|error| {
        ScriptLangError::message(format!("failed to read `{}`: {error}", path.display()))
    })
}

fn validate_loaded_module(form: &Form) -> Result<(), ScriptLangError> {
    validate_reserved_module_name(form)
}

fn validate_repl_module(form: &Form) -> Result<(), ScriptLangError> {
    validate_reserved_module_name(form)
}

fn validate_reserved_module_name(form: &Form) -> Result<(), ScriptLangError> {
    let name = module_name(form)?;
    if name == RESERVED_SESSION_MODULE {
        return Err(ScriptLangError::message(format!(
            "module name `{RESERVED_SESSION_MODULE}` is reserved for the repl"
        )));
    }
    Ok(())
}

fn module_name(form: &Form) -> Result<&str, ScriptLangError> {
    if form.head != "module" {
        return Err(ScriptLangError::message("expected a <module> form"));
    }
    string_attr(form, "name")
}

fn string_attr<'a>(form: &'a Form, name: &str) -> Result<&'a str, ScriptLangError> {
    form.fields
        .iter()
        .find(|field| field.name == name)
        .ok_or_else(|| ScriptLangError::message(format!("<{}> requires `{name}`", form.head)))
        .and_then(|field| match &field.value {
            FormValue::String(value) => Ok(value.as_str()),
            FormValue::Sequence(_) => Err(ScriptLangError::message(format!(
                "<{}>.{name} must be a string",
                form.head
            ))),
        })
}

fn child_forms(form: &Form) -> Vec<&Form> {
    form.fields
        .iter()
        .find(|field| field.name == "children")
        .and_then(|field| match &field.value {
            FormValue::Sequence(items) => Some(
                items
                    .iter()
                    .filter_map(|item| match item {
                        FormItem::Form(form) => Some(form),
                        FormItem::Text(_) => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            FormValue::String(_) => None,
        })
        .unwrap_or_default()
}

fn parse_declared_type(form: &Form) -> Result<DeclaredType, ScriptLangError> {
    parse_declared_type_name(string_attr(form, "type")?, &form.head)
}

fn parse_declared_type_name(raw: &str, head: &str) -> Result<DeclaredType, ScriptLangError> {
    match raw {
        "array" => Ok(DeclaredType::Array),
        "bool" => Ok(DeclaredType::Bool),
        "function" => Ok(DeclaredType::Function),
        "int" => Ok(DeclaredType::Int),
        "object" => Ok(DeclaredType::Object),
        "script" => Ok(DeclaredType::Script),
        "string" => Ok(DeclaredType::String),
        other => Err(ScriptLangError::message(format!(
            "<{head}> has unsupported type `{other}`"
        ))),
    }
}

fn declared_type_name(declared_type: &DeclaredType) -> &'static str {
    match declared_type {
        DeclaredType::Array => "array",
        DeclaredType::Bool => "bool",
        DeclaredType::Function => "function",
        DeclaredType::Int => "int",
        DeclaredType::Object => "object",
        DeclaredType::Script => "script",
        DeclaredType::String => "string",
    }
}

fn split_command(input: &str) -> (&str, Option<&str>) {
    match input.split_once(char::is_whitespace) {
        Some((command, rest)) => (command, Some(rest.trim()).filter(|rest| !rest.is_empty())),
        None => (input, None),
    }
}

fn help_text() -> String {
    [
        ":help",
        ":load PATH",
        ":ast",
        ":semantic",
        ":ir",
        ":bindings",
        ":modules",
        ":choose INDEX",
        ":quit",
    ]
    .join("\n")
}

fn format_load_result(result: &LoadResult) -> String {
    if result.modules.is_empty() {
        "loaded 0 modules".to_string()
    } else {
        format!("loaded {}", result.modules.join(", "))
    }
}

fn format_submission_result(result: &SubmissionResult) -> String {
    match result {
        SubmissionResult::ContextUpdated => "context updated".to_string(),
        SubmissionResult::ModuleUpdated { module_name } => {
            format!("module {module_name} updated")
        }
        SubmissionResult::Executed(execution) => format_execution_result(execution),
    }
}

fn format_execution_result(result: &ExecutionResult) -> String {
    let mut lines = result
        .events
        .iter()
        .map(|event| match event {
            StepEvent::Text { text, tag } => match tag {
                Some(tag) => format!("text[{tag}]: {text:?}"),
                None => format!("text: {text:?}"),
            },
        })
        .collect::<Vec<_>>();
    match &result.state {
        ExecutionState::Ready => lines.push("ready".to_string()),
        ExecutionState::Exited => lines.push("exited".to_string()),
        ExecutionState::SuspendedChoice { prompt, items } => lines.push(format!(
            "choice prompt={} items={items:?}",
            format_option_string(prompt.as_deref())
        )),
    }
    lines.join("\n")
}

fn format_modules(modules: &BTreeMap<String, Form>) -> String {
    let mut names = vec!["kernel".to_string()];
    names.extend(modules.keys().cloned());
    names.join("\n")
}

fn format_live_bindings(artifact: &CompiledArtifact, snapshot: &Snapshot) -> String {
    let globals = artifact
        .globals
        .iter()
        .zip(snapshot.globals.iter())
        .map(|(global, value)| format!("global {} = {}", global.runtime_name, value))
        .collect::<Vec<_>>();
    let locals = artifact
        .scripts
        .get(snapshot.script_id)
        .map(|script| {
            script
                .local_names
                .iter()
                .zip(snapshot.locals.iter())
                .map(|(name, value)| format!("temp {name} = {value}"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut lines = Vec::new();
    lines.extend(globals);
    lines.extend(locals);
    if lines.is_empty() {
        "no bindings".to_string()
    } else {
        lines.join("\n")
    }
}

fn format_persisted_bindings(
    persistent_temps: &[PersistedTemp],
    persisted_globals: &BTreeMap<String, Dynamic>,
) -> String {
    let mut lines = Vec::new();
    for (name, value) in persisted_globals {
        lines.push(format!("global {name} = {value}"));
    }
    for temp in persistent_temps {
        lines.push(format!(
            "temp {}: {} = {}",
            temp.name,
            declared_type_name(&temp.declared_type),
            temp.value
        ));
    }
    if lines.is_empty() {
        "no bindings".to_string()
    } else {
        lines.join("\n")
    }
}

fn format_forms(forms: &[Form]) -> String {
    let mut lines = Vec::new();
    for form in forms {
        format_form(form, 0, &mut lines);
    }
    lines.join("\n")
}

fn format_form(form: &Form, indent: usize, lines: &mut Vec<String>) {
    let padding = "  ".repeat(indent);
    let attrs = form
        .fields
        .iter()
        .filter(|field| field.name != "children")
        .map(format_field)
        .collect::<Vec<_>>();
    if attrs.is_empty() {
        lines.push(format!("{padding}<{}>", form.head));
    } else {
        lines.push(format!("{padding}<{} {}>", form.head, attrs.join(" ")));
    }

    if let Some(children) = form
        .fields
        .iter()
        .find(|field| field.name == "children")
        .and_then(sequence_items)
    {
        for child in children {
            match child {
                FormItem::Text(text) => {
                    if !text.trim().is_empty() {
                        lines.push(format!("{padding}  text {:?}", text));
                    }
                }
                FormItem::Form(child) => format_form(child, indent + 1, lines),
            }
        }
    }
}

fn format_field(field: &FormField) -> String {
    match &field.value {
        FormValue::String(value) => format!("{}={value:?}", field.name),
        FormValue::Sequence(items) => format!("{}=[{} items]", field.name, items.len()),
    }
}

fn sequence_items(value: &FormField) -> Option<&[FormItem]> {
    match &value.value {
        FormValue::Sequence(items) => Some(items.as_slice()),
        FormValue::String(_) => None,
    }
}

fn format_semantic_program(program: &SemanticProgram) -> String {
    let mut lines = Vec::new();
    for module in &program.modules {
        format_semantic_module(module, &mut lines);
    }
    lines.join("\n")
}

fn format_semantic_module(module: &SemanticModule, lines: &mut Vec<String>) {
    lines.push(format!("module {}", module.name));
    for var in &module.vars {
        lines.push(format!(
            "  var {}: {} = {}",
            var.name,
            declared_type_name(&var.declared_type),
            var.expr
        ));
    }
    for function in &module.functions {
        lines.push(format!(
            "  function {}({}) -> {}",
            function.name,
            function.param_names.join(", "),
            declared_type_name(&function.return_type)
        ));
        lines.push(format!("    body {:?}", function.body));
    }
    for script in &module.scripts {
        format_semantic_script(script, lines);
    }
}

fn format_semantic_script(script: &SemanticScript, lines: &mut Vec<String>) {
    lines.push(format!("  script {}", script.name));
    for stmt in &script.body {
        format_semantic_stmt(stmt, 2, lines);
    }
}

fn format_semantic_stmt(stmt: &SemanticStmt, indent: usize, lines: &mut Vec<String>) {
    let padding = "  ".repeat(indent);
    match stmt {
        SemanticStmt::Temp {
            name,
            declared_type,
            expr,
        } => lines.push(format!(
            "{padding}temp {}: {} = {}",
            name,
            declared_type_name(declared_type),
            expr
        )),
        SemanticStmt::Code { code } => lines.push(format!("{padding}code {:?}", code)),
        SemanticStmt::Text { template, tag } => lines.push(format!(
            "{padding}text {} tag={}",
            format_text_template(template),
            format_option_string(tag.as_deref())
        )),
        SemanticStmt::While {
            when,
            body,
            skip_loop_control_capture,
        } => {
            lines.push(format!(
                "{padding}while when={when:?} skip_loop_control_capture={skip_loop_control_capture}"
            ));
            for child in body {
                format_semantic_stmt(child, indent + 1, lines);
            }
        }
        SemanticStmt::Break => lines.push(format!("{padding}break")),
        SemanticStmt::Continue => lines.push(format!("{padding}continue")),
        SemanticStmt::Choice { prompt, options } => {
            lines.push(format!(
                "{padding}choice prompt={}",
                prompt
                    .as_ref()
                    .map(format_text_template)
                    .unwrap_or_else(|| "none".to_string())
            ));
            for option in options {
                format_choice_option(option, indent + 1, lines);
            }
        }
        SemanticStmt::Goto { expr } => lines.push(format!("{padding}goto {expr}")),
        SemanticStmt::End => lines.push(format!("{padding}end")),
    }
}

fn format_choice_option(option: &SemanticChoiceOption, indent: usize, lines: &mut Vec<String>) {
    let padding = "  ".repeat(indent);
    lines.push(format!(
        "{padding}option {}",
        format_text_template(&option.text)
    ));
    for stmt in &option.body {
        format_semantic_stmt(stmt, indent + 1, lines);
    }
}

fn format_artifact(artifact: &CompiledArtifact) -> String {
    let mut lines = vec![
        format!(
            "default_entry_script_id {}",
            artifact.default_entry_script_id
        ),
        format!("boot_script_id {}", artifact.boot_script_id),
    ];

    if !artifact.globals.is_empty() {
        lines.push("globals".to_string());
        for global in &artifact.globals {
            lines.push(format!("  {} => {}", global.global_id, global.runtime_name));
        }
    }

    if !artifact.functions.is_empty() {
        lines.push("functions".to_string());
        for (name, function) in &artifact.functions {
            lines.push(format!("  {}({})", name, function.param_names.join(", ")));
            lines.push(format!("    body {:?}", function.body));
        }
    }

    lines.push("scripts".to_string());
    for script in &artifact.scripts {
        lines.push(format!(
            "  script {} ref={} locals={:?}",
            script.script_id,
            format_script_ref(artifact, script.script_id),
            script.local_names
        ));
        for (pc, instruction) in script.instructions.iter().enumerate() {
            lines.push(format!("    {pc:03}: {}", format_instruction(instruction)));
        }
    }

    lines.join("\n")
}

fn format_instruction(instruction: &Instruction) -> String {
    match instruction {
        Instruction::EvalGlobalInit { global_id, expr } => {
            format!("EvalGlobalInit global_id={global_id} expr={expr:?}")
        }
        Instruction::EvalTemp { local_id, expr } => {
            format!("EvalTemp local_id={local_id} expr={expr:?}")
        }
        Instruction::EvalCond { expr } => format!("EvalCond expr={expr:?}"),
        Instruction::ExecCode { code } => format!("ExecCode code={code:?}"),
        Instruction::EmitText { text, tag } => format!(
            "EmitText text={} tag={}",
            format_compiled_text(text),
            format_option_string(tag.as_deref())
        ),
        Instruction::BuildChoice { prompt, options } => format!(
            "BuildChoice prompt={} options={}",
            prompt
                .as_ref()
                .map(format_compiled_text)
                .unwrap_or_else(|| "none".to_string()),
            options
                .iter()
                .map(|option| format!(
                    "{} -> {}",
                    format_compiled_text(&option.text),
                    option.target_pc
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Instruction::JumpIfFalse { target_pc } => format!("JumpIfFalse target_pc={target_pc}"),
        Instruction::Jump { target_pc } => format!("Jump target_pc={target_pc}"),
        Instruction::JumpScript { target_script_id } => {
            format!("JumpScript target_script_id={target_script_id}")
        }
        Instruction::JumpScriptExpr { expr } => format!("JumpScriptExpr expr={expr:?}"),
        Instruction::ReturnToHost => "ReturnToHost".to_string(),
        Instruction::End => "End".to_string(),
    }
}

fn format_text_template(template: &sl_core::TextTemplate) -> String {
    template
        .segments
        .iter()
        .map(|segment| match segment {
            TextSegment::Literal(text) => format!("lit({text:?})"),
            TextSegment::Expr(expr) => format!("expr({expr:?})"),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn format_compiled_text(text: &CompiledText) -> String {
    text.parts
        .iter()
        .map(|part| match part {
            CompiledTextPart::Literal(text) => format!("lit({text:?})"),
            CompiledTextPart::Expr(expr) => format!("expr({expr:?})"),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn format_option_string(value: Option<&str>) -> String {
    value
        .map(|text| format!("{text:?}"))
        .unwrap_or_else(|| "none".to_string())
}

fn format_script_ref(artifact: &CompiledArtifact, script_id: usize) -> String {
    artifact
        .script_refs
        .iter()
        .find(|(_, id)| **id == script_id)
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| format!("#{script_id}"))
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use sl_core::StepEvent;

    use super::{ExecutionState, InspectTarget, LoadResult, ReplSession, SubmissionResult};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos();
            let path = temp_dir().join(format!("sl-repl-{label}-{}-{unique}", process::id()));
            fs::create_dir_all(&path).expect("temp dir should create");
            Self { path }
        }

        fn write(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.path.join(name);
            fs::write(&path, contents).expect("temp file should write");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn text_events(result: &SubmissionResult) -> Vec<String> {
        match result {
            SubmissionResult::Executed(execution) => execution
                .events
                .iter()
                .map(|event| match event {
                    StepEvent::Text { text, .. } => text.clone(),
                })
                .collect(),
            other => panic!("expected execution result, got {other:?}"),
        }
    }

    fn execution_state(result: SubmissionResult) -> ExecutionState {
        match result {
            SubmissionResult::Executed(execution) => execution.state,
            other => panic!("expected execution result, got {other:?}"),
        }
    }

    #[test]
    fn text_submission_prints_and_returns_to_prompt() {
        let mut repl = ReplSession::new().expect("repl");

        let result = repl
            .submit_xml("<text>hello</text>")
            .expect("text should run");

        assert_eq!(text_events(&result), vec!["hello".to_string()]);
        assert_eq!(execution_state(result), ExecutionState::Ready);
        assert!(!repl.is_exited());
    }

    #[test]
    fn temp_bindings_persist_and_can_be_mutated() {
        let mut repl = ReplSession::new().expect("repl");

        assert!(matches!(
            repl.submit_xml(r#"<temp name="x" type="int">1</temp>"#)
                .expect("temp should run"),
            SubmissionResult::Executed(_)
        ));
        assert!(matches!(
            repl.submit_xml("<code>x = x + 2;</code>")
                .expect("code should run"),
            SubmissionResult::Executed(_)
        ));

        let result = repl
            .submit_xml("<text>${x}</text>")
            .expect("text should run");
        assert_eq!(text_events(&result), vec!["3".to_string()]);
        assert_eq!(repl.inspect(InspectTarget::Bindings), "temp x: int = 3");
    }

    #[test]
    fn choice_suspends_and_choose_resumes_to_prompt() {
        let mut repl = ReplSession::new().expect("repl");

        let state = execution_state(
            repl.submit_xml(
                r#"
                <choice text="pick">
                  <option text="left">
                    <text>L</text>
                  </option>
                  <option text="right">
                    <text>R</text>
                  </option>
                </choice>
                "#,
            )
            .expect("choice should suspend"),
        );
        assert_eq!(
            state,
            ExecutionState::SuspendedChoice {
                prompt: Some("pick".to_string()),
                items: vec!["left".to_string(), "right".to_string()],
            }
        );
        let resumed = repl.choose(1).expect("choose should work");
        assert_eq!(
            resumed.events,
            vec![StepEvent::Text {
                text: "R".to_string(),
                tag: None,
            }]
        );
        assert_eq!(resumed.state, ExecutionState::Ready);
    }

    #[test]
    fn end_exits_the_repl() {
        let mut repl = ReplSession::new().expect("repl");

        let result = repl.submit_xml("<end/>").expect("end should run");

        assert_eq!(execution_state(result), ExecutionState::Exited);
        assert!(repl.is_exited());
    }

    #[test]
    fn goto_into_loaded_script_exits_after_target_end() {
        let dir = TestDir::new("goto");
        let file = dir.write(
            "helper.xml",
            r#"
            <module name="helper">
              <script name="target">
                <text>from target</text>
                <end/>
              </script>
            </module>
            "#,
        );
        let mut repl = ReplSession::new().expect("repl");
        let load = repl.load_path(&file).expect("file load should work");
        assert_eq!(
            load,
            LoadResult {
                modules: vec!["helper".to_string()]
            }
        );

        let result = repl
            .submit_xml(r#"<goto script="@helper.target"/>"#)
            .expect("goto should run");

        assert_eq!(text_events(&result), vec!["from target".to_string()]);
        assert_eq!(execution_state(result), ExecutionState::Exited);
        assert!(repl.is_exited());
    }

    #[test]
    fn kernel_if_macro_works_at_top_level() {
        let mut repl = ReplSession::new().expect("repl");

        let result = repl
            .submit_xml(
                r#"
                <if when="true">
                  <text>inside</text>
                </if>
                "#,
            )
            .expect("if should run");

        assert_eq!(text_events(&result), vec!["inside".to_string()]);
    }

    #[test]
    fn require_enables_repl_defined_macro() {
        let mut repl = ReplSession::new().expect("repl");

        assert!(matches!(
            repl.submit_xml(
                r#"
                <module name="helper">
                  <macro name="mk">
                    <quote>
                      <text>hello</text>
                    </quote>
                  </macro>
                </module>
                "#,
            )
            .expect("module should define"),
            SubmissionResult::ModuleUpdated { .. }
        ));
        assert!(matches!(
            repl.submit_xml(r#"<require name="helper"/>"#)
                .expect("require should work"),
            SubmissionResult::ContextUpdated
        ));

        let result = repl.submit_xml("<mk/>").expect("macro should run");
        assert_eq!(text_events(&result), vec!["hello".to_string()]);
    }

    #[test]
    fn repl_defined_module_replaces_same_named_module() {
        let mut repl = ReplSession::new().expect("repl");

        repl.submit_xml(
            r#"
            <module name="helper">
              <macro name="mk">
                <quote><text>old</text></quote>
              </macro>
            </module>
            "#,
        )
        .expect("module should define");
        repl.submit_xml(r#"<require name="helper"/>"#)
            .expect("require should work");
        let first = repl.submit_xml("<mk/>").expect("first macro should run");
        assert_eq!(text_events(&first), vec!["old".to_string()]);

        repl.submit_xml(
            r#"
            <module name="helper">
              <macro name="mk">
                <quote><text>new</text></quote>
              </macro>
            </module>
            "#,
        )
        .expect("replacement module should define");
        let second = repl.submit_xml("<mk/>").expect("second macro should run");
        assert_eq!(text_events(&second), vec!["new".to_string()]);
    }

    #[test]
    fn load_directory_and_imported_var_work_without_autorun() {
        let dir = TestDir::new("dir-load");
        dir.write(
            "a.xml",
            r#"
            <module name="main">
              <script name="main">
                <text>should not autorun</text>
                <end/>
              </script>
            </module>
            "#,
        );
        dir.write(
            "b.xml",
            r#"
            <module name="helper">
              <var name="value" type="int">7</var>
            </module>
            "#,
        );
        let mut repl = ReplSession::new().expect("repl");
        let load = repl.load_path(&dir.path).expect("dir load should work");
        assert_eq!(
            load,
            LoadResult {
                modules: vec!["main".to_string(), "helper".to_string()]
            }
        );
        assert_eq!(repl.inspect(InspectTarget::Bindings), "no bindings");

        repl.submit_xml(r#"<import name="helper"/>"#)
            .expect("import should work");
        let result = repl
            .submit_xml("<text>${value}</text>")
            .expect("text should run");
        assert_eq!(text_events(&result), vec!["7".to_string()]);
    }

    #[test]
    fn runtime_failures_do_not_mutate_persisted_state() {
        let mut repl = ReplSession::new().expect("repl");

        repl.submit_xml(r#"<temp name="x" type="int">1</temp>"#)
            .expect("temp should run");
        let error = repl
            .submit_xml("<code>x = missing;</code>")
            .expect_err("runtime error should fail");
        assert!(error.to_string().contains("rhai eval error"));

        let result = repl
            .submit_xml("<text>${x}</text>")
            .expect("temp should still persist");
        assert_eq!(text_events(&result), vec!["1".to_string()]);
    }

    #[test]
    fn repl_defined_module_can_define_script_and_function_and_run_them() {
        let mut repl = ReplSession::new().expect("repl");

        assert!(matches!(
            repl.submit_xml(
                r#"
                <module name="helper">
                  <function name="pick" args="" return_type="string">return "picked";</function>
                  <script name="main">
                    <text>${invoke(#pick, [])}</text>
                    <end/>
                  </script>
                </module>
                "#,
            )
            .expect("module with script/function should define"),
            SubmissionResult::ModuleUpdated { .. }
        ));

        let result = repl
            .submit_xml(r#"<goto script="@helper.main"/>"#)
            .expect("goto should run repl-defined script");
        assert_eq!(text_events(&result), vec!["picked".to_string()]);
        assert_eq!(execution_state(result), ExecutionState::Exited);
    }
}
