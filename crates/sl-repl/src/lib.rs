use std::collections::BTreeMap;

use sl_api::parse_modules_from_sources;
use sl_compiler::{
    CompilePipeline, DeclaredType, SemanticChoiceOption, SemanticModule, SemanticProgram,
    SemanticScript, SemanticStmt, compile_pipeline,
};
use sl_core::{
    CompiledArtifact, CompiledText, CompiledTextPart, Form, FormField, FormItem, FormValue,
    Instruction, ScriptLangError, Snapshot, StepEvent, StepResult, Suspension, TextSegment,
};
use sl_runtime::Engine;

pub struct ReplSession {
    forms: Vec<Form>,
    semantic_program: SemanticProgram,
    artifact: CompiledArtifact,
    engine: Engine,
}

impl ReplSession {
    pub fn load_from_xml_map(sources: &BTreeMap<String, String>) -> Result<Self, ScriptLangError> {
        let forms = parse_modules_from_sources(sources)?;
        let CompilePipeline {
            semantic_program,
            artifact,
        } = compile_pipeline(&forms)?;
        let engine = Engine::new(artifact.clone());
        Ok(Self {
            forms,
            semantic_program,
            artifact,
            engine,
        })
    }

    pub fn forms(&self) -> &[Form] {
        &self.forms
    }

    pub fn semantic_program(&self) -> &SemanticProgram {
        &self.semantic_program
    }

    pub fn artifact(&self) -> &CompiledArtifact {
        &self.artifact
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn eval_command(&mut self, input: &str) -> Result<String, ScriptLangError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ScriptLangError::message("empty repl command"));
        }

        let mut parts = trimmed.split_whitespace();
        let command = parts.next().expect("command should exist");
        match command {
            ":help" => Ok(help_text()),
            ":ast" => Ok(format_forms(&self.forms)),
            ":semantic" => Ok(format_semantic_program(&self.semantic_program)),
            ":ir" => Ok(format_artifact(&self.artifact)),
            ":runtime" => Ok(format_runtime_snapshot(
                &self.artifact,
                &self.engine.snapshot(),
            )),
            ":start" => {
                self.engine.start(parts.next())?;
                Ok(format_runtime_snapshot(
                    &self.artifact,
                    &self.engine.snapshot(),
                ))
            }
            ":reset" => {
                self.engine = Engine::new(self.artifact.clone());
                Ok(format_runtime_snapshot(
                    &self.artifact,
                    &self.engine.snapshot(),
                ))
            }
            ":step" => {
                let result = self.engine.step()?;
                Ok(format_step_result(
                    &self.artifact,
                    &result,
                    &self.engine.snapshot(),
                ))
            }
            ":run" => Ok(self.run_until_pause_or_end()?),
            ":choose" => {
                let raw_index = parts
                    .next()
                    .ok_or_else(|| ScriptLangError::message("`:choose` requires an index"))?;
                let index = raw_index.parse::<usize>().map_err(|_| {
                    ScriptLangError::message(format!("invalid choice index `{raw_index}`"))
                })?;
                self.engine.choose(index)?;
                Ok(format_runtime_snapshot(
                    &self.artifact,
                    &self.engine.snapshot(),
                ))
            }
            other => Err(ScriptLangError::message(format!(
                "unknown repl command `{other}`"
            ))),
        }
    }

    fn run_until_pause_or_end(&mut self) -> Result<String, ScriptLangError> {
        let mut lines = Vec::new();
        loop {
            match self.engine.step()? {
                StepResult::Progress => {}
                StepResult::Event(StepEvent::Text { text, tag }) => match tag {
                    Some(tag) => lines.push(format!("text[{tag}]: {text:?}")),
                    None => lines.push(format!("text: {text:?}")),
                },
                StepResult::Suspended(Suspension::Choice { prompt, items }) => {
                    lines.push(format!(
                        "suspended: choice prompt={} items={:?}",
                        format_option_string(prompt.as_deref()),
                        items
                    ));
                    break;
                }
                StepResult::Completed(completion) => {
                    lines.push(format!("completed: {completion:?}"));
                    break;
                }
            }
        }
        lines.push(String::new());
        lines.push(format_runtime_snapshot(
            &self.artifact,
            &self.engine.snapshot(),
        ));
        Ok(lines.join("\n"))
    }
}

