use std::collections::{BTreeMap, BTreeSet};

use sl_core::{Form, ScriptLangError, TextSegment, TextTemplate};

use super::const_eval::{ConstEnv, ConstLookup, ConstValue, parse_const_value};
use super::types::{ModulePath, ResolvedRef, runtime_global_name};
use crate::form_util::{attr, child_forms, error_at, required_attr, trimmed_text_items};

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
pub(crate) struct ModuleMembers {
    declared: BTreeSet<String>,
    exported: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ModuleExports {
    pub(crate) consts: ModuleMembers,
    pub(crate) scripts: ModuleMembers,
    pub(crate) vars: ModuleMembers,
}

impl ModuleMembers {
    fn insert(&mut self, name: String, exported: bool) -> bool {
        if !self.declared.insert(name.clone()) {
            return false;
        }
        if exported {
            self.exported.insert(name);
        }
        true
    }

    fn contains_declared(&self, name: &str) -> bool {
        self.declared.contains(name)
    }

    fn contains_exported(&self, name: &str) -> bool {
        self.exported.contains(name)
    }

    fn declared_names(&self) -> BTreeSet<String> {
        self.declared.clone()
    }
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
                        if !exports
                            .consts
                            .insert(const_name.clone(), !is_private(child)?)
                        {
                            return Err(error_at(
                                child,
                                format!("duplicate const declaration `{name}.{const_name}`"),
                            ));
                        }
                    }
                    "script" => {
                        let script_name = required_attr(child, "name")?.to_string();
                        if !exports
                            .scripts
                            .insert(script_name.clone(), !is_private(child)?)
                        {
                            return Err(error_at(
                                child,
                                format!("duplicate script declaration `{name}.{script_name}`"),
                            ));
                        }
                    }
                    "var" => {
                        let var_name = required_attr(child, "name")?.to_string();
                        if !exports.vars.insert(var_name.clone(), !is_private(child)?) {
                            return Err(error_at(
                                child,
                                format!("duplicate var declaration `{name}.{var_name}`"),
                            ));
                        }
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
        let entry = self.modules.entry(module_name)?;
        let mut remaining_const_names = entry.exports.consts.declared_names();
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
            if target_module == self.scope.current_module() {
                return Ok(ResolvedRef::script(target_module, script_name));
            }
            if self.scope.can_access_module(target_module)
                && self
                    .modules
                    .exports(target_module)?
                    .scripts
                    .contains_exported(script_name)
            {
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
            .contains_declared(raw)
        {
            return Ok(ResolvedRef::script(self.scope.current_module(), raw));
        }

        for import in self.scope.imports().iter().rev() {
            if self
                .modules
                .exports(import.as_str())?
                .scripts
                .contains_exported(raw)
            {
                return Ok(ResolvedRef::script(import.as_str(), raw));
            }
        }

        Ok(ResolvedRef::script(self.scope.current_module(), raw))
    }

    fn resolve_short_var_ref(&self, name: &str) -> Result<Option<ResolvedRef>, ScriptLangError> {
        if self
            .modules
            .exports(self.scope.current_module())?
            .vars
            .contains_declared(name)
        {
            return Ok(Some(ResolvedRef::new(
                self.scope.current_module(),
                name,
                super::types::MemberKind::Var,
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
                    super::types::MemberKind::Var,
                )));
            }
        }
        Ok(None)
    }

    fn resolve_qualified_var_ref(
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
                return Ok(Some(ResolvedRef::new(
                    module_path,
                    name,
                    super::types::MemberKind::Var,
                )));
            }
            return Ok(None);
        }
        if exports.vars.contains_exported(name) {
            Ok(Some(ResolvedRef::new(
                module_path,
                name,
                super::types::MemberKind::Var,
            )))
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
}

impl NameResolver for ScopeResolver<'_, '_> {
    fn resolve_script_ref(&self, raw: &str) -> Result<ResolvedRef, ScriptLangError> {
        self.resolve_script_ref_impl(raw)
    }
}

