//! ADR 0069 D-6 — `today()` and `now()` MUST hard-refuse when the host provides
//! no timezone context. Pre-ADR behavior silently returned `Value::Null` with no
//! diagnostic; this fixture pins the new contract:
//!
//! 1. Evaluation returns `Value::Null` (FEL has no exception channel).
//! 2. The evaluator emits a structured `Diagnostic` whose `kind` is
//!    `DiagnosticKind::MissingTimezoneContext`.
//! 3. Hosts configuring a clock (timezone-equivalent context) get the date.
//!
//! These tests are intentionally aggressive about asserting on the structured
//! diagnostic kind — the entire reason for this change is that callers must be
//! able to detect the refusal programmatically without parsing message strings.

use fel_core::{
    Diagnostic, DiagnosticKind, FormspecEnvironment, MapEnvironment, MissingTimezoneContextReason,
    Severity, Value, evaluate, parse,
};

fn missing_tz_diagnostic(diagnostics: &[Diagnostic]) -> Option<&Diagnostic> {
    diagnostics
        .iter()
        .find(|d| matches!(d.kind, Some(DiagnosticKind::MissingTimezoneContext { .. })))
}

#[test]
fn today_with_no_timezone_context_emits_structured_diagnostic() {
    let env = MapEnvironment::new().with_current_datetime(None);
    let expr = parse("today()").unwrap();
    let result = evaluate(&expr, &env);

    assert_eq!(
        result.value,
        Value::Null,
        "today() must return null when no timezone context is configured"
    );
    let diag = missing_tz_diagnostic(&result.diagnostics)
        .expect("today() must emit DiagnosticKind::MissingTimezoneContext when no clock is set");
    assert_eq!(diag.severity, Severity::Error);
    match diag.kind.as_ref().unwrap() {
        DiagnosticKind::MissingTimezoneContext { reason, fn_name } => {
            assert_eq!(fn_name, "today");
            assert!(matches!(
                reason,
                MissingTimezoneContextReason::NotConfigured
            ));
        }
        other => panic!("unexpected diagnostic kind: {other:?}"),
    }
}

#[test]
fn now_with_no_timezone_context_emits_structured_diagnostic() {
    let env = MapEnvironment::new().with_current_datetime(None);
    let expr = parse("now()").unwrap();
    let result = evaluate(&expr, &env);

    assert_eq!(result.value, Value::Null);
    let diag = missing_tz_diagnostic(&result.diagnostics)
        .expect("now() must emit DiagnosticKind::MissingTimezoneContext when no clock is set");
    match diag.kind.as_ref().unwrap() {
        DiagnosticKind::MissingTimezoneContext { reason, fn_name } => {
            assert_eq!(fn_name, "now");
            assert!(matches!(
                reason,
                MissingTimezoneContextReason::NotConfigured
            ));
        }
        other => panic!("unexpected diagnostic kind: {other:?}"),
    }
}

#[test]
fn formspec_environment_without_now_emits_structured_diagnostic() {
    // FormspecEnvironment::new() leaves current_datetime as None — the path
    // that ADR 0069 calls "the implicit-server-timezone fallback".
    let env = FormspecEnvironment::new();
    let expr = parse("today()").unwrap();
    let result = evaluate(&expr, &env);

    assert_eq!(result.value, Value::Null);
    assert!(
        missing_tz_diagnostic(&result.diagnostics).is_some(),
        "FormspecEnvironment with no clock must emit MissingTimezoneContext, got: {:?}",
        result.diagnostics
    );
}

#[test]
fn today_with_configured_clock_succeeds_without_diagnostic() {
    // Sanity-check the happy path: providing a clock satisfies the contract.
    let env = MapEnvironment::new(); // default-populated clock pinned to 2026-03-20
    let expr = parse("today()").unwrap();
    let result = evaluate(&expr, &env);

    assert!(matches!(result.value, Value::Date(_)));
    assert!(
        missing_tz_diagnostic(&result.diagnostics).is_none(),
        "today() with clock must NOT emit MissingTimezoneContext, got: {:?}",
        result.diagnostics
    );
}

#[test]
fn missing_timezone_diagnostic_json_shape_js_camel() {
    use fel_core::{JsonWireStyle, fel_diagnostics_to_json_value_styled};
    use serde_json::json;
    let env = MapEnvironment::new().with_current_datetime(None);
    let expr = parse("today()").unwrap();
    let result = evaluate(&expr, &env);
    let diag = missing_tz_diagnostic(&result.diagnostics).unwrap();
    let json_arr =
        fel_diagnostics_to_json_value_styled(std::slice::from_ref(diag), JsonWireStyle::JsCamel);
    let kind = json_arr
        .as_array()
        .unwrap()
        .first()
        .unwrap()
        .get("kind")
        .expect("kind present");
    assert_eq!(
        kind,
        &json!({
            "missingTimezoneContext": {
                "fnName": "today",
                "reason": "not_configured"
            }
        }),
        "unexpected camelCase shape: {kind}"
    );
}

#[test]
fn missing_timezone_diagnostic_json_shape_python_snake() {
    use fel_core::{JsonWireStyle, fel_diagnostics_to_json_value_styled};
    use serde_json::json;
    let env = MapEnvironment::new().with_current_datetime(None);
    let expr = parse("now()").unwrap();
    let result = evaluate(&expr, &env);
    let diag = missing_tz_diagnostic(&result.diagnostics).unwrap();
    let json_arr = fel_diagnostics_to_json_value_styled(
        std::slice::from_ref(diag),
        JsonWireStyle::PythonSnake,
    );
    let kind = json_arr
        .as_array()
        .unwrap()
        .first()
        .unwrap()
        .get("kind")
        .expect("kind present");
    assert_eq!(
        kind,
        &json!({
            "missing_timezone_context": {
                "fn_name": "now",
                "reason": "not_configured"
            }
        }),
        "unexpected snake_case shape: {kind}"
    );
}
