use std::collections::{BTreeMap, BTreeSet};

use sl_core::{Form, ScriptLangError};

use super::{
    ConstEnv, ConstLookup, ConstValue, parse_const_value,
    parse_declared_type_form as parse_declared_type,
};
use crate::names::script_literal_key;
use crate::semantic::env::{ModuleExports, ModuleState, ProgramState};
use crate::semantic::types::{MemberKind, ModulePath, ResolvedRef};
use crate::semantic::{body_expr, error_at, required_attr};

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

pub(crate) struct ModuleCatalog<'a> {
    program: &'a ProgramState,
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

    fn module_state(&self, module_name: &str) -> Result<&'a ModuleState, ScriptLangError> {
        self.program.modules.get(module_name).ok_or_else(|| {
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
            .contains_declared(const_name)
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
        let module = self.modules.module_state(module_name)?;
        let mut remaining_const_names = module.exports.consts.declared_names();
        let mut const_env = self.cached_env(module_name);
        for cached_name in const_env.keys() {
            remaining_const_names.remove(cached_name);
        }
        let mut scope = ModuleScope::initial(self.modules, module_name);

        for child in &module.children {
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

                    let raw = body_expr(child)?;
                    let mut blocked = remaining_const_names.clone();
                    blocked.remove(&const_name);
                    let mut resolver = ScopeResolver::new(self.modules, self, &scope);
                    let declared_type = parse_declared_type(child)?;
                    let value = parse_const_value(
                        &raw,
                        &const_env,
                        &mut resolver,
                        &blocked,
                        Some(&declared_type),
                    )?;
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

    pub(crate) fn current_module(&self) -> &str {
        self.scope.current_module()
    }

    pub(crate) fn modules(&self) -> &'a ModuleCatalog<'a> {
        self.modules
    }

    pub(crate) fn resolve_short_var_ref(
        &self,
        name: &str,
    ) -> Result<Option<ResolvedRef>, ScriptLangError> {
        if self
            .modules
            .exports(self.scope.current_module())?
            .vars
            .contains_declared(name)
        {
            return Ok(Some(ResolvedRef::new(
                self.scope.current_module(),
                name,
                MemberKind::Var,
            )));
        }
        for import in self.scope.imports().iter().rev() {
            if self
                .modules
                .exports(import.as_str())?
                .vars
                .contains_exported(name)
            {
                return Ok(Some(ResolvedRef::new(
                    import.as_str(),
                    name,
                    MemberKind::Var,
                )));
            }
        }
        Ok(None)
    }

    pub(crate) fn resolve_qualified_var_ref(
        &self,
        module_path: &str,
        name: &str,
    ) -> Result<Option<ResolvedRef>, ScriptLangError> {
        if !self.modules.contains(module_path) {
            return Ok(None);
        }
        if !self.scope.can_access_module(module_path) {
            return Err(ScriptLangError::message(format!(
                "module `{module_path}` is not imported into `{}`",
                self.scope.current_module()
            )));
        }
        let exports = self.modules.exports(module_path)?;
        if module_path == self.scope.current_module() {
            if exports.vars.contains_declared(name) {
                return Ok(Some(ResolvedRef::new(module_path, name, MemberKind::Var)));
            }
            return Ok(None);
        }
        if exports.vars.contains_exported(name) {
            Ok(Some(ResolvedRef::new(module_path, name, MemberKind::Var)))
        } else {
            Err(ScriptLangError::message(format!(
                "module `{module_path}` does not export var `{name}`"
            )))
        }
    }
}

impl ConstLookup for ScopeResolver<'_, '_> {
    fn current_module(&self) -> &str {
        self.scope.current_module()
    }

    fn resolve_short_const(&mut self, name: &str) -> Result<Option<ConstValue>, ScriptLangError> {
        for import in self.scope.imports().iter().rev() {
            if !self
                .modules
                .exports(import.as_str())?
                .consts
                .contains_exported(name)
            {
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
        let exports = self.modules.exports(module_path)?;
        let is_visible = if module_path == self.scope.current_module() {
            exports.consts.contains_declared(name)
        } else {
            exports.consts.contains_exported(name)
        };
        if !is_visible {
            return Ok(QualifiedConstLookup::UnknownConst);
        }
        let value = self
            .const_catalog
            .resolve_const(module_path, name)?
            .expect("checked exports before resolving");
        Ok(QualifiedConstLookup::Value(value))
    }

    fn resolve_script_literal(&mut self, raw: &str) -> Result<String, ScriptLangError> {
        self.modules
            .resolve_script_literal(self.scope.current_module(), raw)
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
