use std::collections::BTreeMap;

use sl_core::FormItem;

use super::macro_values::MacroValue;
use crate::semantic::env::ExpandEnv;

#[derive(Clone, Debug, Default)]
pub(crate) struct MacroEnv {
    pub(crate) current_module: Option<String>,
    pub(crate) imports: Vec<String>,
    pub(crate) macro_name: String,
    pub(crate) attributes: BTreeMap<String, String>,
    pub(crate) content: Vec<FormItem>,
    pub(crate) locals: BTreeMap<String, MacroValue>,
    pub(crate) gensym_counter: usize,
}

impl MacroEnv {
    pub(crate) fn from_invocation(
        expand_env: &ExpandEnv,
        macro_name: &str,
        attributes: BTreeMap<String, String>,
        content: Vec<FormItem>,
    ) -> Self {
        Self {
            current_module: expand_env.module.module_name.clone(),
            imports: expand_env.module.imports.clone(),
            macro_name: macro_name.to_string(),
            attributes,
            content,
            locals: BTreeMap::new(),
            gensym_counter: 0,
        }
    }

    pub(crate) fn context_label(&self) -> String {
        let module_name = self.current_module.as_deref().unwrap_or("<unknown>");
        format!(
            "macro `{}` in module `{}` ({} imports)",
            self.macro_name,
            module_name,
            self.imports.len()
        )
    }
}
