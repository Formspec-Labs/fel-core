//! Phase 3b Tier-2 — JSON-export styled-sibling parity proptests.
//!
//! Covers the 5 styled JSON-export functions whose default (camelCase)
//! siblings are pinned elsewhere:
//!
//! * `lexer::tokenize_to_json_value`
//! * `lexer::tokenize_to_json_value_styled`
//! * `error::fel_diagnostics_to_json_value_styled`
//!   (non-styled `fel_diagnostics_to_json_value` is already covered by
//!   `tests/diagnostic_proptest.rs::json_roundtrip_kind_key_matches_variant`)
//! * `dependencies::dependencies_to_json_value`
//! * `dependencies::dependencies_to_json_value_styled`
//!
//! Properties asserted per group (1-3 below) close the Tier-2 styled-sibling
//! gap by establishing that both wire styles produce shape-equivalent JSON
//! (same length, same primitive values) differing only in object-key casing.
//!
//! All `match style` arms are exhaustive (no `_` wildcard) so adding a new
//! `JsonWireStyle` variant breaks compilation here — forcing a deliberate
//! coverage update rather than silent drift.
//!
//! ## Feature gate
//!
//! Gated behind `#![cfg(feature = "proptest-strategies")]` for the
//! `arb_expr` strategy. Run via
//! `cargo test --test json_exports_proptest --features proptest-strategies`
//! or `make test` (which uses `--all-features`). Plain `cargo test` skips
//! this file entirely.

#![cfg(feature = "proptest-strategies")]
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::testing::strategies::arb_expr;
use fel_core::{
    Diagnostic, DiagnosticKind, JsonWireStyle, MissingTimezoneContextReason, Severity,
    builtin_function_catalog, dependencies_to_json_value, dependencies_to_json_value_styled,
    extract_dependencies, fel_diagnostics_to_json_value, fel_diagnostics_to_json_value_styled,
    print_expr, tokenize_to_json_value, tokenize_to_json_value_styled,
};
use proptest::prelude::*;

// ── exhaustive style-key helpers ────────────────────────────────────────────
//
// Each helper exhaustively matches `JsonWireStyle` (no `_` wildcard). Adding
// a new variant fails compilation here and forces a deliberate update of
// every styled-sibling property below.

/// Top-level key under which a per-token JSON object stores its kind string,
/// per `tokenize_to_json_value_styled` in `src/lexer.rs:668-692`.
fn lexer_token_type_key(style: JsonWireStyle) -> &'static str {
    match style {
        JsonWireStyle::JsCamel => "tokenType",
        JsonWireStyle::PythonSnake => "token_type",
    }
}

/// Top-level diagnostic-kind variant key (camel vs snake), mirroring
/// `diagnostic_kind_to_json` in `src/error.rs`. Exhaustive on BOTH axes.
fn diagnostic_kind_variant_key(kind: &DiagnosticKind, style: JsonWireStyle) -> &'static str {
    match (kind, style) {
        (DiagnosticKind::UndefinedFunction { .. }, JsonWireStyle::JsCamel) => "undefinedFunction",
        (DiagnosticKind::UndefinedFunction { .. }, JsonWireStyle::PythonSnake) => {
            "undefined_function"
        }
        (DiagnosticKind::TypeMismatch { .. }, JsonWireStyle::JsCamel) => "typeMismatch",
        (DiagnosticKind::TypeMismatch { .. }, JsonWireStyle::PythonSnake) => "type_mismatch",
        (DiagnosticKind::ArityMismatch { .. }, JsonWireStyle::JsCamel) => "arityMismatch",
        (DiagnosticKind::ArityMismatch { .. }, JsonWireStyle::PythonSnake) => "arity_mismatch",
        (DiagnosticKind::MissingTimezoneContext { .. }, JsonWireStyle::JsCamel) => {
            "missingTimezoneContext"
        }
        (DiagnosticKind::MissingTimezoneContext { .. }, JsonWireStyle::PythonSnake) => {
            "missing_timezone_context"
        }
    }
}

/// Expected top-level Dependencies-object keys per style (`fields` is shared
/// across both styles, the other six rename). Mirrors
/// `dependencies_to_json_value_styled` in `src/dependencies.rs:286-310`.
fn dependencies_top_level_keys(style: JsonWireStyle) -> [&'static str; 7] {
    match style {
        JsonWireStyle::JsCamel => [
            "fields",
            "contextRefs",
            "instanceRefs",
            "mipDeps",
            "hasSelfRef",
            "hasWildcard",
            "usesPrevNext",
        ],
        JsonWireStyle::PythonSnake => [
            "fields",
            "context_refs",
            "instance_refs",
            "mip_deps",
            "has_self_ref",
            "has_wildcard",
            "uses_prev_next",
        ],
    }
}

