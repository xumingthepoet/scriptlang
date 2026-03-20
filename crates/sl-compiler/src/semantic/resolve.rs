use std::collections::{BTreeMap, BTreeSet};

use sl_core::{Form, ScriptLangError};

use super::const_eval::{ConstEnv, ConstLookup, ConstValue, parse_const_value};
use super::types::{ModulePath, ResolvedRef};
use crate::form_util::{child_forms, error_at, required_attr, trimmed_text_items};

pub(crate) const DEFAULT_KERNEL_MODULE: &str = "kernel";

pub(crate) enum QualifiedConstLookup {
    Value(ConstValue),
    HiddenModule,
    UnknownConst,
    NotModulePath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModuleScope {
    current_module: ModulePath,
    imports: Vec<ModulePath>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ModuleExports {
    pub(crate) consts: BTreeSet<String>,
    pub(crate) scripts: BTreeSet<String>,
}

pub(crate) struct ModuleCatalog<'a> {
    entries: BTreeMap<String, ModuleCatalogEntry<'a>>,
}

struct ModuleCatalogEntry<'a> {
    children: Vec<&'a Form>,
    exports: ModuleExports,
}

pub(crate) struct ConstCatalog<'a> {
    modules: &'a ModuleCatalog<'a>,
    cached: BTreeMap<String, ConstEnv>,
    resolving: BTreeSet<String>,
}

pub(crate) struct ScopeResolver<'a, 'b> {
    modules: &'a ModuleCatalog<'a>,
    const_catalog: &'b mut ConstCatalog<'a>,
    scope: &'b ModuleScope,
}

pub(crate) trait NameResolver: ConstLookup {
    fn resolve_script_ref(&self, raw: &str) -> Result<ResolvedRef, ScriptLangError>;
}

impl ModulePath {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl ModuleScope {
    pub(crate) fn initial(catalog: &ModuleCatalog<'_>, module_name: &str) -> Self {
        let mut imports = Vec::new();
        if module_name != DEFAULT_KERNEL_MODULE && catalog.contains(DEFAULT_KERNEL_MODULE) {
            imports.push(ModulePath(DEFAULT_KERNEL_MODULE.to_string()));
        }
        Self {
            current_module: ModulePath(module_name.to_string()),
            imports,
        }
    }

    pub(crate) fn current_module(&self) -> &str {
        self.current_module.as_str()
    }

    pub(crate) fn add_import(&mut self, module_name: &str) {
        self.imports.push(ModulePath(module_name.to_string()));
    }

    fn imports(&self) -> &[ModulePath] {
        &self.imports
    }

    fn can_access_module(&self, module_name: &str) -> bool {
        module_name == self.current_module()
            || self
                .imports
                .iter()
                .any(|import| import.as_str() == module_name)
    }
}

impl<'a> ModuleCatalog<'a> {
    pub(crate) fn build(forms: &'a [Form]) -> Result<Self, ScriptLangError> {
        let mut entries = BTreeMap::new();
        for form in forms {
            if form.head != "module" {
                return Err(error_at(
                    form,
                    format!("top-level <{}> is not supported in MVP", form.head),
                ));
            }

            let name = required_attr(form, "name")?.to_string();
            if entries.contains_key(&name) {
                return Err(error_at(
                    form,
                    format!("duplicate module declaration `{name}`"),
                ));
            }

            let children = child_forms(form)?;
            let mut exports = ModuleExports::default();
            for child in &children {
                match child.head.as_str() {
                    "const" => {
                        let const_name = required_attr(child, "name")?.to_string();
                        if !exports.consts.insert(const_name.clone()) {
                            return Err(error_at(
                                child,
                                format!("duplicate const declaration `{name}.{const_name}`"),
                            ));
                        }
                    }
                    "script" => {
                        exports
                            .scripts
                            .insert(required_attr(child, "name")?.to_string());
                    }
                    "import" => {
                        let _ = required_attr(child, "name")?;
                    }
                    _ => {}
                }
            }

            entries.insert(name, ModuleCatalogEntry { children, exports });
        }
        Ok(Self { entries })
    }

    pub(crate) fn contains(&self, module_name: &str) -> bool {
        self.entries.contains_key(module_name)
    }

    pub(crate) fn exports(&self, module_name: &str) -> Result<&ModuleExports, ScriptLangError> {
        Ok(&self.entry(module_name)?.exports)
    }

    fn entry(&self, module_name: &str) -> Result<&ModuleCatalogEntry<'a>, ScriptLangError> {
        self.entries.get(module_name).ok_or_else(|| {
            ScriptLangError::message(format!("module `{module_name}` does not exist"))
        })
    }
}

impl<'a> ConstCatalog<'a> {
    pub(crate) fn new(modules: &'a ModuleCatalog<'a>) -> Self {
        Self {
            modules,
            cached: BTreeMap::new(),
            resolving: BTreeSet::new(),
        }
    }

    fn cached_env(&self, module_name: &str) -> ConstEnv {
        self.cached.get(module_name).cloned().unwrap_or_default()
    }

    fn cache_value(&mut self, module_name: &str, const_name: &str, value: ConstValue) {
        self.cached
            .entry(module_name.to_string())
            .or_default()
            .insert(const_name.to_string(), value);
    }

