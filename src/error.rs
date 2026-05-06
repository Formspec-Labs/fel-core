//! FEL error types and diagnostic messages.
use std::fmt;

/// Failure from [`crate::parse`] or fatal-style evaluation errors surfaced as `Err`.
#[derive(Debug, Clone)]
pub enum Error {
    /// Lex/parse failure (message from lexer or parser).
    Parse(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

/// A non-fatal diagnostic recorded during evaluation.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Severity for hosts and JSON wire encoding.
    pub severity: Severity,
    /// Human-readable explanation.
    pub message: String,
    /// Machine-readable category for robust downstream handling.
    pub kind: Option<DiagnosticKind>,
}

/// Diagnostic severity for tooling and JSON wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Blocking / error-level.
    Error,
    /// Warning-level.
    Warning,
    /// Informational.
    Info,
}

/// Machine-readable diagnostic categories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// Function name could not be resolved in builtins or extension registry.
    UndefinedFunction {
        /// The unresolved function identifier as written in the expression.
        name: String,
    },
}

impl Severity {
    /// Wire string used in JSON diagnostics (`error` / `warning` / `info`).
    pub fn as_wire_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

impl Diagnostic {
    /// Build an error-severity diagnostic.
    pub fn error(msg: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            message: msg.into(),
            kind: None,
        }
    }

    /// Build a warning-severity diagnostic.
    pub fn warning(msg: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            message: msg.into(),
            kind: None,
        }
    }

    /// Build a structured undefined-function diagnostic.
    pub fn undefined_function(name: impl Into<String>) -> Self {
        let name = name.into();
        Diagnostic {
            severity: Severity::Error,
            message: format!("undefined function: {name}"),
            kind: Some(DiagnosticKind::UndefinedFunction { name }),
        }
    }
}

/// Names from `undefined function: …` diagnostics (host bindings reject these as unsupported).
pub fn undefined_function_names_from_diagnostics(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .filter_map(|d| {
            if let Some(DiagnosticKind::UndefinedFunction { name }) = &d.kind {
                if !name.trim().is_empty() {
                    return Some(name.trim().to_string());
                }
            }
            d.message
                .strip_prefix("undefined function: ")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .collect()
}

/// Returns `true` if any diagnostic has error severity.
pub fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

/// Returns `Err` when any undefined-function diagnostic is present (WASM / strict hosts).
pub fn reject_undefined_functions(diagnostics: &[Diagnostic]) -> Result<(), String> {
    let names = undefined_function_names_from_diagnostics(diagnostics);
    if names.is_empty() {
        Ok(())
    } else {
        Err(format!("Unsupported FEL function: {}", names.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_undefined_function_names() {
        let diagnostics = vec![
            Diagnostic::undefined_function("foo"),
            Diagnostic::warning("other"),
            Diagnostic::undefined_function("bar"),
        ];
        assert_eq!(
            undefined_function_names_from_diagnostics(&diagnostics),
            vec!["foo".to_string(), "bar".to_string()]
        );
    }

    #[test]
    fn detects_error_severity_presence() {
        let diagnostics = vec![Diagnostic::warning("warn"), Diagnostic::error("err")];
        assert!(has_error_diagnostics(&diagnostics));
    }

    #[test]
    fn reject_undefined_functions_returns_error() {
        let diagnostics = vec![Diagnostic::undefined_function("randomFn")];
        let err = reject_undefined_functions(&diagnostics).expect_err("should reject");
        assert!(err.contains("randomFn"));
    }

    #[test]
    fn extracts_undefined_names_from_legacy_message_prefix() {
        let diagnostics = vec![Diagnostic::error("undefined function: legacyFn")];
        assert_eq!(
            undefined_function_names_from_diagnostics(&diagnostics),
            vec!["legacyFn".to_string()]
        );
    }

    #[test]
    fn severity_wire_strings_are_stable() {
        assert_eq!(Severity::Error.as_wire_str(), "error");
        assert_eq!(Severity::Warning.as_wire_str(), "warning");
        assert_eq!(Severity::Info.as_wire_str(), "info");
    }
}
