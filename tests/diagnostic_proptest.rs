//! Phase 3 P0 proptest #3 — Diagnostic / DiagnosticKind / has_error_diagnostics / reject_undefined_functions / undefined_function_names_from_diagnostics / fel_diagnostics_to_json_value. See thoughts/2026-05-23-phase-3-ci-gate-design.md.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{
    Diagnostic, DiagnosticKind, Severity, fel_diagnostics_to_json_value, has_error_diagnostics,
    reject_undefined_functions, undefined_function_names_from_diagnostics,
};
use proptest::prelude::*;

// ---- strategies ----------------------------------------------------------

/// Short alphanumeric identifier suitable for function / field names. The first
/// char is alphabetic so the string never collides with the legacy
/// `"undefined function: "` message prefix when used as a free-form message.
fn arb_ident() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9_]{0,7}".prop_map(String::from)
}

/// Free-form message that intentionally never begins with
/// `"undefined function: "`. Keeps property #3 / #4 clean by ensuring the
/// legacy message-prefix extraction branch only fires when we explicitly
/// build an `UndefinedFunction` kind.
fn arb_safe_message() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 ]{0,16}".prop_map(String::from)
}

fn arb_severity() -> impl Strategy<Value = Severity> {
    prop_oneof![
        Just(Severity::Error),
        Just(Severity::Warning),
        Just(Severity::Info),
    ]
}

prop_compose! {
    fn arb_undefined_function()(name in arb_ident()) -> DiagnosticKind {
        DiagnosticKind::UndefinedFunction { name }
    }
}

prop_compose! {
    fn arb_type_mismatch()(
        fn_name in arb_ident(),
        expected in arb_ident(),
        got in arb_ident(),
    ) -> DiagnosticKind {
        DiagnosticKind::TypeMismatch { fn_name, expected, got }
    }
}

prop_compose! {
    fn arb_arity_mismatch()(
        name in arb_ident(),
        min_args in 0usize..6,
        max_args in proptest::option::of(0usize..6),
        got in 0usize..6,
    ) -> DiagnosticKind {
        DiagnosticKind::ArityMismatch { name, min_args, max_args, got }
    }
}

fn arb_diagnostic_kind() -> impl Strategy<Value = DiagnosticKind> {
    prop_oneof![
        arb_undefined_function(),
        arb_type_mismatch(),
        arb_arity_mismatch(),
    ]
}

prop_compose! {
    fn arb_diagnostic()(
        severity in arb_severity(),
        message in arb_safe_message(),
        kind in proptest::option::of(arb_diagnostic_kind()),
        code in proptest::option::of(arb_ident()),
        span in proptest::option::of((0usize..32, 0usize..32)),
    ) -> Diagnostic {
        Diagnostic {
            severity,
            message,
            code,
            kind,
            span: span.map(|(a, b)| {
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                lo..hi
            }),
        }
    }
}

fn arb_diagnostics() -> impl Strategy<Value = Vec<Diagnostic>> {
    proptest::collection::vec(arb_diagnostic(), 0..12)
}

// ---- helpers (exhaustive matches) ---------------------------------------

/// Camel-case top-level key the JSON encoder emits for each `DiagnosticKind`
/// variant. Exhaustive match — adding a new variant fails compilation here.
fn expected_kind_camel_key(kind: &DiagnosticKind) -> &'static str {
    match kind {
        DiagnosticKind::UndefinedFunction { .. } => "undefinedFunction",
        DiagnosticKind::TypeMismatch { .. } => "typeMismatch",
        DiagnosticKind::ArityMismatch { .. } => "arityMismatch",
    }
}

/// Returns `true` iff this diagnostic carries an `UndefinedFunction` kind with
/// a non-blank trimmed name (matches the structured branch in
/// `undefined_function_names_from_diagnostics`). Exhaustive match.
fn structured_undefined_name(d: &Diagnostic) -> Option<String> {
    match &d.kind {
        Some(DiagnosticKind::UndefinedFunction { name }) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Some(DiagnosticKind::TypeMismatch { .. }) | Some(DiagnosticKind::ArityMismatch { .. }) => {
            None
        }
        None => None,
    }
}

