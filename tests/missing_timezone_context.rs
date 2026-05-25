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
    Date, Diagnostic, DiagnosticKind, FormspecEnvironment, MapEnvironment,
    MissingTimezoneContextError, MissingTimezoneContextReason, Severity, Value, evaluate, parse,
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
    // Pin the actual payload (per arch-review NIT) so a silent default-clock
    // regression fails loudly here, not in a downstream proptest shrink.
    let env = MapEnvironment::new(); // default-populated clock pinned to 2026-03-20
    let expr = parse("today()").unwrap();
    let result = evaluate(&expr, &env);

    assert_eq!(
        result.value,
        Value::Date(Date::Date {
            year: 2026,
            month: 3,
            day: 20,
        }),
        "today() with the pinned default clock must yield 2026-03-20"
    );
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

// ── MultiCalendarConflict — closed-set second variant ─────────────
//
// No in-tree Environment impl produces this today (per the design's
// "pre-emptive closed-set" stance per arch-review NIT N2); these tests
// lock the user-visible Display + JSON wire shapes so a future
// business-calendar host inherits a stable target.

#[test]
fn multi_calendar_conflict_display_includes_calendar_list() {
    let err =
        MissingTimezoneContextError::multi_calendar_conflict(vec!["us-fed".into(), "ca-on".into()]);
    let msg = err.to_string();
    assert!(
        msg.contains("us-fed") && msg.contains("ca-on"),
        "Display must surface conflicting calendar IDs, got: {msg}"
    );
}

#[test]
fn multi_calendar_conflict_json_shape_js_camel() {
    use fel_core::{JsonWireStyle, fel_diagnostics_to_json_value_styled};
    use serde_json::json;
    let err = MissingTimezoneContextError::multi_calendar_conflict(vec![
        "us-fed".into(),
        "uk-bank".into(),
    ]);
    let diag = Diagnostic::missing_timezone_context("today", err);
    let json_arr =
        fel_diagnostics_to_json_value_styled(std::slice::from_ref(&diag), JsonWireStyle::JsCamel);
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
                "reason": "multi_calendar_conflict",
                "calendars": ["us-fed", "uk-bank"]
            }
        }),
        "unexpected camelCase shape for multi_calendar_conflict: {kind}"
    );
}

#[test]
fn multi_calendar_conflict_json_shape_python_snake() {
    use fel_core::{JsonWireStyle, fel_diagnostics_to_json_value_styled};
    use serde_json::json;
    let err = MissingTimezoneContextError::multi_calendar_conflict(vec![
        "us-fed".into(),
        "uk-bank".into(),
    ]);
    let diag = Diagnostic::missing_timezone_context("now", err);
    let json_arr = fel_diagnostics_to_json_value_styled(
        std::slice::from_ref(&diag),
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
                "reason": "multi_calendar_conflict",
                "calendars": ["us-fed", "uk-bank"]
            }
        }),
        "unexpected snake_case shape for multi_calendar_conflict: {kind}"
    );
}
