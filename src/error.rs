//! FEL error types and diagnostic messages.
use std::fmt;
use std::ops::Range;

/// Lex or parse failure with optional source span (byte offsets into the expression).
///
/// [`Error`]'s [`std::fmt::Display`] output uses the `message` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Human-readable explanation.
    pub message: String,
    /// Byte range in the source, when known.
    pub span: Option<Range<usize>>,
}

impl ParseError {
    /// Parse failure with no associated span.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }

    /// Parse failure with a source span.
    #[must_use]
    pub fn with_span(span: Range<usize>, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Failure from [`crate::parse`] or fatal-style evaluation errors surfaced as `Err`.
#[derive(Debug, Clone)]
pub enum Error {
    /// Lex/parse failure (message from lexer or parser).
    Parse(ParseError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse(pe) => write!(f, "parse error: {}", pe.message),
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
    /// Stable machine-readable code for lint/UI (e.g. `FEL_SUM_REJECTS_MONEY`).
    pub code: Option<String>,
    /// Machine-readable category for robust downstream handling.
    pub kind: Option<DiagnosticKind>,
    /// Byte range in the source expression, when known.
    pub span: Option<Range<usize>>,
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
    /// Builtin or expression context expected a different runtime type.
    TypeMismatch {
        /// Builtin name or logical callee (`"if"` for ternary / keyword-if).
        fn_name: String,
        /// Expected type phrase as emitted in the human message (e.g. `"boolean"`).
        expected: String,
        /// Observed type name from [`crate::types::Value::type_name`].
        got: String,
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
            code: None,
            kind: None,
            span: None,
        }
    }

    /// Build an error-severity diagnostic with a stable [`Diagnostic::code`].
    pub fn error_coded(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            message: msg.into(),
            code: Some(code.into()),
            kind: None,
            span: None,
        }
    }

    /// Build a warning-severity diagnostic.
    pub fn warning(msg: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            message: msg.into(),
            code: None,
            kind: None,
            span: None,
        }
    }

    /// Build a structured undefined-function diagnostic.
    pub fn undefined_function(name: impl Into<String>) -> Self {
        let name = name.into();
        Diagnostic {
            severity: Severity::Error,
            message: format!("undefined function: {name}"),
            code: None,
            kind: Some(DiagnosticKind::UndefinedFunction { name }),
            span: None,
        }
    }

    /// Build a structured type-mismatch diagnostic (same message shape as runtime type errors).
    pub fn type_mismatch(
        fn_name: impl Into<String>,
        expected: impl Into<String>,
        got_type: impl Into<String>,
    ) -> Self {
        let fn_name = fn_name.into();
        let expected = expected.into();
        let got_type = got_type.into();
        Diagnostic {
            severity: Severity::Error,
            message: format!("{fn_name}: expected {expected}, got {got_type}"),
            code: None,
            kind: Some(DiagnosticKind::TypeMismatch {
                fn_name: fn_name.clone(),
                expected: expected.clone(),
                got: got_type.clone(),
            }),
            span: None,
        }
    }

