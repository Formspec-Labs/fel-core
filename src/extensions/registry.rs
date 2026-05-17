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

/// Result of [`ExtensionRegistry::call`].
#[derive(Debug, Clone, PartialEq)]
pub enum ExtensionCallOutcome {
    /// No extension registered under this name.
    NotFound,
    /// Extension invoked (or null-propagated without invoking).
    Ok(TypeValue),
    /// Argument count outside registered bounds; host should record a diagnostic and yield null.
    ArityMismatch {
        /// Extension name.
        name: String,
        /// Registered minimum arity.
        min_args: usize,
        /// Registered maximum arity, if bounded.
        max_args: Option<usize>,
        /// Supplied argument count.
        got: usize,
    },
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
    pub fn get(&self, name: &str) -> Option<&ExtensionFunc> {
        self.extensions.get(name)
    }

    /// True if `name` is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.extensions.contains_key(name)
    }

    /// Call an extension function with null propagation.
    ///
    /// Returns [`ExtensionCallOutcome::NotFound`] if the extension is not registered.
    /// Returns [`ExtensionCallOutcome::ArityMismatch`] when `args.len()` is outside
    /// the bounds recorded at registration (caller should emit the message and yield null).
    pub fn call(&self, name: &str, args: &[TypeValue]) -> ExtensionCallOutcome {
        let Some(ext) = self.extensions.get(name) else {
            return ExtensionCallOutcome::NotFound;
        };

        let len = args.len();
        if len < ext.min_args || ext.max_args.is_some_and(|max| len > max) {
            return ExtensionCallOutcome::ArityMismatch {
                name: name.to_string(),
                min_args: ext.min_args,
                max_args: ext.max_args,
                got: len,
            };
        }

        if args.iter().any(|a| a.is_null()) {
            return ExtensionCallOutcome::Ok(TypeValue::Null);
        }

        ExtensionCallOutcome::Ok((ext.func)(args))
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