/// Mirrors the message-prefix fallback branch in
/// `undefined_function_names_from_diagnostics`.
fn message_prefix_undefined_name(d: &Diagnostic) -> Option<String> {
    d.message
        .strip_prefix("undefined function: ")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Hand-rolled mirror of `undefined_function_names_from_diagnostics` used as
/// the property-test oracle.
fn oracle_undefined_names(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .filter_map(|d| structured_undefined_name(d).or_else(|| message_prefix_undefined_name(d)))
        .collect()
}

// ---- properties ----------------------------------------------------------

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        ..Default::default()
    })]

    /// Property #1 — `fel_diagnostics_to_json_value` returns a JSON array of
    /// the same length as the input, and every entry whose source diagnostic
    /// carries a `kind` has a `"kind"` JSON key whose inner object has a
    /// single top-level key matching the camelCase variant name. Entries
    /// without a structured kind must not emit a `"kind"` key.
    #[test]
    fn json_roundtrip_kind_key_matches_variant(diagnostics in arb_diagnostics()) {
        let json = fel_diagnostics_to_json_value(&diagnostics);

        // Always an array.
        let arr = json.as_array().expect("fel_diagnostics_to_json_value emits an array");

        // Length preserved.
        prop_assert_eq!(arr.len(), diagnostics.len());

        for (d, entry) in diagnostics.iter().zip(arr.iter()) {
            let obj = entry.as_object().expect("each entry is a JSON object");

            match &d.kind {
                Some(kind) => {
                    let kind_value = obj.get("kind").expect("kind present when d.kind is Some");
                    let kind_obj = kind_value.as_object().expect("kind value is an object");
                    let expected_key = expected_kind_camel_key(kind);
                    prop_assert_eq!(kind_obj.len(), 1);
                    prop_assert!(
                        kind_obj.contains_key(expected_key),
                        "expected kind key {} present in {:?}",
                        expected_key,
                        kind_obj
                    );
                }
                None => {
                    prop_assert!(
                        !obj.contains_key("kind"),
                        "kind absent when d.kind is None, got {:?}",
                        obj
                    );
                }
            }
        }
    }

    /// Property #2 — `has_error_diagnostics` returns true iff at least one
    /// entry has `Severity::Error`. Compared against a hand-rolled filter.
    #[test]
    fn has_error_diagnostics_matches_filter(diagnostics in arb_diagnostics()) {
        let actual = has_error_diagnostics(&diagnostics);
        let oracle = diagnostics.iter().any(|d| matches!(d.severity, Severity::Error));
        prop_assert_eq!(actual, oracle);
    }

    /// Property #3 — `undefined_function_names_from_diagnostics` extracts
    /// every `UndefinedFunction`-kind name (preserving input order) and is
    /// equal to a hand-rolled oracle that mirrors the structured-first /
    /// message-prefix-fallback contract in `error.rs`.
    #[test]
    fn undefined_function_names_match_oracle(diagnostics in arb_diagnostics()) {
        let actual = undefined_function_names_from_diagnostics(&diagnostics);
        let oracle = oracle_undefined_names(&diagnostics);
        prop_assert_eq!(actual, oracle);
    }

    /// Property #4a — `reject_undefined_functions` returns `Ok(())` when no
    /// `UndefinedFunction` diagnostics are present. We generate non-undefined
    /// kinds only (TypeMismatch / ArityMismatch / None) so the message-prefix
    /// fallback never fires under `arb_safe_message`.
    #[test]
    fn reject_ok_when_no_undefined_function(
        diagnostics in proptest::collection::vec(
            (
                arb_severity(),
                arb_safe_message(),
                proptest::option::of(prop_oneof![arb_type_mismatch(), arb_arity_mismatch()]),
            ).prop_map(|(severity, message, kind)| Diagnostic {
                severity,
                message,
                code: None,
                kind,
                span: None,
            }),
            0..8,
        )
    ) {
        let result = reject_undefined_functions(&diagnostics);
        prop_assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    /// Property #4b — `reject_undefined_functions` returns `Err` whenever at
    /// least one `UndefinedFunction` diagnostic is present (the planted name
    /// must appear in the error string).
    #[test]
    fn reject_err_when_undefined_function_present(
        prefix in arb_diagnostics(),
        planted_name in "[a-zA-Z][a-zA-Z0-9_]{0,7}",
        suffix in arb_diagnostics(),
    ) {
        let mut diagnostics = prefix;
        diagnostics.push(Diagnostic::undefined_function(planted_name.clone()));
        diagnostics.extend(suffix);

        let err = reject_undefined_functions(&diagnostics)
            .expect_err("at least one UndefinedFunction → Err");
        prop_assert!(
            err.contains(&planted_name),
            "error string {:?} should contain planted name {:?}",
            err,
            planted_name
        );
    }
}
