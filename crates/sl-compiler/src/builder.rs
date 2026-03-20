use std::collections::{BTreeMap, HashMap};

use sl_core::{
    CompiledArtifact, CompiledScript, Form, GlobalVar, Instruction, LocalId, ScriptId,
    ScriptLangError,
};

use crate::form::{child_forms, error_at, required_attr, trimmed_text_items};
use crate::lower::lower_script;

pub fn compile_artifact(forms: &[Form]) -> Result<CompiledArtifact, ScriptLangError> {
    compile_modules(forms)
}

pub(crate) fn compile_modules(forms: &[Form]) -> Result<CompiledArtifact, ScriptLangError> {
    let mut builder = ArtifactBuilder {
        scripts: Vec::new(),
        script_refs: BTreeMap::new(),
        globals: Vec::new(),
        default_entry_script_id: None,
    };

    builder.collect_declarations(forms)?;
    builder.lower_modules(forms)?;

    let default_entry_script_id = builder
        .default_entry_script_id
        .ok_or_else(|| ScriptLangError::message("no <script> declarations found"))?;

    let boot_script_id = builder.scripts.len();
    let boot_script = builder.build_boot_script(default_entry_script_id);
    let mut scripts = builder
        .scripts
        .into_iter()
        .enumerate()
        .map(|(script_id, draft)| CompiledScript {
            script_id,
            script_ref: draft.script_ref,
            local_names: draft.local_names,
            instructions: draft.instructions,
        })
        .collect::<Vec<_>>();
    scripts.push(CompiledScript {
        script_id: boot_script_id,
        script_ref: "__boot__".to_string(),
        local_names: Vec::new(),
        instructions: boot_script,
    });

    Ok(CompiledArtifact {
        default_entry_script_id,
        boot_script_id,
        script_refs: builder.script_refs,
        scripts,
        globals: builder.globals,
    })
}

pub(crate) struct ArtifactBuilder {
    pub(crate) scripts: Vec<ScriptDraft>,
    pub(crate) script_refs: BTreeMap<String, ScriptId>,
    pub(crate) globals: Vec<GlobalVar>,
    pub(crate) default_entry_script_id: Option<ScriptId>,
}

#[derive(Clone)]
pub(crate) struct ScriptDraft {
    pub(crate) script_ref: String,
    pub(crate) module_name: String,
    pub(crate) local_names: Vec<String>,
    pub(crate) local_lookup: HashMap<String, LocalId>,
    pub(crate) instructions: Vec<Instruction>,
}