fn help_text() -> String {
    [
        ":help",
        ":ast",
        ":semantic",
        ":ir",
        ":runtime",
        ":start [script_ref]",
        ":reset",
        ":step",
        ":run",
        ":choose INDEX",
    ]
    .join("\n")
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
            format_declared_type(&var.declared_type),
            var.expr
        ));
    }
    for function in &module.functions {
        lines.push(format!(
            "  function {}({}) -> {}",
            function.name,
            function.param_names.join(", "),
            format_declared_type(&function.return_type)
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
            format_declared_type(declared_type),
            expr
        )),
        SemanticStmt::Code { code } => lines.push(format!("{padding}code {:?}", code)),
        SemanticStmt::Text { template, tag } => {
            lines.push(format!(
                "{padding}text {} tag={}",
                format_text_template(template),
                format_option_string(tag.as_deref())
            ));
        }
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

fn format_declared_type(declared_type: &DeclaredType) -> &'static str {
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
        Instruction::End => "End".to_string(),
    }
}

fn format_runtime_snapshot(artifact: &CompiledArtifact, snapshot: &Snapshot) -> String {
    let script_name = format_script_ref(artifact, snapshot.script_id);
    let current_instruction = artifact
        .scripts
        .get(snapshot.script_id)
        .and_then(|script| script.instructions.get(snapshot.pc))
        .map(format_instruction)
        .unwrap_or_else(|| "none".to_string());
    let globals = artifact
        .globals
        .iter()
        .zip(snapshot.globals.iter())
        .map(|(global, value)| format!("{}={}", global.runtime_name, value))
        .collect::<Vec<_>>()
        .join(", ");
    let locals = artifact
        .scripts
        .get(snapshot.script_id)
        .map(|script| {
            script
                .local_names
                .iter()
                .zip(snapshot.locals.iter())
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let pending = match &snapshot.pending {
        Some(pending) => format!(
            "prompt={} options={:?}",
            format_option_string(pending.prompt.as_deref()),
            pending
                .options
                .iter()
                .map(|option| option.text.clone())
                .collect::<Vec<_>>()
        ),
        None => "none".to_string(),
    };

    [
        format!("script {} ({script_name})", snapshot.script_id),
        format!("pc {}", snapshot.pc),
        format!("started {}", snapshot.started),
        format!("halted {}", snapshot.halted),
        format!(
            "entry_override {}",
            snapshot
                .entry_override
                .map(|id| format_script_ref(artifact, id))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("current_instruction {current_instruction}"),
        format!("current_condition {:?}", snapshot.current_condition),
        format!("globals [{globals}]"),
        format!("locals [{locals}]"),
        format!("pending {pending}"),
    ]
    .join("\n")
}

fn format_step_result(
    artifact: &CompiledArtifact,
    result: &StepResult,
    snapshot: &Snapshot,
) -> String {
    let mut lines = vec![match result {
        StepResult::Progress => "result Progress".to_string(),
        StepResult::Event(StepEvent::Text { text, tag }) => {
            format!(
                "result Event::Text text={text:?} tag={}",
                format_option_string(tag.as_deref())
            )
        }
        StepResult::Suspended(Suspension::Choice { prompt, items }) => format!(
            "result Suspended::Choice prompt={} items={items:?}",
            format_option_string(prompt.as_deref())
        ),
        StepResult::Completed(completion) => format!("result Completed::{completion:?}"),
    }];
    lines.push(String::new());
    lines.push(format_runtime_snapshot(artifact, snapshot));
    lines.join("\n")
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
    use std::collections::BTreeMap;

    use super::ReplSession;

    #[test]
    fn ast_and_semantic_views_show_macro_expansion_boundary() {
        let sources = BTreeMap::from([(
            "main.xml".to_string(),
            r#"
            <module name="main">
              <script name="main">
                <if when="true">
                  <text>hello</text>
                </if>
                <end/>
              </script>
            </module>
            "#
            .to_string(),
        )]);
        let mut session = ReplSession::load_from_xml_map(&sources).expect("session");

        let ast = session.eval_command(":ast").expect("ast");
        let semantic = session.eval_command(":semantic").expect("semantic");

        assert!(ast.contains("<if when=\"true\">"));
        assert!(semantic.contains("skip_loop_control_capture=true"));
        assert!(semantic.contains("text lit(\"hello\")"));
        assert!(!semantic.contains("  if "));
    }

    #[test]
    fn ir_and_runtime_commands_show_execution_progress() {
        let sources = BTreeMap::from([(
            "main.xml".to_string(),
            r#"
            <module name="main">
              <script name="main">
                <text>hello</text>
                <end/>
              </script>
            </module>
            "#
            .to_string(),
        )]);
        let mut session = ReplSession::load_from_xml_map(&sources).expect("session");

        let ir = session.eval_command(":ir").expect("ir");
        assert!(ir.contains("script 0 ref=main.main"));
        assert!(ir.contains("EmitText"));

        let run = session.eval_command(":run").expect("run");
        assert!(run.contains("text: \"hello\""));
        assert!(run.contains("completed: End"));
        assert!(run.contains("halted true"));
    }
}
