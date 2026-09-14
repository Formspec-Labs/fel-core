//! Extension function registry (`ExtensionRegistry`) and registration errors.
#![allow(clippy::missing_docs_in_private_items)]

use std::collections::HashMap;

use crate::types::Value as TypeValue;

use super::catalog::{BUILTIN_FUNCTIONS, RESERVED_WORDS};
use super::types::ExtensionFunc;

/// Host-supplied FEL extension functions (Core §3.12), consulted for non-builtin names.
///
/// The port lets a host back extensions with what its runtime has: Rust closures
/// ([`ExtensionRegistry`]), JavaScript functions across WASM, or Python callables.
/// Implementations need not be `Send` or `Sync`. The evaluator owns the call contract
/// through [`call_extension`] (arity bounds, null propagation, a failed call becoming
/// a diagnostic), so an implementation only looks up and invokes.
pub trait ExtensionFunctions {
    /// Arity bounds `(min_args, max_args)` when `name` is registered; `None` otherwise.
    fn arity(&self, name: &str) -> Option<(usize, Option<usize>)>;

    /// Invokes extension `name` with arity-checked, non-null `args`.
    ///
    /// # Errors
    ///
    /// A message when the host implementation failed (threw, raised, or returned an
    /// unrepresentable value). Core §3.12 forbids propagating it: the evaluator yields
    /// `null` and records an error diagnostic.
    fn invoke(&self, name: &str, args: &[TypeValue]) -> Result<TypeValue, String>;
}

/// Checks that `name` may be registered: neither a reserved word nor a built-in (Core §3.12).
///
/// # Errors
///
/// [`ExtensionError::NameConflict`] when `name` collides.
pub fn check_extension_name(name: &str) -> Result<(), ExtensionError> {
    if RESERVED_WORDS.contains(&name) || BUILTIN_FUNCTIONS.iter().any(|entry| entry.name == name) {
        return Err(ExtensionError::NameConflict(name.to_string()));
    }
    Ok(())
}

/// Calls extension `name` under Core §3.12: arity bounds, null propagation, totality.
///
/// A `null` argument short-circuits to `null` without invoking the host; a host
/// failure becomes [`ExtensionCallOutcome::Failed`], never a panic.
pub fn call_extension(
    functions: &dyn ExtensionFunctions,
    name: &str,
    args: &[TypeValue],
) -> ExtensionCallOutcome {
    let Some((min_args, max_args)) = functions.arity(name) else {
        return ExtensionCallOutcome::NotFound;
    };

    let got = args.len();
    if got < min_args || max_args.is_some_and(|max| got > max) {
        return ExtensionCallOutcome::ArityMismatch {
            name: name.to_string(),
            min_args,
            max_args,
            got,
        };
    }

    if args.iter().any(TypeValue::is_null) {
        return ExtensionCallOutcome::Ok(TypeValue::Null);
    }

    match functions.invoke(name, args) {
        Ok(value) => ExtensionCallOutcome::Ok(value),
        Err(message) => ExtensionCallOutcome::Failed {
            name: name.to_string(),
            message,
        },
    }
}

/// Registry of extension functions backed by Rust closures.
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
    /// The host implementation failed; host should record a diagnostic and yield null.
    Failed {
        /// Extension name.
        name: String,
        /// Host failure message.
        message: String,
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
        check_extension_name(&name)?;

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
        call_extension(self, name, args)
    }
}

impl ExtensionFunctions for ExtensionRegistry {
    fn arity(&self, name: &str) -> Option<(usize, Option<usize>)> {
        self.extensions
            .get(name)
            .map(|ext| (ext.min_args, ext.max_args))
    }

    fn invoke(&self, name: &str, args: &[TypeValue]) -> Result<TypeValue, String> {
        self.extensions
            .get(name)
            .map(|ext| (ext.func)(args))
            .ok_or_else(|| format!("extension '{name}' is not registered"))
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
