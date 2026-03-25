use std::collections::{BTreeMap, BTreeSet};

use sl_core::ScriptLangError;

use super::{ConstEnv, ConstLookup, ConstValue, eval_const_form};
use crate::semantic::required_attr;
use crate::semantic::types::{MemberKind, ModulePath, ResolvedRef};

use super::imports::{alias_name, validate_alias_target, validate_import_target};
use super::modules::{DEFAULT_KERNEL_MODULE, ModuleCatalog};

/// Kinds of module members searched by ScopeResolver.
#[derive(Clone, Copy)]
enum MemberSearchKind {
    Var,
    Function,
}

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
    aliases: BTreeMap<String, ModulePath>,
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

    fn imports(&self) -> &[ModulePath] {
        &self.imports
    }

    fn normalize_module_path<'a>(&'a self, module_name: &'a str) -> &'a str {
        self.aliases
            .get(module_name)
            .map(|path| path.as_str())
            .unwrap_or(module_name)
    }

    fn can_access_module(&self, module_name: &str) -> bool {
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
    fn normalize_literal(&self, raw: &str, prefix: char) -> String {
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

    fn normalize_script_literal(&self, raw: &str) -> String {
        self.normalize_literal(raw, '@')
    }

    fn normalize_function_literal(&self, raw: &str) -> String {
        self.normalize_literal(raw, '#')
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
                "alias" => {
                    let alias_target = required_attr(child, "name")?.to_string();
                    validate_alias_target(self.modules, child, module_name, &alias_target)?;
                    let alias_name = alias_name(child)?;
                    scope.add_alias(&alias_name, &alias_target);
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

                    let mut resolver = ScopeResolver::new(self.modules, self, &scope);
                    let (_, value) =
                        eval_const_form(child, &const_env, &mut resolver, &remaining_const_names)?;
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

/// Result of normalizing a module path and checking basic preconditions.
/// Used by try_qualified_export and resolve_qualified_const to share preamble logic.
enum QualifiedExportLookup<'a> {
    NotFound,
    NotAccessible,
    Found {
        normalized: &'a str,
        exports: &'a crate::semantic::env::ModuleExports,
    },
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

    /// Search imported modules (in reverse order) for a member matching `kind` and `name`.
    /// Returns the import path and resolved name if found.
    fn search_imports_reverse(
        &self,
        kind: MemberSearchKind,
        name: &str,
    ) -> Option<(String, String)> {
        for import in self.scope.imports().iter().rev() {
            let import_path = import.as_str();
            let exports = match self.modules.exports(import_path) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let found = match kind {
                MemberSearchKind::Var => exports.vars.contains_exported(name),
                MemberSearchKind::Function => exports.functions.contains_exported(name),
            };
            if found {
                return Some((import_path.to_string(), name.to_string()));
            }
        }
        None
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
        if let Some((module_path, var_name)) =
            self.search_imports_reverse(MemberSearchKind::Var, name)
        {
            return Ok(Some(ResolvedRef::new(
                &module_path,
                &var_name,
                MemberKind::Var,
            )));
        }
        Ok(None)
    }

    // Normalize path, check module exists and is accessible, then return exports.
    // Shared by try_qualified_export and resolve_qualified_const.
    fn try_lookup_qualified_export<'c>(
        modules: &'c ModuleCatalog<'c>,
        scope: &'c ModuleScope,
        module_path: &'c str,
    ) -> Result<QualifiedExportLookup<'c>, ScriptLangError> {
        let normalized = scope.normalize_module_path(module_path);
        if !modules.contains(normalized) {
            return Ok(QualifiedExportLookup::NotFound);
        }
        if !scope.can_access_module(normalized) {
            return Ok(QualifiedExportLookup::NotAccessible);
        }
        let exports = modules.exports(normalized)?;
        Ok(QualifiedExportLookup::Found {
            normalized,
            exports,
        })
    }

    /// Normalize path, check module is accessible, then call `f` with exports.
    /// Returns `Ok(None)` if module doesn't exist; `Err` if not accessible; `f`'s `Err` propagates.
    fn try_qualified_export<F, T>(
        &self,
        module_path: &str,
        f: F,
    ) -> Result<Option<T>, ScriptLangError>
    where
        F: FnOnce(&crate::semantic::env::ModuleExports, &str) -> Result<Option<T>, ScriptLangError>,
    {
        match Self::try_lookup_qualified_export(self.modules, self.scope, module_path)? {
            QualifiedExportLookup::NotFound => Ok(None),
            QualifiedExportLookup::NotAccessible => Err(ScriptLangError::message(format!(
                "module `{module_path}` is not imported into `{}`",
                self.scope.current_module()
            ))),
            QualifiedExportLookup::Found {
                normalized,
                exports,
            } => f(exports, normalized),
        }
    }

    pub(crate) fn resolve_qualified_var_ref(
        &self,
        module_path: &str,
        name: &str,
    ) -> Result<Option<ResolvedRef>, ScriptLangError> {
        if let Some(exports) = self.try_qualified_export(module_path, |exports, normalized| {
            if normalized == self.scope.current_module() {
                Ok(exports
                    .vars
                    .contains_declared(name)
                    .then(|| ResolvedRef::new(normalized, name, MemberKind::Var)))
            } else if exports.vars.contains_exported(name) {
                Ok(Some(ResolvedRef::new(normalized, name, MemberKind::Var)))
            } else {
                Err(ScriptLangError::message(format!(
                    "module `{module_path}` does not export var `{name}`"
                )))
            }
        })? {
            return Ok(Some(exports));
        }
        Ok(None)
    }

    pub(crate) fn resolve_short_function_ref(
        &self,
        name: &str,
    ) -> Result<Option<String>, ScriptLangError> {
        if self
            .modules
            .exports(self.scope.current_module())?
            .functions
            .contains_declared(name)
        {
            return Ok(Some(format!("{}.{}", self.scope.current_module(), name)));
        }
        if let Some((module_path, fn_name)) =
            self.search_imports_reverse(MemberSearchKind::Function, name)
        {
            return Ok(Some(format!("{module_path}.{fn_name}")));
        }
        Ok(None)
    }

    pub(crate) fn resolve_qualified_function_ref(
        &self,
        module_path: &str,
        name: &str,
    ) -> Result<Option<String>, ScriptLangError> {
        if let Some(qualified) = self.try_qualified_export(module_path, |exports, normalized| {
            if normalized == self.scope.current_module() {
                Ok(exports
                    .functions
                    .contains_declared(name)
                    .then(|| format!("{normalized}.{name}")))
            } else if exports.functions.contains_exported(name) {
                Ok(Some(format!("{normalized}.{name}")))
            } else {
                Err(ScriptLangError::message(format!(
                    "module `{module_path}` does not export function `{name}`"
                )))
            }
        })? {
            return Ok(Some(qualified));
        }
        Ok(None)
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
        let lookup = Self::try_lookup_qualified_export(self.modules, self.scope, module_path)?;
        match lookup {
            QualifiedExportLookup::NotFound => Ok(QualifiedConstLookup::NotModulePath),
            QualifiedExportLookup::NotAccessible => Ok(QualifiedConstLookup::HiddenModule),
            QualifiedExportLookup::Found {
                normalized,
                exports,
            } => {
                let is_visible = if normalized == self.scope.current_module() {
                    exports.consts.contains_declared(name)
                } else {
                    exports.consts.contains_exported(name)
                };
                if !is_visible {
                    return Ok(QualifiedConstLookup::UnknownConst);
                }
                let value = self
                    .const_catalog
                    .resolve_const(normalized, name)?
                    .expect("checked exports before resolving");
                Ok(QualifiedConstLookup::Value(value))
            }
        }
    }

    fn resolve_script_literal(&mut self, raw: &str) -> Result<String, ScriptLangError> {
        self.modules.resolve_script_literal(
            self.scope.current_module(),
            &self.scope.normalize_script_literal(raw),
        )
    }

    fn resolve_function_literal(&mut self, raw: &str) -> Result<String, ScriptLangError> {
        self.modules.resolve_function_literal(
            self.scope.current_module(),
            &self.scope.normalize_function_literal(raw),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use sl_core::Form;

    use crate::semantic::env::{ModuleExports, ModuleState, ProgramState};
    use crate::semantic::expand::test_helpers::{children, form, form_field, text};

    fn const_form(name: &str, value: &str) -> Form {
        form(
            "const",
            vec![
                form_field("name", name),
                form_field("type", "int"),
                children(vec![text(value)]),
            ],
        )
    }

    fn module_state_with(
        module_name: &str,
        exports: ModuleExports,
        children: Vec<sl_core::Form>,
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
    fn scope_resolver_covers_var_and_const_visibility_paths() {
        let program = program_state();
        let catalog = ModuleCatalog::build(&program).expect("catalog");
        let mut const_catalog = ConstCatalog::new(&catalog);
        let mut scope = ModuleScope::initial(&catalog, "main");
        scope.add_import("helper");
        scope.add_alias("h", "helper");
        let mut resolver = ScopeResolver::new(&catalog, &mut const_catalog, &scope);

        assert!(matches!(
            resolver.resolve_short_var_ref("value").expect("short local"),
            Some(reference) if reference.qualified_name() == "main.value"
        ));
        assert!(matches!(
            resolver.resolve_qualified_var_ref("helper", "value").expect("qualified import"),
            Some(reference) if reference.qualified_name() == "helper.value"
        ));
        assert!(matches!(
            resolver.resolve_qualified_var_ref("h", "value").expect("qualified alias"),
            Some(reference) if reference.qualified_name() == "helper.value"
        ));
        let hidden_var = resolver
            .resolve_qualified_var_ref("helper", "priv")
            .expect_err("private var");
        assert!(
            hidden_var
                .to_string()
                .contains("does not export var `priv`")
        );
        let hidden_module = resolver
            .resolve_qualified_const("nope", "x")
            .expect("missing module");
        assert!(matches!(hidden_module, QualifiedConstLookup::NotModulePath));
        let hidden_import = resolver
            .resolve_qualified_const("helper", "hidden")
            .expect("hidden const");
        assert!(matches!(hidden_import, QualifiedConstLookup::UnknownConst));
        assert!(matches!(
            resolver.resolve_short_const("answer").expect("short const"),
            Some(ConstValue::Integer(42))
        ));
        assert!(matches!(
            resolver
                .resolve_qualified_const("helper", "answer")
                .expect("qualified const"),
            QualifiedConstLookup::Value(ConstValue::Integer(42))
        ));
        assert!(matches!(
            resolver
                .resolve_qualified_const("h", "answer")
                .expect("qualified alias const"),
            QualifiedConstLookup::Value(ConstValue::Integer(42))
        ));
        assert_eq!(
            resolver
                .resolve_script_literal("@main")
                .expect("script literal"),
            "main.main"
        );
        assert_eq!(
            resolver
                .resolve_script_literal("@h.entry")
                .expect("script alias literal"),
            "helper.entry"
        );
        assert_eq!(
            resolver
                .resolve_function_literal("#choose")
                .expect("short function literal"),
            "main.choose"
        );
        assert_eq!(
            resolver
                .resolve_function_literal("#h.pick")
                .expect("function alias literal"),
            "helper.pick"
        );

        // Verify search_imports_reverse covers the Function branch via resolve_short_function_ref.
        assert_eq!(
            resolver
                .resolve_short_function_ref("pick")
                .expect("short function from import"),
            Some("helper.pick".to_string())
        );
        assert_eq!(
            resolver
                .resolve_short_function_ref("choose")
                .expect("short function from self"),
            Some("main.choose".to_string())
        );
        assert_eq!(
            resolver
                .resolve_short_function_ref("missing")
                .expect("missing function"),
            None
        );
    }

    #[test]
    fn const_catalog_covers_cache_miss_and_cycle_detection() {
        let cyclic_program = ProgramState {
            modules: BTreeMap::from([(
                "main".to_string(),
                module_state_with(
                    "main",
                    exports_with(&[("a", true), ("b", true)], &[], &[], &[]),
                    vec![
                        form(
                            "const",
                            vec![
                                form_field("name", "a"),
                                form_field("type", "int"),
                                children(vec![text("b")]),
                            ],
                        ),
                        form(
                            "const",
                            vec![
                                form_field("name", "b"),
                                form_field("type", "int"),
                                children(vec![text("a")]),
                            ],
                        ),
                    ],
                ),
            )]),
            module_order: vec!["main".to_string()],
            module_macros: BTreeMap::new(),
        };
        let catalog = ModuleCatalog::build(&cyclic_program).expect("catalog");
        let mut const_catalog = ConstCatalog::new(&catalog);

        let none = const_catalog
            .resolve_const("main", "missing")
            .expect("missing const");
        assert!(none.is_none());

        let cycle = const_catalog.resolve_const("main", "a").expect_err("cycle");
        assert!(
            cycle
                .to_string()
                .contains("cannot be referenced before it is defined")
        );
    }

    #[test]
    fn scope_resolver_reports_hidden_modules_and_unknown_functions() {
        let program = program_state();
        let catalog = ModuleCatalog::build(&program).expect("catalog");
        let mut const_catalog = ConstCatalog::new(&catalog);
        let scope = ModuleScope::initial(&catalog, "main");
        let mut resolver = ScopeResolver::new(&catalog, &mut const_catalog, &scope);

        let hidden = resolver
            .resolve_qualified_const("helper", "answer")
            .expect("hidden module without import");
        assert!(matches!(hidden, QualifiedConstLookup::HiddenModule));

        let unknown_function = resolver
            .resolve_function_literal("#helper.nope")
            .expect_err("unknown function");
        assert!(unknown_function.to_string().contains("unknown function"));

        let unknown_script = resolver
            .resolve_script_literal("@helper.nope")
            .expect_err("unknown script");
        assert!(unknown_script.to_string().contains("unknown script"));
    }

    #[test]
    fn module_scope_normalizes_aliases_for_literals_and_access_checks() {
        let program = program_state();
        let catalog = ModuleCatalog::build(&program).expect("catalog");
        let mut scope = ModuleScope::initial(&catalog, "main");
        scope.add_alias("h", "helper");

        assert!(scope.can_access_module("h"));
        assert_eq!(scope.normalize_script_literal("@h.entry"), "@helper.entry");
        assert_eq!(scope.normalize_function_literal("#h.pick"), "#helper.pick");
        assert_eq!(scope.normalize_script_literal("@loop"), "@loop");
        assert_eq!(scope.normalize_function_literal("#pick"), "#pick");
    }

    #[test]
    fn module_scope_and_alias_helpers_cover_edge_paths() {
        let program = program_state();
        let catalog = ModuleCatalog::build(&program).expect("catalog");
        let scope = ModuleScope::initial(&catalog, "main");

        assert_eq!(scope.normalize_script_literal("@"), "@");
        assert_eq!(scope.normalize_function_literal("#"), "#");
        assert_eq!(scope.normalize_script_literal("plain"), "plain");
        assert_eq!(scope.normalize_function_literal("plain"), "plain");
        assert!(!scope.can_access_module("helper"));

        let alias = form("alias", vec![form_field("name", "main.helper")]);
        assert_eq!(alias_name(&alias).expect("default alias"), "helper");

        let invalid = form("alias", vec![form_field("name", "")]);
        assert!(
            alias_name(&invalid)
                .expect_err("invalid alias target")
                .to_string()
                .contains("requires valid `name`")
        );
    }

    #[test]
    fn scope_resolver_covers_current_module_and_hidden_var_paths() {
        let program = program_state();
        let catalog = ModuleCatalog::build(&program).expect("catalog");
        let mut const_catalog = ConstCatalog::new(&catalog);
        let scope = ModuleScope::initial(&catalog, "main");
        let resolver = ScopeResolver::new(&catalog, &mut const_catalog, &scope);

        assert!(matches!(
            resolver
                .resolve_qualified_var_ref("main", "value")
                .expect("current module qualified"),
            Some(reference) if reference.qualified_name() == "main.value"
        ));
        assert!(
            resolver
                .resolve_qualified_var_ref("main", "missing")
                .expect("missing current var")
                .is_none()
        );
        assert!(
            resolver
                .resolve_qualified_var_ref("missing", "value")
                .expect("not a module")
                .is_none()
        );
    }

    #[test]
    fn const_catalog_and_scope_cover_cache_alias_and_hidden_module_paths() {
        let program = program_state();
        let catalog = ModuleCatalog::build(&program).expect("catalog");
        let mut const_catalog = ConstCatalog::new(&catalog);

        assert!(matches!(
            const_catalog
                .resolve_const("helper", "answer")
                .expect("resolve first"),
            Some(ConstValue::Integer(42))
        ));
        assert!(matches!(
            const_catalog
                .resolve_const("helper", "answer")
                .expect("resolve cached"),
            Some(ConstValue::Integer(42))
        ));

        let mut scope = ModuleScope::initial(&catalog, "main");
        scope.add_import("helper");
        scope.add_alias("h", "helper");
        let mut resolver = ScopeResolver::new(&catalog, &mut const_catalog, &scope);

        assert!(
            resolver
                .resolve_short_const("missing")
                .expect("missing short const")
                .is_none()
        );
        assert!(matches!(
            resolver
                .resolve_qualified_const("main", "local")
                .expect("current module const"),
            QualifiedConstLookup::Value(ConstValue::Integer(1))
        ));

        let hidden_var = resolver
            .resolve_qualified_var_ref("kernel", "zero")
            .expect_err("kernel not imported for var ref");
        assert!(
            hidden_var
                .to_string()
                .contains("does not export var `zero`")
        );

        let helper_private = resolver
            .resolve_qualified_const("helper", "hidden")
            .expect("private helper const");
        assert!(matches!(helper_private, QualifiedConstLookup::UnknownConst));

        let alias_public = resolver
            .resolve_qualified_const("h", "answer")
            .expect("alias const");
        assert!(matches!(
            alias_public,
            QualifiedConstLookup::Value(ConstValue::Integer(42))
        ));
    }
}
