use sl_core::{Form, ScriptLangError};

use super::modules::ModuleCatalog;
use crate::semantic::error_at;

pub(crate) fn validate_import_target(
    catalog: &ModuleCatalog<'_>,
    form: &Form,
    current_module: &str,
    import_name: &str,
) -> Result<(), ScriptLangError> {
    if import_name == current_module {
        return Err(error_at(
            form,
            format!("module `{current_module}` cannot import itself"),
        ));
    }
    if catalog.contains(import_name) {
        Ok(())
    } else {
        Err(error_at(
            form,
            format!("imported module `{import_name}` does not exist"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use super::validate_import_target;
    use crate::semantic::env::{ModuleExports, ModuleState, ProgramState};
    use crate::semantic::expand::modules::ModuleCatalog;

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

    fn program_state() -> ProgramState {
        ProgramState {
            modules: BTreeMap::from([
                (
                    "kernel".to_string(),
                    ModuleState {
                        module_name: Some("kernel".to_string()),
                        imports: Vec::new(),
                        const_decls: BTreeMap::new(),
                        exports: ModuleExports::default(),
                        children: Vec::new(),
                        locals: Default::default(),
                    },
                ),
                (
                    "main".to_string(),
                    ModuleState {
                        module_name: Some("main".to_string()),
                        imports: Vec::new(),
                        const_decls: BTreeMap::new(),
                        exports: ModuleExports::default(),
                        children: Vec::new(),
                        locals: Default::default(),
                    },
                ),
                (
                    "helper".to_string(),
                    ModuleState {
                        module_name: Some("helper".to_string()),
                        imports: Vec::new(),
                        const_decls: BTreeMap::new(),
                        exports: ModuleExports::default(),
                        children: Vec::new(),
                        locals: Default::default(),
                    },
                ),
            ]),
            module_order: vec![
                "kernel".to_string(),
                "main".to_string(),
                "helper".to_string(),
            ],
            module_macros: BTreeMap::new(),
        }
    }

    #[test]
    fn validate_import_target_accepts_real_modules_and_rejects_invalid_targets() {
        let program = program_state();
        let catalog = ModuleCatalog::build(&program).expect("catalog");
        let import_form = form("import", vec![attr("name", "helper"), children(vec![])]);

        assert!(validate_import_target(&catalog, &import_form, "main", "helper").is_ok());
        let self_error =
            validate_import_target(&catalog, &import_form, "main", "main").expect_err("self");
        assert!(self_error.to_string().contains("cannot import itself"));
        let missing_error =
            validate_import_target(&catalog, &import_form, "main", "nope").expect_err("missing");
        assert!(missing_error.to_string().contains("does not exist"));
    }
}
