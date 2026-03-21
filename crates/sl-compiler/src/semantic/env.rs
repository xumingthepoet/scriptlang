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
    pub(crate) scripts: ModuleMembers,
    pub(crate) vars: ModuleMembers,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProgramState {
    pub(crate) modules: BTreeMap<String, ModuleState>,
    pub(crate) module_order: Vec<String>,
    pub(crate) module_macros: BTreeMap<String, BTreeMap<String, Vec<MacroDefinition>>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExpandEnv {
    pub(crate) phase: Option<CompilePhase>,
    pub(crate) source_name: Option<String>,
    pub(crate) program: ProgramState,
    pub(crate) module: ModuleState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MacroDefinition {
    pub(crate) module_name: String,
    pub(crate) name: String,
    pub(crate) scope: MacroScope,
    pub(crate) body: Vec<FormItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MacroScope {
    ModuleChild,
    Statement,
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

    pub(crate) fn declare_var(&mut self, name: impl Into<String>, exported: bool) -> bool {
        self.module.exports.vars.insert(name.into(), exported)
    }

    pub(crate) fn resolve_macro(&self, name: &str, scope: MacroScope) -> Option<&MacroDefinition> {
        let current_module = self.module.module_name.as_deref();
        self.program
            .resolve_macro(current_module, &self.module.imports, name, scope)
    }
}

impl ProgramState {
    pub(crate) fn register_macro(&mut self, definition: MacroDefinition) -> Result<(), String> {
        let module_macros = self
            .module_macros
            .entry(definition.module_name.clone())
            .or_default();
        let macros = module_macros.entry(definition.name.clone()).or_default();
        if macros
            .iter()
            .any(|existing| existing.scope == definition.scope)
        {
            return Err(format!(
                "duplicate macro declaration `{}.{}` for scope `{:?}`",
                definition.module_name, definition.name, definition.scope
            ));
        }
        macros.push(definition);
        Ok(())
    }

    pub(crate) fn resolve_macro<'a>(
        &'a self,
        current_module: Option<&str>,
        imports: &[String],
        name: &str,
        scope: MacroScope,
    ) -> Option<&'a MacroDefinition> {
        if let Some(definition) = current_module
            .and_then(|module_name| self.module_macros.get(module_name))
            .and_then(|macros| macros.get(name))
            .and_then(|definitions| {
                definitions
                    .iter()
                    .find(|definition| definition.scope == scope)
            })
        {
            return Some(definition);
        }

        for import in imports.iter().rev() {
            if let Some(definition) = self
                .module_macros
                .get(import)
                .and_then(|macros| macros.get(name))
                .and_then(|definitions| {
                    definitions
                        .iter()
                        .find(|definition| definition.scope == scope)
                })
            {
                return Some(definition);
            }
        }

        if current_module != Some("kernel") && !imports.iter().any(|import| import == "kernel") {
            return self
                .module_macros
                .get("kernel")
                .and_then(|macros| macros.get(name))
                .and_then(|definitions| {
                    definitions
                        .iter()
                        .find(|definition| definition.scope == scope)
                });
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
