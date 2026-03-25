use std::collections::BTreeMap;

use crate::semantic::types::ModulePath;

use super::super::modules::{DEFAULT_KERNEL_MODULE, ModuleCatalog};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModuleScope {
    current_module: ModulePath,
    imports: Vec<ModulePath>,
    aliases: BTreeMap<String, ModulePath>,
}

impl ModuleScope {
    pub(crate) fn initial(catalog: &ModuleCatalog<'_>, module_name: &str) -> Self {
        let mut imports = Vec::new();
        if module_name != DEFAULT_KERNEL_MODULE && catalog.contains(DEFAULT_KERNEL_MODULE) {
            imports.push(ModulePath(DEFAULT_KERNEL_MODULE.to_string()));
        }
        let aliases = catalog
            .module_state(module_name)
            .ok()
            .map(|module| {
                module
                    .child_aliases
                    .iter()
                    .map(|(alias_name, target)| (alias_name.clone(), ModulePath(target.clone())))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        Self {
            current_module: ModulePath(module_name.to_string()),
            imports,
            aliases,
        }
    }

    pub(crate) fn current_module(&self) -> &str {
        self.current_module.as_str()
    }

    pub(crate) fn add_import(&mut self, module_name: &str) {
        self.imports.push(ModulePath(module_name.to_string()));
    }

    pub(crate) fn add_alias(&mut self, alias_name: &str, module_name: &str) {
        self.aliases
            .insert(alias_name.to_string(), ModulePath(module_name.to_string()));
    }

    pub(crate) fn imports(&self) -> &[ModulePath] {
        &self.imports
    }

    pub(crate) fn normalize_module_path<'a>(&'a self, module_name: &'a str) -> &'a str {
        self.aliases
            .get(module_name)
            .map(|path| path.as_str())
            .unwrap_or(module_name)
    }

    pub(crate) fn can_access_module(&self, module_name: &str) -> bool {
        let module_name = self.normalize_module_path(module_name);
        module_name == self.current_module()
            || self
                .aliases
                .values()
                .any(|alias| alias.as_str() == module_name)
            || self
                .imports
                .iter()
                .any(|import| import.as_str() == module_name)
    }

    /// Normalize a literal (script `@...` or function `#...`) by resolving its module path.
    pub(crate) fn normalize_literal(&self, raw: &str, prefix: char) -> String {
        let Some(rest) = raw.strip_prefix(prefix) else {
            return raw.to_string();
        };
        if rest.is_empty() {
            return format!("{prefix}");
        }
        if let Some((module_name, member_name)) = rest.rsplit_once('.') {
            let module_name = self.normalize_module_path(module_name);
            format!("{prefix}{module_name}.{member_name}")
        } else {
            format!("{prefix}{rest}")
        }
    }

    pub(crate) fn normalize_script_literal(&self, raw: &str) -> String {
        self.normalize_literal(raw, '@')
    }

    pub(crate) fn normalize_function_literal(&self, raw: &str) -> String {
        self.normalize_literal(raw, '#')
    }
}
