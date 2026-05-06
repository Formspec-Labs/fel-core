//! Extension function registry (`ExtensionRegistry`) and registration errors.
#![allow(clippy::missing_docs_in_private_items)]

use std::collections::HashMap;

use crate::types::Value as TypeValue;

use super::catalog::{BUILTIN_FUNCTIONS, RESERVED_WORDS};
use super::types::ExtensionFunc;

/// Registry of extension functions.
pub struct ExtensionRegistry {
    extensions: HashMap<String, ExtensionFunc>,
}
/// Error type for extension registration failures.
#[derive(Debug, Clone)]
pub enum ExtensionError {
    /// Registration rejected: name matches a reserved word or built-in function.
    NameConflict(String),
}

impl std::fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtensionError::NameConflict(name) => {
                write!(
                    f,
                    "cannot register extension '{name}': conflicts with reserved word or built-in function"
                )
            }
        }
    }
}

impl std::error::Error for ExtensionError {}

impl ExtensionRegistry {
    /// Empty registry (no custom extensions).
    pub fn new() -> Self {
        Self {
            extensions: HashMap::new(),
        }
    }

    /// Register an extension function.
    ///
    /// Returns an error if the name conflicts with a reserved word or built-in.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        min_args: usize,
        max_args: Option<usize>,
        func: impl Fn(&[TypeValue]) -> TypeValue + Send + Sync + 'static,
    ) -> Result<(), ExtensionError> {
        let name = name.into();

        if RESERVED_WORDS.contains(&name.as_str())
            || BUILTIN_FUNCTIONS
                .iter()
                .any(|entry| entry.name == name.as_str())
        {
            return Err(ExtensionError::NameConflict(name));
        }

        self.extensions.insert(
            name.clone(),
            ExtensionFunc {
                name: name.clone(),
                min_args,
                max_args,
                func: Box::new(func),
            },
        );
        Ok(())
    }

    /// Look up an extension function by name.
    /// Lookup registered extension by name.
    pub fn get(&self, name: &str) -> Option<&ExtensionFunc> {
        self.extensions.get(name)
    }

    /// Check if a name is registered.
    /// True if `name` is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.extensions.contains_key(name)
    }

    /// Call an extension function with null propagation.
    ///
    /// If any argument is null, returns null without calling the function.
    /// Returns None if the extension is not found.
    /// Invoke extension if present; returns `None` if unknown (caller may treat as undefined function).
    pub fn call(&self, name: &str, args: &[TypeValue]) -> Option<TypeValue> {
        let ext = self.extensions.get(name)?;

        // Null propagation: any null arg → null result
        if args.iter().any(|a| a.is_null()) {
            return Some(TypeValue::Null);
        }

        Some((ext.func)(args))
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
