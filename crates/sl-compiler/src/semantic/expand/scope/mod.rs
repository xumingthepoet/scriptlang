mod module_scope;
mod scope_impl;

pub(crate) use module_scope::ModuleScope;
pub(crate) use scope_impl::{ConstCatalog, QualifiedConstLookup, ScopeResolver};
