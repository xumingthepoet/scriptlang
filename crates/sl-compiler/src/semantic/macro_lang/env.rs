//! Compile-time environment for macro evaluation.

use super::CtValue;
use std::collections::HashMap;

/// Environment for compile-time macro evaluation.
pub struct CtEnv {
    /// Local variable bindings
    locals: HashMap<String, CtValue>,
}

impl CtEnv {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            locals: HashMap::new(),
        }
    }

    /// Get a local variable value.
    pub fn get(&self, name: &str) -> Option<&CtValue> {
        self.locals.get(name)
    }

    /// Set a local variable value (creates if not exists).
    pub fn set(&mut self, name: String, value: CtValue) {
        self.locals.insert(name, value);
    }

    /// Update an existing local variable (returns error if not found).
    pub fn update(&mut self, name: &str, value: CtValue) -> Result<(), String> {
        if self.locals.contains_key(name) {
            self.locals.insert(name.to_string(), value);
            Ok(())
        } else {
            Err(format!("Undefined variable: {}", name))
        }
    }

    /// Create a child environment (for nested scopes).
    #[allow(dead_code)]
    pub fn child(&self) -> Self {
        Self {
            locals: self.locals.clone(),
        }
    }
}