    /// Attaches a source span (byte offsets into the FEL source string).
    #[must_use]
    pub fn with_span(mut self, span: Range<usize>) -> Self {
        self.span = Some(span);
        self
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

/// Evaluation diagnostics as JSON objects (default `camelCase`).
pub fn fel_diagnostics_to_json_value(diagnostics: &[Diagnostic]) -> serde_json::Value {
    fel_diagnostics_to_json_value_styled(diagnostics, crate::wire_style::JsonWireStyle::JsCamel)
}

/// Serializes the structured diagnostic kind with the selected key style.
fn diagnostic_kind_to_json(
    kind: &DiagnosticKind,
    style: crate::wire_style::JsonWireStyle,
) -> serde_json::Value {
    use crate::wire_style::JsonWireStyle;

    match kind {
        DiagnosticKind::UndefinedFunction { name } => match style {
            JsonWireStyle::JsCamel => serde_json::json!({
                "undefinedFunction": { "name": name }
            }),
            JsonWireStyle::PythonSnake => serde_json::json!({
                "undefined_function": { "name": name }
            }),
        },
        DiagnosticKind::TypeMismatch {
            fn_name,
            expected,
            got,
        } => match style {
            JsonWireStyle::JsCamel => serde_json::json!({
                "typeMismatch": {
                    "fnName": fn_name,
                    "expected": expected,
                    "got": got,
                }
            }),
            JsonWireStyle::PythonSnake => serde_json::json!({
                "type_mismatch": {
                    "fn_name": fn_name,
                    "expected": expected,
                    "got": got,
                }
            }),
        },
    }
}

/// Serializes one diagnostic into its public JSON object shape.
fn diagnostic_to_json_object(
    d: &Diagnostic,
    style: crate::wire_style::JsonWireStyle,
) -> serde_json::Value {
    use serde_json::{Map, Value};

    let mut map = Map::new();
    map.insert("message".to_string(), Value::String(d.message.clone()));
    if let Some(code) = &d.code {
        map.insert("code".to_string(), Value::String(code.clone()));
    }
    map.insert(
        "severity".to_string(),
        Value::String(d.severity.as_wire_str().to_string()),
    );
    if let Some(sp) = &d.span {
        map.insert(
            "span".to_string(),
            serde_json::json!({ "start": sp.start, "end": sp.end }),
        );
    }
    if let Some(kind) = &d.kind {
        map.insert("kind".to_string(), diagnostic_kind_to_json(kind, style));
    }
    Value::Object(map)
}

/// Evaluation diagnostics as JSON objects with configurable key style.
pub fn fel_diagnostics_to_json_value_styled(
    diagnostics: &[Diagnostic],
    style: crate::wire_style::JsonWireStyle,
) -> serde_json::Value {
    serde_json::Value::Array(
        diagnostics
            .iter()
            .map(|d| diagnostic_to_json_object(d, style))
            .collect(),
    )
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

    #[test]
    fn diagnostic_json_styled_matches_default_shape() {
        use serde_json::json;

        use crate::wire_style::JsonWireStyle;

        let diagnostics = vec![Diagnostic::error("boom")];
        let js = fel_diagnostics_to_json_value_styled(&diagnostics, JsonWireStyle::JsCamel);
        let py = fel_diagnostics_to_json_value_styled(&diagnostics, JsonWireStyle::PythonSnake);
        let default = fel_diagnostics_to_json_value(&diagnostics);

        let expected = json!([{ "message": "boom", "severity": "error" }]);
        assert_eq!(js, expected);
        assert_eq!(py, expected);
        assert_eq!(default, expected);
    }

    #[test]
    fn diagnostic_json_includes_span_when_set() {
        use serde_json::json;

        let diagnostics = vec![Diagnostic::error("bad").with_span(3..9)];
        assert_eq!(
            fel_diagnostics_to_json_value(&diagnostics),
            json!([{
                "message": "bad",
                "severity": "error",
                "span": { "start": 3, "end": 9 }
            }])
        );
    }

    #[test]
    fn diagnostic_json_includes_kind_undefined_function_js_camel() {
        use serde_json::json;

        use crate::wire_style::JsonWireStyle;

        let diagnostics = vec![Diagnostic::undefined_function("foo")];
        assert_eq!(
            fel_diagnostics_to_json_value_styled(&diagnostics, JsonWireStyle::JsCamel),
            json!([{
                "message": "undefined function: foo",
                "severity": "error",
                "kind": { "undefinedFunction": { "name": "foo" } }
            }])
        );
    }

    #[test]
    fn diagnostic_json_includes_kind_and_span_together() {
        use serde_json::json;

        let diagnostics = vec![Diagnostic::undefined_function("bar").with_span(1..5)];
        assert_eq!(
            fel_diagnostics_to_json_value(&diagnostics),
            json!([{
                "message": "undefined function: bar",
                "severity": "error",
                "span": { "start": 1, "end": 5 },
                "kind": { "undefinedFunction": { "name": "bar" } }
            }])
        );
    }

    #[test]
    fn diagnostic_json_snake_style_kind_keys() {
        use serde_json::json;

        use crate::wire_style::JsonWireStyle;

        let diagnostics = vec![Diagnostic::undefined_function("x")];
        assert_eq!(
            fel_diagnostics_to_json_value_styled(&diagnostics, JsonWireStyle::PythonSnake),
            json!([{
                "message": "undefined function: x",
                "severity": "error",
                "kind": { "undefined_function": { "name": "x" } }
            }])
        );
    }

    #[test]
    fn diagnostic_json_type_mismatch_js_camel() {
        use serde_json::json;

        use crate::wire_style::JsonWireStyle;

        let diagnostics = vec![Diagnostic::type_mismatch("if", "boolean", "number")];
        assert_eq!(
            fel_diagnostics_to_json_value_styled(&diagnostics, JsonWireStyle::JsCamel),
            json!([{
                "message": "if: expected boolean, got number",
                "severity": "error",
                "kind": { "typeMismatch": { "fnName": "if", "expected": "boolean", "got": "number" } }
            }])
        );
    }

    #[test]
    fn diagnostic_json_type_mismatch_python_snake() {
        use serde_json::json;

        use crate::wire_style::JsonWireStyle;

        let diagnostics = vec![Diagnostic::type_mismatch("sum", "array", "string")];
        assert_eq!(
            fel_diagnostics_to_json_value_styled(&diagnostics, JsonWireStyle::PythonSnake),
            json!([{
                "message": "sum: expected array, got string",
                "severity": "error",
                "kind": { "type_mismatch": { "fn_name": "sum", "expected": "array", "got": "string" } }
            }])
        );
    }
}
