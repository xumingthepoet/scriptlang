use std::collections::{BTreeMap, BTreeSet};

use sl_core::{Form, FormItem};

use super::types::DeclaredType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompilePhase {
    Module,
    Script,
    Statement,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LocalScope {
    names: BTreeSet<String>,
}

impl LocalScope {
    #[cfg(test)]
    pub(crate) fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    pub(crate) fn insert(&mut self, name: impl Into<String>) {
        self.names.insert(name.into());
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ModuleState {
    pub(crate) module_name: Option<String>,
    pub(crate) imports: Vec<String>,
    pub(crate) requires: Vec<String>,
    pub(crate) aliases: BTreeMap<String, String>,
    pub(crate) child_aliases: BTreeMap<String, String>,
    pub(crate) const_decls: BTreeMap<String, PendingConstDecl>,
    pub(crate) exports: ModuleExports,
    pub(crate) children: Vec<Form>,
    pub(crate) locals: LocalScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingConstDecl {
    pub(crate) declared_type: DeclaredType,
    pub(crate) raw_expr: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ModuleMembers {
    declared: BTreeSet<String>,
    exported: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ModuleExports {
    pub(crate) consts: ModuleMembers,
    pub(crate) functions: ModuleMembers,
    pub(crate) scripts: ModuleMembers,
    pub(crate) vars: ModuleMembers,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProgramState {
    pub(crate) modules: BTreeMap<String, ModuleState>,
    pub(crate) module_order: Vec<String>,
    pub(crate) module_macros: BTreeMap<String, BTreeMap<String, MacroDefinition>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExpandEnv {
    pub(crate) phase: Option<CompilePhase>,
    pub(crate) source_name: Option<String>,
    pub(crate) program: ProgramState,
    pub(crate) module: ModuleState,
    pub(crate) macro_invocation_counters: BTreeMap<String, usize>,
    /// Module name of the caller when inside a `use` macro expansion.
    /// Used to detect conflicts when `use` injects public members into the caller.
    pub(crate) use_caller_module: Option<String>,
}

/// Macro parameter type in the new explicit parameter protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MacroParamType {
    /// Compile-time expression source
    Expr,
    /// Child AST nodes
    Ast,
    /// Compile-time string value
    String,
    /// Compile-time boolean value
    Bool,
    /// Compile-time integer value
    Int,
    /// Ordered key-value pairs
    Keyword,
    /// Module reference (before alias expansion)
    Module,
}

/// Single macro parameter declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MacroParam {
    pub(crate) param_type: MacroParamType,
    pub(crate) name: String,
}

/// Legacy macro attribute/content protocol for backward compatibility.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LegacyProtocol {
    /// Attribute bindings: (attribute_name, local_var_name, is_expr)
    pub(crate) attributes: Vec<(String, String, bool)>,
    /// Content binding: (local_var_name, optional_head_filter)
    pub(crate) content: Option<(String, Option<String>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MacroDefinition {
    pub(crate) module_name: String,
    pub(crate) name: String,
    /// New explicit parameter protocol (Step 2)
    pub(crate) params: Option<Vec<MacroParam>>,
    /// Legacy attribute/content protocol for backward compatibility
    pub(crate) legacy_protocol: Option<LegacyProtocol>,
    pub(crate) body: Vec<FormItem>,
}

impl ExpandEnv {
    pub(crate) fn with_phase(mut self, phase: CompilePhase) -> Self {
        self.phase = Some(phase);
        self
    }

    pub(crate) fn begin_module(
        &mut self,
        module_name: Option<String>,
        source_name: Option<String>,
    ) -> Result<(), String> {
        if let Some(module_name) = &module_name
            && self.program.modules.contains_key(module_name)
        {
            return Err(format!("duplicate module declaration `{module_name}`"));
        }
        self.phase = Some(CompilePhase::Module);
        self.source_name = source_name;
        self.module = ModuleState {
            module_name,
            imports: Vec::new(),
            requires: Vec::new(),
            aliases: BTreeMap::new(),
            child_aliases: BTreeMap::new(),
            const_decls: BTreeMap::new(),
            exports: ModuleExports::default(),
            children: Vec::new(),
            locals: LocalScope::default(),
        };
        Ok(())
    }

    pub(crate) fn finish_module(&mut self) {
        if let Some(module_name) = self.module.module_name.clone() {
            self.program.module_order.push(module_name.clone());
            self.program
                .modules
                .insert(module_name, self.module.clone());
        }
    }

    pub(crate) fn set_module_children(&mut self, children: Vec<Form>) {
        self.module.children = children;
    }

    pub(crate) fn begin_script(&mut self) {
        self.phase = Some(CompilePhase::Script);
        self.module.locals = LocalScope::default();
    }

    pub(crate) fn enter_statement(&mut self) {
        self.phase = Some(CompilePhase::Statement);
    }

    pub(crate) fn add_import(&mut self, import_name: impl Into<String>) {
        self.module.imports.push(import_name.into());
    }

    pub(crate) fn add_require(&mut self, require_name: impl Into<String>) {
        self.module.requires.push(require_name.into());
    }

    pub(crate) fn add_alias(
        &mut self,
        alias_name: impl Into<String>,
        module_name: impl Into<String>,
    ) -> Result<(), String> {
        let alias_name = alias_name.into();
        let module_name = module_name.into();
        match self.module.aliases.get(&alias_name) {
            Some(existing) if existing != &module_name => Err(format!(
                "alias `{alias_name}` already points to `{existing}`"
            )),
            Some(_) => Ok(()),
            None => {
                self.module.aliases.insert(alias_name, module_name);
                Ok(())
            }
        }
    }

    pub(crate) fn add_child_alias(
        &mut self,
        alias_name: impl Into<String>,
        module_name: impl Into<String>,
    ) -> Result<(), String> {
        let alias_name = alias_name.into();
        let module_name = module_name.into();
        self.add_alias(alias_name.clone(), module_name.clone())?;
        self.module.child_aliases.insert(alias_name, module_name);
        Ok(())
    }

    pub(crate) fn add_local(&mut self, name: impl Into<String>) {
        self.module.locals.insert(name.into());
    }

    pub(crate) fn add_const_decl(
        &mut self,
        name: impl Into<String>,
        declared_type: DeclaredType,
        raw_expr: Option<String>,
    ) {
        self.module.const_decls.insert(
            name.into(),
            PendingConstDecl {
                declared_type,
                raw_expr,
            },
        );
    }

    pub(crate) fn declare_const(&mut self, name: impl Into<String>, exported: bool) -> bool {
        self.module.exports.consts.insert(name.into(), exported)
    }

    pub(crate) fn declare_script(&mut self, name: impl Into<String>, exported: bool) -> bool {
        self.module.exports.scripts.insert(name.into(), exported)
    }

    pub(crate) fn declare_function(&mut self, name: impl Into<String>, exported: bool) -> bool {
        self.module.exports.functions.insert(name.into(), exported)
    }

    pub(crate) fn declare_var(&mut self, name: impl Into<String>, exported: bool) -> bool {
        self.module.exports.vars.insert(name.into(), exported)
    }

    pub(crate) fn resolve_macro(&self, name: &str) -> Option<&MacroDefinition> {
        let current_module = self.module.module_name.as_deref();
        self.program
            .resolve_macro(current_module, &self.module.requires, name)
    }

    pub(crate) fn reserve_macro_invocation_seed(&mut self) -> usize {
        let module_name = self
            .module
            .module_name
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string());
        let counter = self
            .macro_invocation_counters
            .entry(module_name)
            .or_insert(0);
        *counter += 1;
        *counter
    }

    /// Push the use caller context (set caller module for conflict detection).
    /// Called before expanding a `use` macro so that injected public members
    /// can be checked against the caller's existing exports.
    pub(crate) fn push_use_caller(&mut self) {
        if self.use_caller_module.is_none() {
            self.use_caller_module = self.module.module_name.clone();
        }
    }

    /// Pop the use caller context after `use` macro expansion completes.
    pub(crate) fn pop_use_caller(&mut self) {
        self.use_caller_module = None;
    }

    /// Check if a public member name already exists in the caller's exports.
    /// Called when `use` macro tries to inject a public member.
    ///
    /// Checks both `env.module` (the in-progress current module) and
    /// `program.modules` (completed modules) to handle the case where
    /// `use` is called from within the caller module itself.
    pub(crate) fn caller_exports_has(&self, name: &str) -> bool {
        if let Some(ref caller) = self.use_caller_module {
            // First check: is the caller the current module being compiled?
            // During compilation, the current module lives in env.module, not yet in program.modules.
            if self.module.module_name.as_deref() == Some(caller) {
                let exports = &self.module.exports;
                return exports.consts.contains_declared(name)
                    || exports.functions.contains_declared(name)
                    || exports.scripts.contains_declared(name)
                    || exports.vars.contains_declared(name);
            }

            // Second check: is the caller a completed module in program.modules?
            if let Some(module_state) = self.program.modules.get(caller) {
                let exports = &module_state.exports;
                return exports.consts.contains_declared(name)
                    || exports.functions.contains_declared(name)
                    || exports.scripts.contains_declared(name)
                    || exports.vars.contains_declared(name);
            }
        }
        false
    }
}

impl ProgramState {
    pub(crate) fn register_macro(&mut self, definition: MacroDefinition) -> Result<(), String> {
        let module_macros = self
            .module_macros
            .entry(definition.module_name.clone())
            .or_default();
        if module_macros.contains_key(&definition.name) {
            return Err(format!(
                "duplicate macro declaration `{}.{}`",
                definition.module_name, definition.name
            ));
        }
        module_macros.insert(definition.name.clone(), definition);
        Ok(())
    }

    pub(crate) fn resolve_macro<'a>(
        &'a self,
        current_module: Option<&str>,
        imports: &[String],
        name: &str,
    ) -> Option<&'a MacroDefinition> {
        if let Some(definition) = current_module
            .and_then(|module_name| self.module_macros.get(module_name))
            .and_then(|macros| macros.get(name))
        {
            return Some(definition);
        }

        for import in imports.iter().rev() {
            if let Some(definition) = self
                .module_macros
                .get(import)
                .and_then(|macros| macros.get(name))
            {
                return Some(definition);
            }
        }

        if current_module != Some("kernel") && !imports.iter().any(|import| import == "kernel") {
            return self
                .module_macros
                .get("kernel")
                .and_then(|macros| macros.get(name));
        }

        None
    }
}

impl ModuleMembers {
    pub(crate) fn insert(&mut self, name: String, exported: bool) -> bool {
        if !self.declared.insert(name.clone()) {
            return false;
        }
        if exported {
            self.exported.insert(name);
        }
        true
    }

    pub(crate) fn contains_declared(&self, name: &str) -> bool {
        self.declared.contains(name)
    }

    pub(crate) fn contains_exported(&self, name: &str) -> bool {
        self.exported.contains(name)
    }

    pub(crate) fn declared_names(&self) -> BTreeSet<String> {
        self.declared.clone()
    }
}
