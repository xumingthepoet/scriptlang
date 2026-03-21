use sl_core::ScriptLangError;

use crate::names::{function_literal_key, script_literal_key};
use crate::semantic::env::{ModuleExports, ModuleState, ProgramState};

pub(crate) const DEFAULT_KERNEL_MODULE: &str = "kernel";

pub(crate) struct ModuleCatalog<'a> {
    program: &'a ProgramState,
}

impl<'a> ModuleCatalog<'a> {
    pub(crate) fn build(program: &'a ProgramState) -> Result<Self, ScriptLangError> {
        for module_name in &program.module_order {
            if !program.modules.contains_key(module_name) {
                return Err(ScriptLangError::message(format!(
                    "module `{module_name}` missing expand-time state"
                )));
            }
        }
        Ok(Self { program })
    }

    pub(crate) fn contains(&self, module_name: &str) -> bool {
        self.program.modules.contains_key(module_name)
    }

    pub(crate) fn exports(&self, module_name: &str) -> Result<&ModuleExports, ScriptLangError> {
        Ok(&self.module_state(module_name)?.exports)
    }

    pub(crate) fn resolve_script_literal(
        &self,
        current_module: &str,
        raw: &str,
    ) -> Result<String, ScriptLangError> {
        let qualified = script_literal_key(raw, current_module)
            .ok_or_else(|| ScriptLangError::message(format!("invalid script literal `{raw}`")))?;
        let (module_name, script_name) = qualified
            .rsplit_once('.')
            .expect("qualified script literal must contain module separator");
        if !self.contains(module_name)
            || !self
                .exports(module_name)?
                .scripts
                .contains_declared(script_name)
        {
            return Err(ScriptLangError::message(format!(
                "unknown script `{qualified}`"
            )));
        }
        Ok(qualified)
    }

    pub(crate) fn resolve_function_literal(
        &self,
        current_module: &str,
        raw: &str,
    ) -> Result<String, ScriptLangError> {
        let qualified = function_literal_key(raw, current_module)
            .ok_or_else(|| ScriptLangError::message(format!("invalid function literal `{raw}`")))?;
        let (module_name, function_name) = qualified
            .rsplit_once('.')
            .expect("qualified function literal must contain module separator");
        if !self.contains(module_name)
            || !self
                .exports(module_name)?
                .functions
                .contains_declared(function_name)
        {
            return Err(ScriptLangError::message(format!(
                "unknown function `{qualified}`"
            )));
        }
        Ok(qualified)
    }