impl ArtifactBuilder {
    fn collect_declarations(&mut self, modules: &[Form]) -> Result<(), ScriptLangError> {
        let mut global_short_names = HashMap::<String, String>::new();

        for module in modules {
            let module_name = required_attr(module, "name")?;
            for child in child_forms(module)? {
                match child.head.as_str() {
                    "var" => {
                        let var_name = required_attr(child, "name")?;
                        let qualified_name = format!("{module_name}.{var_name}");
                        if let Some(existing) =
                            global_short_names.insert(var_name.to_string(), qualified_name.clone())
                        {
                            return Err(ScriptLangError::message(format!(
                                "global short name `{}` is ambiguous between `{existing}` and `{qualified_name}`",
                                var_name
                            )));
                        }
                        self.globals.push(GlobalVar {
                            global_id: self.globals.len(),
                            qualified_name,
                            short_name: var_name.to_string(),
                            initializer: trimmed_text_items(child)?,
                        });
                    }
                    "script" => {
                        let script_name = required_attr(child, "name")?;
                        let script_ref = format!("{module_name}.{script_name}");
                        if self.script_refs.contains_key(&script_ref) {
                            return Err(ScriptLangError::message(format!(
                                "duplicate script declaration `{script_ref}`"
                            )));
                        }
                        let script_id = self.scripts.len();
                        self.script_refs.insert(script_ref.clone(), script_id);
                        if self.default_entry_script_id.is_none() {
                            self.default_entry_script_id = Some(script_id);
                        }
                        self.scripts.push(ScriptDraft {
                            script_ref,
                            module_name: module_name.to_string(),
                            local_names: Vec::new(),
                            local_lookup: HashMap::new(),
                            instructions: Vec::new(),
                        });
                    }
                    other => {
                        return Err(error_at(
                            child,
                            format!("unsupported <module> child <{other}> in MVP"),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    fn lower_modules(&mut self, modules: &[Form]) -> Result<(), ScriptLangError> {
        let mut script_index = 0;
        for module in modules {
            let module_name = required_attr(module, "name")?.to_string();
            for child in child_forms(module)? {
                if child.head != "script" {
                    continue;
                }
                lower_script(self, script_index, &module_name, child)?;
                script_index += 1;
            }
        }
        Ok(())
    }

    fn build_boot_script(&self, default_entry_script_id: ScriptId) -> Vec<Instruction> {
        let mut instructions = Vec::with_capacity(self.globals.len() + 2);
        for global in &self.globals {
            instructions.push(Instruction::EvalGlobalInit {
                global_id: global.global_id,
                expr: global.initializer.clone(),
            });
        }
        instructions.push(Instruction::JumpScript {
            target_script_id: default_entry_script_id,
        });
        instructions
    }
}

#[cfg(test)]
mod tests {
    use super::{ArtifactBuilder, Instruction, compile_artifact, compile_modules};
    use sl_core::{
        Form, FormField, FormItem, FormMeta, FormValue, SourcePosition, TextSegment, TextTemplate,
    };

    fn meta() -> FormMeta {
        FormMeta {
            source_name: Some("main.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition {
                row: 1,
                column: 100,
            },
            start_byte: 0,
            end_byte: 100,
        }
    }

    fn form(head: &str, fields: Vec<FormField>) -> Form {
        Form {
            head: head.to_string(),
            meta: meta(),
            fields,
        }
    }

    fn attr(name: &str, value: &str) -> FormField {
        FormField {
            name: name.to_string(),
            value: FormValue::String(value.to_string()),
        }
    }

    fn children(items: Vec<FormItem>) -> FormField {
        FormField {
            name: "children".to_string(),
            value: FormValue::Sequence(items),
        }
    }

    fn node(head: &str, attrs: Vec<(&str, &str)>, items: Vec<FormItem>) -> Form {
        let mut fields = attrs
            .into_iter()
            .map(|(k, v)| attr(k, v))
            .collect::<Vec<_>>();
        fields.push(children(items));
        form(head, fields)
    }

    fn text(text: &str) -> FormItem {
        FormItem::Text(text.to_string())
    }

    fn child(form: Form) -> FormItem {
        FormItem::Form(form)
    }

    fn module(name: &str, vars: Vec<(&str, &str)>, scripts: Vec<(&str, Vec<Form>)>) -> Form {
        let mut items = vars
            .into_iter()
            .map(|(var_name, expr)| child(node("var", vec![("name", var_name)], vec![text(expr)])))
            .collect::<Vec<_>>();
        items.extend(scripts.into_iter().map(|(script_name, body)| {
            child(node(
                "script",
                vec![("name", script_name)],
                body.into_iter().map(child).collect(),
            ))
        }));
        node("module", vec![("name", name)], items)
    }

    fn stmt(head: &str, attrs: Vec<(&str, &str)>, items: Vec<FormItem>) -> Form {
        node(head, attrs, items)
    }

    fn text_template() -> TextTemplate {
        TextTemplate {
            segments: vec![
                TextSegment::Literal("hello ".to_string()),
                TextSegment::Expr("name".to_string()),
            ],
        }
    }

    #[test]
    fn compile_artifact_requires_at_least_one_script() {
        let error = compile_artifact(&[module("main", vec![], vec![])]).expect_err("should fail");
        assert_eq!(error.to_string(), "no <script> declarations found");
    }

    #[test]
    fn collect_declarations_rejects_ambiguous_global_short_names() {
        let error = compile_modules(&[
            module(
                "a",
                vec![("name", "1")],
                vec![("entry", vec![stmt("end", vec![], vec![])])],
            ),
            module(
                "b",
                vec![("name", "2")],
                vec![("entry", vec![stmt("end", vec![], vec![])])],
            ),
        ])
        .expect_err("should fail");

        assert_eq!(
            error.to_string(),
            "global short name `name` is ambiguous between `a.name` and `b.name`"
        );
    }

    #[test]
    fn collect_declarations_rejects_duplicate_script_refs() {
        let error = compile_modules(&[module(
            "main",
            vec![],
            vec![
                ("entry", vec![stmt("end", vec![], vec![])]),
                ("entry", vec![stmt("end", vec![], vec![])]),
            ],
        )])
        .expect_err("should fail");

        assert_eq!(
            error.to_string(),
            "duplicate script declaration `main.entry`"
        );
    }

    #[test]
    fn compile_artifact_sets_default_entry_and_boot_script_order() {
        let artifact = compile_artifact(&[module(
            "main",
            vec![("answer", "40 + 2")],
            vec![
                ("entry", vec![stmt("end", vec![], vec![])]),
                ("other", vec![stmt("end", vec![], vec![])]),
            ],
        )])
        .expect("artifact should compile");

        assert_eq!(artifact.default_entry_script_id, 0);
        assert_eq!(artifact.boot_script_id, 2);
        assert_eq!(artifact.script_refs["main.entry"], 0);
        assert_eq!(artifact.script_refs["main.other"], 1);
        assert_eq!(artifact.scripts[2].script_ref, "__boot__");
        assert!(matches!(
            &artifact.scripts[2].instructions[..],
            [
                Instruction::EvalGlobalInit { global_id, expr },
                Instruction::JumpScript { target_script_id }
            ] if *global_id == 0 && expr == "40 + 2" && *target_script_id == 0
        ));
    }

    #[test]
    fn compile_artifact_lowers_statements_into_expected_instructions() {
        let artifact = compile_artifact(&[
            module(
                "main",
                vec![("shared", "10")],
                vec![
                    (
                        "entry",
                        vec![
                            stmt("temp", vec![("name", "x")], vec![text("1")]),
                            stmt("temp", vec![("name", "x")], vec![text("2")]),
                            stmt("code", vec![], vec![text("x += 1;")]),
                            stmt("text", vec![("tag", "line")], vec![text("hello ${name}")]),
                            stmt(
                                "if",
                                vec![("when", "x > 0")],
                                vec![child(stmt("text", vec![], vec![text("inside")]))],
                            ),
                            stmt(
                                "choice",
                                vec![("text", "pick")],
                                vec![
                                    child(stmt(
                                        "option",
                                        vec![("text", "left")],
                                        vec![child(stmt(
                                            "goto",
                                            vec![("script", "target")],
                                            vec![],
                                        ))],
                                    )),
                                    child(stmt(
                                        "option",
                                        vec![("text", "right")],
                                        vec![child(stmt("end", vec![], vec![]))],
                                    )),
                                ],
                            ),
                        ],
                    ),
                    ("target", vec![stmt("end", vec![], vec![])]),
                    (
                        "jump_end",
                        vec![stmt("goto", vec![("script", "main.target")], vec![])],
                    ),
                ],
            ),
            module(
                "other",
                vec![],
                vec![("remote", vec![stmt("end", vec![], vec![])])],
            ),
        ])
        .expect("artifact should compile");

        let instructions = &artifact.scripts[0].instructions;
        assert!(matches!(
            &instructions[0],
            Instruction::EvalTemp { local_id, expr } if *local_id == 0 && expr == "1"
        ));
        assert!(matches!(
            &instructions[1],
            Instruction::EvalTemp { local_id, expr } if *local_id == 0 && expr == "2"
        ));
        assert!(matches!(
            &instructions[2],
            Instruction::ExecCode { code } if code == "x += 1;"
        ));
        assert!(matches!(
            &instructions[3],
            Instruction::EmitText { tag, .. } if tag.as_deref() == Some("line")
        ));
        assert!(matches!(
            &instructions[4],
            Instruction::EvalCond { expr } if expr == "x > 0"
        ));
        assert!(matches!(
            &instructions[5],
            Instruction::JumpIfFalse { target_pc } if *target_pc > 5
        ));
        assert!(matches!(&instructions[6], Instruction::EmitText { .. }));
        assert!(matches!(
            &instructions[7],
            Instruction::BuildChoice { prompt, options }
                if prompt.is_some() && options.len() == 2
        ));
        assert!(matches!(
            &instructions[8],
            Instruction::JumpScript { target_script_id } if *target_script_id == artifact.script_refs["main.target"]
        ));
        assert!(matches!(
            &instructions[9],
            Instruction::Jump { target_pc } if *target_pc == 12
        ));
        assert!(matches!(&instructions[10], Instruction::End));
        assert!(matches!(
            &instructions[11],
            Instruction::Jump { target_pc } if *target_pc == 12
        ));
        assert!(matches!(&instructions[12], Instruction::End));
        assert_eq!(artifact.scripts[0].local_names, vec!["x".to_string()]);
        assert!(matches!(
            artifact.scripts[2].instructions.as_slice(),
            [Instruction::JumpScript { .. }]
        ));
        assert!(matches!(
            artifact.scripts[1].instructions.as_slice(),
            [Instruction::End]
        ));
    }

    #[test]
    fn compile_artifact_resolves_goto_variants_and_rejects_unknown_targets() {
        let artifact = compile_artifact(&[
            module(
                "main",
                vec![],
                vec![
                    (
                        "entry",
                        vec![
                            stmt("goto", vec![("script", "next")], vec![]),
                            stmt("goto", vec![("script", "@next")], vec![]),
                            stmt("goto", vec![("script", "other.remote")], vec![]),
                        ],
                    ),
                    ("next", vec![stmt("end", vec![], vec![])]),
                ],
            ),
            module(
                "other",
                vec![],
                vec![("remote", vec![stmt("end", vec![], vec![])])],
            ),
        ])
        .expect("artifact should compile");
        let target_next = artifact.script_refs["main.next"];
        let target_remote = artifact.script_refs["other.remote"];

        assert!(matches!(
            &artifact.scripts[0].instructions[..],
            [
                Instruction::JumpScript { target_script_id: first },
                Instruction::JumpScript { target_script_id: second },
                Instruction::JumpScript { target_script_id: third },
            ] if *first == target_next && *second == target_next && *third == target_remote
        ));

        let error = compile_artifact(&[module(
            "main",
            vec![],
            vec![(
                "entry",
                vec![stmt("goto", vec![("script", "missing")], vec![])],
            )],
        )])
        .expect_err("should fail");

        assert!(
            error
                .to_string()
                .contains("script `main.missing` referenced by <goto> does not exist")
        );
    }

    #[test]
    fn compile_artifact_rejects_invalid_mvp_structure_from_forms() {
        let unsupported = compile_artifact(&[module(
            "main",
            vec![],
            vec![("entry", vec![stmt("while", vec![], vec![])])],
        )])
        .expect_err("should fail");
        assert!(
            unsupported
                .to_string()
                .contains("unsupported statement <while> in MVP")
        );

        let missing_attr = compile_artifact(&[node(
            "module",
            vec![],
            vec![child(node(
                "script",
                vec![],
                vec![child(stmt("end", vec![], vec![]))],
            ))],
        )])
        .expect_err("should fail");
        assert!(
            missing_attr
                .to_string()
                .contains("<module> requires `name`")
        );

        let bad_choice = compile_artifact(&[module(
            "main",
            vec![],
            vec![(
                "entry",
                vec![stmt(
                    "choice",
                    vec![],
                    vec![child(stmt("text", vec![], vec![text("bad")]))],
                )],
            )],
        )])
        .expect_err("should fail");
        assert!(
            bad_choice
                .to_string()
                .contains("<choice> only supports <option> children in MVP")
        );
    }

    #[test]
    fn build_boot_script_is_empty_except_for_jump_when_no_globals_exist() {
        let builder = ArtifactBuilder {
            scripts: Vec::new(),
            script_refs: Default::default(),
            globals: Vec::new(),
            default_entry_script_id: Some(0),
        };

        assert!(matches!(
            builder.build_boot_script(3).as_slice(),
            [Instruction::JumpScript { target_script_id }] if *target_script_id == 3
        ));
    }

    #[test]
    fn helper_text_template_fixture_is_well_formed() {
        assert!(matches!(
            text_template().segments.as_slice(),
            [TextSegment::Literal(text), TextSegment::Expr(expr)]
                if text == "hello " && expr == "name"
        ));
    }
}