    pub(crate) fn resolve_const(
        &mut self,
        module_name: &str,
        const_name: &str,
    ) -> Result<Option<ConstValue>, ScriptLangError> {
        if let Some(value) = self
            .cached
            .get(module_name)
            .and_then(|env| env.get(const_name))
            .cloned()
        {
            return Ok(Some(value));
        }

        if !self
            .modules
            .exports(module_name)?
            .consts
            .contains(const_name)
        {
            return Ok(None);
        }

        let key = format!("{module_name}.{const_name}");
        if !self.resolving.insert(key.clone()) {
            return Err(ScriptLangError::message(format!(
                "cyclic const resolution involving `{key}`"
            )));
        }

        let result = self.compute_const(module_name, const_name);
        self.resolving.remove(&key);
        result
    }

    fn compute_const(
        &mut self,
        module_name: &str,
        target_name: &str,
    ) -> Result<Option<ConstValue>, ScriptLangError> {
        let entry = self.modules.entry(module_name)?;
        let mut remaining_const_names = entry.exports.consts.clone();
        let mut const_env = self.cached_env(module_name);
        for cached_name in const_env.keys() {
            remaining_const_names.remove(cached_name);
        }
        let mut scope = ModuleScope::initial(self.modules, module_name);

        for child in &entry.children {
            match child.head.as_str() {
                "import" => {
                    let import_name = required_attr(child, "name")?.to_string();
                    validate_import_target(self.modules, child, module_name, &import_name)?;
                    scope.add_import(&import_name);
                }
                "const" => {
                    let const_name = required_attr(child, "name")?.to_string();
                    if let Some(value) = const_env.get(&const_name).cloned() {
                        remaining_const_names.remove(&const_name);
                        if const_name == target_name {
                            return Ok(Some(value));
                        }
                        continue;
                    }

                    let raw = trimmed_text_items(child)?;
                    let mut blocked = remaining_const_names.clone();
                    blocked.remove(&const_name);
                    let mut resolver = ScopeResolver::new(self.modules, self, &scope);
                    let value = parse_const_value(&raw, &const_env, &mut resolver, &blocked)?;
                    self.cache_value(module_name, &const_name, value.clone());
                    remaining_const_names.remove(&const_name);
                    const_env.insert(const_name.clone(), value.clone());
                    if const_name == target_name {
                        return Ok(Some(value));
                    }
                }
                _ => {}
            }
        }

        Ok(const_env.get(target_name).cloned())
    }
}

impl<'a, 'b> ScopeResolver<'a, 'b> {
    pub(crate) fn new(
        modules: &'a ModuleCatalog<'a>,
        const_catalog: &'b mut ConstCatalog<'a>,
        scope: &'b ModuleScope,
    ) -> Self {
        Self {
            modules,
            const_catalog,
            scope,
        }
    }

    fn resolve_script_ref_impl(&self, raw: &str) -> Result<ResolvedRef, ScriptLangError> {
        let raw = raw.strip_prefix('@').unwrap_or(raw);
        if let Some((target_module, script_name)) = raw.rsplit_once('.') {
            if self.scope.can_access_module(target_module) {
                return Ok(ResolvedRef::script(target_module, script_name));
            }
            if self.modules.contains(target_module) {
                return Err(ScriptLangError::message(format!(
                    "script `{raw}` referenced by <goto> is not visible in module `{}`",
                    self.scope.current_module()
                )));
            }
            return Ok(ResolvedRef::script(target_module, script_name));
        }

        if self
            .modules
            .exports(self.scope.current_module())?
            .scripts
            .contains(raw)
        {
            return Ok(ResolvedRef::script(self.scope.current_module(), raw));
        }

        for import in self.scope.imports().iter().rev() {
            if self.modules.exports(import.as_str())?.scripts.contains(raw) {
                return Ok(ResolvedRef::script(import.as_str(), raw));
            }
        }

        Ok(ResolvedRef::script(self.scope.current_module(), raw))
    }
}

impl ConstLookup for ScopeResolver<'_, '_> {
    fn current_module(&self) -> &str {
        self.scope.current_module()
    }

    fn resolve_short_const(&mut self, name: &str) -> Result<Option<ConstValue>, ScriptLangError> {
        for import in self.scope.imports().iter().rev() {
            if !self.modules.exports(import.as_str())?.consts.contains(name) {
                continue;
            }
            if let Some(value) = self.const_catalog.resolve_const(import.as_str(), name)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    fn resolve_qualified_const(
        &mut self,
        module_path: &str,
        name: &str,
    ) -> Result<QualifiedConstLookup, ScriptLangError> {
        if !self.modules.contains(module_path) {
            return Ok(QualifiedConstLookup::NotModulePath);
        }
        if !self.scope.can_access_module(module_path) {
            return Ok(QualifiedConstLookup::HiddenModule);
        }
        if !self.modules.exports(module_path)?.consts.contains(name) {
            return Ok(QualifiedConstLookup::UnknownConst);
        }
        let value = self
            .const_catalog
            .resolve_const(module_path, name)?
            .expect("checked exports before resolving");
        Ok(QualifiedConstLookup::Value(value))
    }
}

impl NameResolver for ScopeResolver<'_, '_> {
    fn resolve_script_ref(&self, raw: &str) -> Result<ResolvedRef, ScriptLangError> {
        self.resolve_script_ref_impl(raw)
    }
}

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