    pub(crate) fn module_state(
        &self,
        module_name: &str,
    ) -> Result<&'a ModuleState, ScriptLangError> {
        self.program.modules.get(module_name).ok_or_else(|| {
            ScriptLangError::message(format!("module `{module_name}` does not exist"))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use super::ModuleCatalog;
    use crate::semantic::env::{ModuleExports, ModuleState, ProgramState};

    fn meta() -> FormMeta {
        FormMeta {
            source_name: Some("main.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 20 },
            start_byte: 0,
            end_byte: 20,
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

    fn text(value: &str) -> FormItem {
        FormItem::Text(value.to_string())
    }

    fn const_form(name: &str, value: &str) -> Form {
        form(
            "const",
            vec![
                attr("name", name),
                attr("type", "int"),
                children(vec![text(value)]),
            ],
        )
    }

    fn module_state_with(
        module_name: &str,
        exports: ModuleExports,
        children: Vec<Form>,
    ) -> ModuleState {
        ModuleState {
            module_name: Some(module_name.to_string()),
            imports: Vec::new(),
            requires: Vec::new(),
            aliases: BTreeMap::new(),
            child_aliases: BTreeMap::new(),
            const_decls: BTreeMap::new(),
            exports,
            children,
            locals: Default::default(),
        }
    }

    fn exports_with(
        consts: &[(&str, bool)],
        functions: &[(&str, bool)],
        vars: &[(&str, bool)],
        scripts: &[(&str, bool)],
    ) -> ModuleExports {
        let mut result = ModuleExports::default();
        for (name, exported) in consts {
            result.consts.insert((*name).to_string(), *exported);
        }
        for (name, exported) in functions {
            result.functions.insert((*name).to_string(), *exported);
        }
        for (name, exported) in vars {
            result.vars.insert((*name).to_string(), *exported);
        }
        for (name, exported) in scripts {
            result.scripts.insert((*name).to_string(), *exported);
        }
        result
    }

    fn program_state() -> ProgramState {
        ProgramState {
            modules: BTreeMap::from([
                (
                    "kernel".to_string(),
                    module_state_with(
                        "kernel",
                        exports_with(&[("zero", true)], &[], &[], &[]),
                        vec![const_form("zero", "0")],
                    ),
                ),
                (
                    "helper".to_string(),
                    module_state_with(
                        "helper",
                        exports_with(
                            &[("answer", true), ("hidden", false)],
                            &[("pick", true)],
                            &[("value", true), ("priv", false)],
                            &[("entry", true)],
                        ),
                        vec![const_form("answer", "42"), const_form("hidden", "7")],
                    ),
                ),
                (
                    "main".to_string(),
                    module_state_with(
                        "main",
                        exports_with(
                            &[("local", true)],
                            &[("choose", true)],
                            &[("value", true)],
                            &[("main", true)],
                        ),
                        vec![const_form("local", "1")],
                    ),
                ),
            ]),
            module_order: vec![
                "kernel".to_string(),
                "helper".to_string(),
                "main".to_string(),
            ],
            module_macros: BTreeMap::new(),
        }
    }

    #[test]
    fn module_catalog_build_and_script_lookup_cover_basic_paths() {
        let program = program_state();
        let catalog = ModuleCatalog::build(&program).expect("catalog");

        assert!(catalog.contains("kernel"));
        assert_eq!(
            catalog
                .resolve_script_literal("main", "@main")
                .expect("script"),
            "main.main"
        );
        assert_eq!(
            catalog
                .resolve_script_literal("main", "@helper.entry")
                .expect("qualified"),
            "helper.entry"
        );
        assert_eq!(
            catalog
                .resolve_function_literal("main", "#choose")
                .expect("function"),
            "main.choose"
        );
        assert_eq!(
            catalog
                .resolve_function_literal("main", "#helper.pick")
                .expect("qualified function"),
            "helper.pick"
        );
    }

    #[test]
    fn module_catalog_reports_missing_state_and_unknown_script() {
        let mut broken = program_state();
        broken.module_order.push("missing".to_string());
        let build_error = ModuleCatalog::build(&broken).err().expect("missing module");
        assert!(
            build_error
                .to_string()
                .contains("missing expand-time state")
        );

        let program = program_state();
        let catalog = ModuleCatalog::build(&program).expect("catalog");
        let script_error = catalog
            .resolve_script_literal("main", "@helper.nope")
            .expect_err("unknown script");
        assert!(script_error.to_string().contains("unknown script"));
        let function_error = catalog
            .resolve_function_literal("main", "#helper.nope")
            .expect_err("unknown function");
        assert!(function_error.to_string().contains("unknown function"));
    }

    #[test]
    fn module_catalog_rejects_missing_module_and_invalid_literal_shapes() {
        let program = program_state();
        let catalog = ModuleCatalog::build(&program).expect("catalog");

        let missing = catalog.module_state("missing").expect_err("missing module");
        assert!(missing.to_string().contains("does not exist"));

        let invalid_script = catalog
            .resolve_script_literal("main", "@")
            .expect_err("invalid script literal");
        assert!(
            invalid_script
                .to_string()
                .contains("invalid script literal")
        );

        let invalid_function = catalog
            .resolve_function_literal("main", "#")
            .expect_err("invalid function literal");
        assert!(
            invalid_function
                .to_string()
                .contains("invalid function literal")
        );
    }
}