pub(crate) fn rewrite_expr_with_vars(
    source: &str,
    resolver: &ScopeResolver<'_, '_>,
    shadowed_names: &BTreeSet<String>,
) -> Result<String, ScriptLangError> {
    let mut rewritten = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let ch = bytes[cursor] as char;
        if ch == '"' || ch == '\'' {
            let end = scan_quoted(bytes, cursor)?;
            rewritten.push_str(&source[cursor..end]);
            cursor = end;
            continue;
        }

        if is_ident_start(ch) {
            let (end, segments) = scan_reference_path(source, cursor);
            let raw = &source[cursor..end];
            let first = segments[0].as_str();

            if shadowed_names.contains(first) || is_property_access(bytes, cursor) {
                rewritten.push_str(raw);
                cursor = end;
                continue;
            }

            let resolved = if segments.len() == 1 {
                resolver.resolve_short_var_ref(first)?
            } else {
                let module_path = segments[..segments.len() - 1].join(".");
                let name = segments.last().expect("qualified path");
                resolver.resolve_qualified_var_ref(&module_path, name)?
            };

            if let Some(target) = resolved {
                if is_map_key(source, end) {
                    rewritten.push_str(raw);
                } else {
                    rewritten.push_str(&runtime_global_name(&target.qualified_name()));
                }
            } else {
                rewritten.push_str(raw);
            }
            cursor = end;
            continue;
        }

        rewritten.push(ch);
        cursor += ch.len_utf8();
    }

    Ok(rewritten)
}

pub(crate) fn rewrite_template_with_vars(
    template: TextTemplate,
    resolver: &ScopeResolver<'_, '_>,
    shadowed_names: &BTreeSet<String>,
) -> Result<TextTemplate, ScriptLangError> {
    let segments = template
        .segments
        .into_iter()
        .map(|segment| match segment {
            TextSegment::Literal(text) => Ok(TextSegment::Literal(text)),
            TextSegment::Expr(expr) => Ok(TextSegment::Expr(rewrite_expr_with_vars(
                &expr,
                resolver,
                shadowed_names,
            )?)),
        })
        .collect::<Result<Vec<_>, ScriptLangError>>()?;
    Ok(TextTemplate { segments })
}

fn scan_quoted(bytes: &[u8], start: usize) -> Result<usize, ScriptLangError> {
    let quote = bytes[start];
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor += 2,
            ch if ch == quote => return Ok(cursor + 1),
            _ => cursor += 1,
        }
    }
    Err(ScriptLangError::message("unterminated string literal"))
}

fn scan_reference_path(source: &str, start: usize) -> (usize, Vec<String>) {
    let mut cursor = start;
    let mut segments = Vec::new();
    loop {
        let ident_start = cursor;
        cursor += 1;
        let bytes = source.as_bytes();
        while cursor < bytes.len() && is_ident_continue(bytes[cursor] as char) {
            cursor += 1;
        }
        segments.push(source[ident_start..cursor].to_string());
        if cursor >= bytes.len() || bytes[cursor] != b'.' {
            break;
        }
        let next = cursor + 1;
        if next >= bytes.len() || !is_ident_start(bytes[next] as char) {
            break;
        }
        cursor = next;
    }
    (cursor, segments)
}

fn is_property_access(bytes: &[u8], ident_start: usize) -> bool {
    let mut cursor = ident_start;
    while cursor > 0 {
        cursor -= 1;
        let ch = bytes[cursor] as char;
        if ch.is_whitespace() {
            continue;
        }
        return ch == '.';
    }
    false
}

fn is_map_key(source: &str, ident_end: usize) -> bool {
    let mut chars = source[ident_end..].chars();
    loop {
        match chars.next() {
            Some(ch) if ch.is_whitespace() => continue,
            Some(':') => return true,
            _ => return false,
        }
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
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

fn is_private(form: &Form) -> Result<bool, ScriptLangError> {
    match attr(form, "private") {
        None => Ok(false),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(other) => Err(error_at(
            form,
            format!("invalid boolean value `{other}` for `private`"),
        )),
    }
}