/// `(camel_key, snake_key)` pairs whose JSON values must be identical across
/// the two styles. Excludes `fields` (same key in both). Lets the parity
/// property assert value-equality under different casings.
fn dependencies_key_pairs() -> [(&'static str, &'static str); 6] {
    [
        ("contextRefs", "context_refs"),
        ("instanceRefs", "instance_refs"),
        ("mipDeps", "mip_deps"),
        ("hasSelfRef", "has_self_ref"),
        ("hasWildcard", "has_wildcard"),
        ("usesPrevNext", "uses_prev_next"),
    ]
}

// ── diagnostic strategies (inlined from tests/diagnostic_proptest.rs) ──────
//
// Re-stated here because the strategy is `fn`-private in that file and
// proptest_derive isn't available; copying keeps the dependency surface
// minimal.

fn arb_ident() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9_]{0,7}".prop_map(String::from)
}

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

fn arb_missing_tz_reason() -> impl Strategy<Value = MissingTimezoneContextReason> {
    prop_oneof![
        Just(MissingTimezoneContextReason::NotConfigured),
        proptest::collection::vec(arb_ident(), 1..4).prop_map(|calendars| {
            MissingTimezoneContextReason::MultiCalendarConflict { calendars }
        }),
    ]
}

prop_compose! {
    fn arb_missing_timezone_context()(
        fn_name in prop_oneof![Just("today".to_string()), Just("now".to_string())],
        reason in arb_missing_tz_reason(),
    ) -> DiagnosticKind {
        DiagnosticKind::MissingTimezoneContext { fn_name, reason }
    }
}

