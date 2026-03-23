use std::collections::BTreeMap;

use sl_core::FormItem;

use super::macro_values::MacroValue;
use crate::semantic::env::ExpandEnv;

#[derive(Clone, Debug, Default)]
pub(crate) struct MacroEnv {
    pub(crate) current_module: Option<String>,
    pub(crate) imports: Vec<String>,
    pub(crate) requires: Vec<String>,
    pub(crate) aliases: BTreeMap<String, String>,
    pub(crate) macro_name: String,
    pub(crate) attributes: BTreeMap<String, String>,
    pub(crate) content: Vec<FormItem>,
    pub(crate) locals: BTreeMap<String, MacroValue>,
    pub(crate) gensym_seed: usize,
    pub(crate) gensym_counter: usize,
}

impl MacroEnv {
    pub(crate) fn from_invocation(
        expand_env: &mut ExpandEnv,
        macro_name: &str,
        attributes: BTreeMap<String, String>,
        content: Vec<FormItem>,
    ) -> Self {
        let gensym_seed = expand_env.reserve_macro_invocation_seed();
        Self {
            current_module: expand_env.module.module_name.clone(),
            imports: expand_env.module.imports.clone(),
            requires: expand_env.module.requires.clone(),
            aliases: expand_env.module.aliases.clone(),
            macro_name: macro_name.to_string(),
            attributes,
            content,
            locals: BTreeMap::new(),
            gensym_seed,
            gensym_counter: 0,
        }
    }

    /// Get a macro invocation attribute value.
    pub(crate) fn get_attribute(&self, name: &str) -> Option<&String> {
        self.attributes.get(name)
    }

    /// Check if macro invocation has an attribute.
    pub(crate) fn has_attribute(&self, name: &str) -> bool {
        self.attributes.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

    use super::*;
    use crate::semantic::env::ExpandEnv;

    fn meta() -> FormMeta {
        FormMeta {
            source_name: Some("main.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        }
    }

    fn form(head: &str) -> Form {
        Form {
            head: head.to_string(),
            meta: meta(),
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(Vec::new()),
            }],
        }
    }

    #[test]
    fn macro_env_from_invocation_copies_expand_context() {
        let mut expand_env = ExpandEnv::default();
        expand_env
            .begin_module(Some("main".to_string()), Some("main.xml".to_string()))
            .ok();
        expand_env.add_import("kernel");
        expand_env.add_require("helper");
        expand_env.add_alias("h", "helper").expect("alias");

        let env = MacroEnv::from_invocation(
            &mut expand_env,
            "unless",
            BTreeMap::from([("when".to_string(), "true".to_string())]),
            vec![FormItem::Form(form("text"))],
        );

        assert_eq!(env.current_module.as_deref(), Some("main"));
        assert_eq!(env.imports, vec!["kernel".to_string()]);
        assert_eq!(env.requires, vec!["helper".to_string()]);
        assert_eq!(env.aliases.get("h").map(String::as_str), Some("helper"));
        assert_eq!(env.macro_name, "unless");
        assert_eq!(env.attributes["when"], "true");
        assert_eq!(env.content.len(), 1);
        assert!(env.locals.is_empty());
        assert_eq!(env.gensym_seed, 1);
        assert_eq!(env.gensym_counter, 0);
    }

    #[test]
    fn macro_env_context_label_reports_current_context() {
        #[allow(non_local_definitions)]
        impl MacroEnv {
            fn context_label(&self) -> String {
                let module_name = self.current_module.as_deref().unwrap_or("<unknown>");
                format!(
                    "macro `{}` in module `{}` ({} imports, {} requires, {} aliases)",
                    self.macro_name,
                    module_name,
                    self.imports.len(),
                    self.requires.len(),
                    self.aliases.len()
                )
            }
        }
        let env = MacroEnv {
            current_module: Some("main".to_string()),
            imports: vec!["kernel".to_string()],
            requires: vec!["helper".to_string()],
            aliases: BTreeMap::from([("h".to_string(), "helper".to_string())]),
            macro_name: "unless".to_string(),
            attributes: BTreeMap::new(),
            content: Vec::new(),
            locals: BTreeMap::new(),
            gensym_seed: 0,
            gensym_counter: 0,
        };

        assert_eq!(
            env.context_label(),
            "macro `unless` in module `main` (1 imports, 1 requires, 1 aliases)"
        );
    }

    #[test]
    fn macro_env_gensym_seed_is_scoped_to_current_module() {
        let mut expand_env = ExpandEnv::default();
        expand_env
            .begin_module(Some("main".to_string()), Some("main.xml".to_string()))
            .expect("main module");

        let first = MacroEnv::from_invocation(&mut expand_env, "unless", BTreeMap::new(), vec![]);
        let second = MacroEnv::from_invocation(&mut expand_env, "if_else", BTreeMap::new(), vec![]);
        assert_eq!(first.gensym_seed, 1);
        assert_eq!(second.gensym_seed, 2);

        expand_env.finish_module();
        expand_env
            .begin_module(Some("helper".to_string()), Some("helper.xml".to_string()))
            .expect("helper module");

        let helper =
            MacroEnv::from_invocation(&mut expand_env, "surround", BTreeMap::new(), vec![]);
        assert_eq!(helper.gensym_seed, 1);
    }
}