fn arb_diagnostic_kind() -> impl Strategy<Value = DiagnosticKind> {
    prop_oneof![
        arb_undefined_function(),
        arb_type_mismatch(),
        arb_arity_mismatch(),
        arb_missing_timezone_context(),
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

// ── properties ──────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 96,
        ..Default::default()
    })]

    /// Lexer JSON parity — for any AST printable to valid FEL source via
    /// `print_expr`, `tokenize_to_json_value` (camel default) and
    /// `tokenize_to_json_value_styled(_, PythonSnake)` yield arrays of the
    /// same length, with element-wise identical primitive values stored
    /// under the style-appropriate token-type key.
    #[test]
    fn lexer_json_parity_between_styles(expr in arb_expr(3, builtin_function_catalog())) {
        let src = print_expr(&expr);

        let default = tokenize_to_json_value(&src)
            .expect("tokenize_to_json_value should not fail on printed FEL");
        let camel = tokenize_to_json_value_styled(&src, JsonWireStyle::JsCamel)
            .expect("tokenize_to_json_value_styled camel should not fail");
        let snake = tokenize_to_json_value_styled(&src, JsonWireStyle::PythonSnake)
            .expect("tokenize_to_json_value_styled snake should not fail");

        // Default == camel (camel is the documented default).
        prop_assert_eq!(&default, &camel);

        let camel_arr = camel.as_array().expect("camel result is an array");
        let snake_arr = snake.as_array().expect("snake result is an array");
        prop_assert_eq!(camel_arr.len(), snake_arr.len());

        let camel_key = lexer_token_type_key(JsonWireStyle::JsCamel);
        let snake_key = lexer_token_type_key(JsonWireStyle::PythonSnake);

        for (c, s) in camel_arr.iter().zip(snake_arr.iter()) {
            let cobj = c.as_object().expect("camel element is object");
            let sobj = s.as_object().expect("snake element is object");

            // Same field count in both styles.
            prop_assert_eq!(cobj.len(), sobj.len());

            // token-type key flips, value stays identical.
            prop_assert_eq!(
                cobj.get(camel_key),
                sobj.get(snake_key),
                "token-type value must be identical under different keys"
            );
            // Camel must NOT carry the snake key, and vice versa.
            prop_assert!(!cobj.contains_key(snake_key));
            prop_assert!(!sobj.contains_key(camel_key));

            // Shared keys are byte-identical.
            for shared in ["text", "start", "end"] {
                prop_assert_eq!(cobj.get(shared), sobj.get(shared));
            }
        }
    }

    /// Diagnostic JSON styled-sibling parity — the styled function under
    /// either wire style matches the non-styled (camel-default) function
    /// modulo `kind`-variant key casing. Length and per-entry primitive
    /// values are preserved.
    #[test]
    fn diagnostics_json_styled_matches_default_modulo_kind_key(
        diagnostics in arb_diagnostics()
    ) {
        let default = fel_diagnostics_to_json_value(&diagnostics);
        let camel = fel_diagnostics_to_json_value_styled(&diagnostics, JsonWireStyle::JsCamel);
        let snake = fel_diagnostics_to_json_value_styled(&diagnostics, JsonWireStyle::PythonSnake);

        // Default is camel.
        prop_assert_eq!(&default, &camel);

        let camel_arr = camel.as_array().expect("camel diagnostics array");
        let snake_arr = snake.as_array().expect("snake diagnostics array");
        prop_assert_eq!(camel_arr.len(), diagnostics.len());
        prop_assert_eq!(snake_arr.len(), diagnostics.len());

        for ((d, c), s) in diagnostics.iter().zip(camel_arr.iter()).zip(snake_arr.iter()) {
            let cobj = c.as_object().expect("camel entry is object");
            let sobj = s.as_object().expect("snake entry is object");

            // Shared (non-kind) top-level keys are identical across styles.
            prop_assert_eq!(cobj.get("message"), sobj.get("message"));
            prop_assert_eq!(cobj.get("severity"), sobj.get("severity"));
            prop_assert_eq!(cobj.get("code"), sobj.get("code"));
            prop_assert_eq!(cobj.get("span"), sobj.get("span"));

            match &d.kind {
                None => {
                    prop_assert!(!cobj.contains_key("kind"));
                    prop_assert!(!sobj.contains_key("kind"));
                }
                Some(kind) => {
                    let camel_kind = cobj.get("kind").and_then(|v| v.as_object())
                        .expect("camel kind object");
                    let snake_kind = sobj.get("kind").and_then(|v| v.as_object())
                        .expect("snake kind object");

                    let camel_variant = diagnostic_kind_variant_key(kind, JsonWireStyle::JsCamel);
                    let snake_variant = diagnostic_kind_variant_key(kind, JsonWireStyle::PythonSnake);

                    prop_assert_eq!(camel_kind.len(), 1);
                    prop_assert_eq!(snake_kind.len(), 1);
                    prop_assert!(camel_kind.contains_key(camel_variant));
                    prop_assert!(snake_kind.contains_key(snake_variant));

                    // Inner-payload primitive values are equivalent — both
                    // styles serialize the same Rust fields, only the keys
                    // (e.g. `minArgs` vs `min_args`, `fnName` vs `fn_name`)
                    // differ. Re-encode each kind's inner object's *values*
                    // as a sorted multiset and compare.
                    let camel_inner = camel_kind.get(camel_variant)
                        .and_then(|v| v.as_object())
                        .expect("camel kind inner object");
                    let snake_inner = snake_kind.get(snake_variant)
                        .and_then(|v| v.as_object())
                        .expect("snake kind inner object");

                    prop_assert_eq!(camel_inner.len(), snake_inner.len());

                    let mut camel_vals: Vec<String> = camel_inner.values()
                        .map(|v| v.to_string()).collect();
                    let mut snake_vals: Vec<String> = snake_inner.values()
                        .map(|v| v.to_string()).collect();
                    camel_vals.sort();
                    snake_vals.sort();
                    prop_assert_eq!(camel_vals, snake_vals);
                }
            }
        }
    }

    /// Dependencies JSON shape + styled parity — for any printable AST, the
    /// default (camel) export has the documented seven top-level keys;
    /// `_styled` under either wire style exposes the same set of values
    /// under style-appropriate key casings.
    #[test]
    fn dependencies_json_styled_parity(expr in arb_expr(3, builtin_function_catalog())) {
        let deps = extract_dependencies(&expr);

        let default = dependencies_to_json_value(&deps);
        let camel = dependencies_to_json_value_styled(&deps, JsonWireStyle::JsCamel);
        let snake = dependencies_to_json_value_styled(&deps, JsonWireStyle::PythonSnake);

        // Default is camel.
        prop_assert_eq!(&default, &camel);

        let camel_obj = camel.as_object().expect("camel deps is object");
        let snake_obj = snake.as_object().expect("snake deps is object");

        // Exact seven top-level keys per style.
        let camel_expected = dependencies_top_level_keys(JsonWireStyle::JsCamel);
        let snake_expected = dependencies_top_level_keys(JsonWireStyle::PythonSnake);

        prop_assert_eq!(camel_obj.len(), camel_expected.len());
        prop_assert_eq!(snake_obj.len(), snake_expected.len());

        for key in camel_expected {
            prop_assert!(
                camel_obj.contains_key(key),
                "camel deps missing key {}: {:?}",
                key,
                camel_obj
            );
        }
        for key in snake_expected {
            prop_assert!(
                snake_obj.contains_key(key),
                "snake deps missing key {}: {:?}",
                key,
                snake_obj
            );
        }

        // `fields` is the only key that doesn't rename across styles.
        prop_assert_eq!(camel_obj.get("fields"), snake_obj.get("fields"));

        // Every other key is value-equal under its style-paired counterpart.
        for (camel_key, snake_key) in dependencies_key_pairs() {
            prop_assert_eq!(
                camel_obj.get(camel_key),
                snake_obj.get(snake_key),
                "value under {}/{} should be identical across styles",
                camel_key,
                snake_key
            );
            // Cross-key absence: camel must NOT carry the snake key, etc.
            prop_assert!(!camel_obj.contains_key(snake_key));
            prop_assert!(!snake_obj.contains_key(camel_key));
        }
    }
}
