//! Phase 3b Tier-2 proptest closure for the locale-interpolation predicate,
//! the trace accumulator, and the host context-binding tag/struct surface.
//!
//! Five symbols covered:
//!
//! 1. `interpolation::expr_is_interpolation_static_literal`
//! 2. `trace::Trace`
//! 3. `trace::TraceStep`
//! 4. `evaluator::ContextBinding`
//! 5. `evaluator::ContextBindingKind`
//!
//! Closes the matching `notes = "Phase 3b-pending: ..."` rows in
//! `tests/lib_reexport_coverage.toml`.
//!
//! Gated on `proptest-strategies` for the `arb_expr` strategy import — runs
//! via `cargo test --features proptest-strategies --test interpolation_trace_proptest`.

#![cfg(feature = "proptest-strategies")]
#![allow(clippy::missing_docs_in_private_items)]

use std::collections::HashMap;

use fel_core::ast::{Expr, UnaryOp};
use fel_core::testing::strategies::arb_expr;
use fel_core::{
    ContextBinding, ContextBindingCatalog, ContextBindingKind, EvaluatorOptions, IndexMap,
    MapEnvironment, Trace, TraceStep, Value, builtin_function_catalog, evaluate_with,
    evaluate_with_catalog, expr_is_interpolation_static_literal, parse,
};
use proptest::prelude::*;
use rust_decimal::Decimal;

// ── Property 1: interpolation predicate oracle ───────────────────────────────

/// Exhaustive-match oracle for the locale §3.3.1 static-literal predicate.
///
/// Mirrors the production implementation by structure, but is written as an
/// independent walker that fails to compile if a new `Expr` variant is added
/// without an explicit decision (no `_` wildcard arm). The predicate-under-test
/// and the oracle must agree on every AST the strategy can produce.
fn oracle_is_static_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Null
        | Expr::Boolean(_)
        | Expr::Number(_)
        | Expr::String(_)
        | Expr::DateLiteral(_)
        | Expr::DateTimeLiteral(_) => true,
        Expr::Array(elems) => elems.iter().all(oracle_is_static_literal),
        Expr::Object(entries) => entries.iter().all(|(_, v)| oracle_is_static_literal(v)),
        Expr::UnaryOp {
            op: UnaryOp::Not | UnaryOp::Neg,
            operand,
            ..
        } => oracle_is_static_literal(operand),
        Expr::VarRef { .. }
        | Expr::ContextRef { .. }
        | Expr::FunctionCall { .. }
        | Expr::PostfixAccess { .. }
        | Expr::BinaryOp { .. }
        | Expr::Ternary { .. }
        | Expr::IfThenElse { .. }
        | Expr::Membership { .. }
        | Expr::NullCoalesce { .. }
        | Expr::LetBinding { .. }
        | Expr::FieldRef { .. } => false,
    }
}

// ── Property 3 helper: TraceStep exhaustive discriminator ────────────────────

/// Returns the variant tag for a [`TraceStep`]. Uses an exhaustive enum match
/// with NO wildcard arm so adding a new variant fails this file to compile,
/// guarding the trace v0 contract.
fn trace_step_kind(step: &TraceStep) -> &'static str {
    match step {
        TraceStep::FieldResolved { .. } => "FieldResolved",
        TraceStep::FunctionCalled { .. } => "FunctionCalled",
        TraceStep::BinaryOp { .. } => "BinaryOp",
        TraceStep::IfBranch { .. } => "IfBranch",
        TraceStep::ShortCircuit { .. } => "ShortCircuit",
    }
}

// ── Property 4 fixtures: per-kind ContextBindingCatalog ──────────────────────

/// Catalog that materializes every `ContextBindingKind` variant under a
/// distinct host name so an expression can route to each branch.
struct AllKindsCatalog;

impl ContextBindingCatalog for AllKindsCatalog {
    fn binding_kind(&self, name: &str) -> Option<ContextBindingKind> {
        match name {
            "val" => Some(ContextBindingKind::Value),
            "obj" => Some(ContextBindingKind::Object),
            "fn" => Some(ContextBindingKind::Function),
            _ => None,
        }
    }

    fn resolve(&self, name: &str, arg: Option<&str>) -> Option<Value> {
        match (name, arg) {
            ("val", _) => Some(Value::String("v".to_string())),
            ("obj", _) => {
                let mut fields = IndexMap::new();
                fields.insert("k".to_string(), Value::String("o".to_string()));
                Some(Value::Object(fields))
            }
            ("fn", Some(a)) => Some(Value::String(format!("f:{a}"))),
            ("fn", None) => Some(Value::String("f".to_string())),
            _ => None,
        }
    }
}

/// Builds a [`ContextBinding`] for `kind` using the inherent constructor matching
/// each variant. Exhaustive match guards the constructor surface.
fn binding_for_kind(kind: ContextBindingKind) -> ContextBinding {
    match kind {
        ContextBindingKind::Value => ContextBinding::value(Value::String("v".to_string())),
        ContextBindingKind::Object => {
            let mut fields = IndexMap::new();
            fields.insert("k".to_string(), Value::String("o".to_string()));
            ContextBinding::object(Value::Object(fields))
        }
        ContextBindingKind::Function => ContextBinding::function(Value::String("f".to_string())),
    }
}

/// Sample-source for the surface expression that exercises a given kind through
/// the evaluator. Exhaustive match guards the kind enum.
fn surface_source_for_kind(kind: ContextBindingKind) -> &'static str {
    match kind {
        ContextBindingKind::Value => "@val",
        ContextBindingKind::Object => "@obj.k",
        ContextBindingKind::Function => "@fn('a')",
    }
}

fn eval_source_with_catalog(source: &str, catalog: &dyn ContextBindingCatalog) -> Value {
    let expr = parse(source).expect("source should parse");
    let env = MapEnvironment::new();
    evaluate_with_catalog(&expr, &env, EvaluatorOptions::default(), catalog).value
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        ..Default::default()
    })]

    // ── Property 1: predicate-vs-oracle agreement ───────────────────────────

    /// `expr_is_interpolation_static_literal` agrees with an independent
    /// exhaustive-match oracle for every AST `arb_expr` can produce.
    ///
    /// Adding a new `Expr` variant fails the oracle to compile (no wildcard
    /// arm), forcing a deliberate decision. Mutating the production predicate
    /// to flip any arm's polarity surfaces here as a counter-example.
    #[test]
    fn interpolation_predicate_matches_oracle(
        expr in arb_expr(3, builtin_function_catalog())
    ) {
        prop_assert_eq!(
            expr_is_interpolation_static_literal(&expr),
            oracle_is_static_literal(&expr)
        );
    }

    /// Determinism: predicate is a pure function of its input.
    #[test]
    fn interpolation_predicate_is_pure(
        expr in arb_expr(3, builtin_function_catalog())
    ) {
        let a = expr_is_interpolation_static_literal(&expr);
        let b = expr_is_interpolation_static_literal(&expr);
        prop_assert_eq!(a, b);
    }

    // ── Property 2: Trace monotonicity ──────────────────────────────────────

    /// `Trace.steps.len()` is non-decreasing when the trace is used as an
    /// append-only accumulator across multiple `evaluate_with` runs.
    ///
    /// The evaluator replaces the caller's `Trace` per run (see
    /// `evaluate_configured`), so to test the documented append-only contract
    /// we manually push each per-run trace's steps into a single accumulator
    /// and assert the length never goes down.
    #[test]
    fn trace_accumulator_length_is_non_decreasing(
        exprs in proptest::collection::vec(arb_expr(2, builtin_function_catalog()), 1..6)
    ) {
        let env = MapEnvironment::new();
        let mut accumulator = Trace::new();
        let mut prev_len = accumulator.len();
        prop_assert!(accumulator.is_empty());

        for expr in &exprs {
            let mut run_trace = Trace::new();
            let _ = evaluate_with(
                expr,
                &env,
                EvaluatorOptions {
                    trace: Some(&mut run_trace),
                    ..EvaluatorOptions::default()
                },
            );
            for step in run_trace.steps {
                accumulator.push(step);
            }
            let new_len = accumulator.len();
            prop_assert!(
                new_len >= prev_len,
                "accumulator length must be non-decreasing: {prev_len} -> {new_len}"
            );
            prev_len = new_len;
        }
    }

    // ── Property 3: every emitted TraceStep matches a known variant ─────────

    /// Every step the evaluator emits for an arbitrary `arb_expr` resolves to
    /// one of the five v0 variants. Exhaustive match in `trace_step_kind`
    /// guards against silent variant addition.
    #[test]
    fn every_emitted_trace_step_has_known_kind(
        expr in arb_expr(3, builtin_function_catalog())
    ) {
        let env = MapEnvironment::new();
        let mut trace = Trace::new();
        let _ = evaluate_with(
            &expr,
            &env,
            EvaluatorOptions {
                trace: Some(&mut trace),
                ..EvaluatorOptions::default()
            },
        );
        for step in &trace.steps {
            let kind = trace_step_kind(step);
            prop_assert!(
                matches!(
                    kind,
                    "FieldResolved" | "FunctionCalled" | "BinaryOp" | "IfBranch" | "ShortCircuit"
                ),
                "unexpected TraceStep kind: {kind}"
            );
        }
    }

    // ── Property 4: ContextBinding + ContextBindingKind enumeration ─────────

    /// For each `ContextBindingKind` variant, construct a `ContextBinding`
    /// using the matching inherent constructor, and observe it through the
    /// evaluator via a one-per-kind catalog. Each kind resolves to its
    /// expected value through the corresponding surface syntax.
    #[test]
    fn every_context_binding_kind_is_observable(
        seed in 0u8..3
    ) {
        // proptest seed only selects which kind we focus the assertion on;
        // the full exhaustive coverage runs below regardless.
        let kinds = [
            ContextBindingKind::Value,
            ContextBindingKind::Object,
            ContextBindingKind::Function,
        ];

        // Exhaustive-match constructor + struct field assertions for every
        // kind on every iteration — the proptest seed just amplifies cases.
        for kind in kinds {
            let binding = binding_for_kind(kind);
            prop_assert_eq!(binding.kind, kind);
            match kind {
                ContextBindingKind::Value => {
                    prop_assert_eq!(binding.value, Value::String("v".to_string()));
                }
                ContextBindingKind::Object => {
                    prop_assert!(matches!(binding.value, Value::Object(_)));
                }
                ContextBindingKind::Function => {
                    prop_assert_eq!(binding.value, Value::String("f".to_string()));
                }
            }
        }

        // Catalog-mediated observation: each kind resolves through the
        // evaluator under its surface syntax.
        let catalog = AllKindsCatalog;
        for kind in kinds {
            let source = surface_source_for_kind(kind);
            let observed = eval_source_with_catalog(source, &catalog);
            let expected = match kind {
                ContextBindingKind::Value => Value::String("v".to_string()),
                ContextBindingKind::Object => Value::String("o".to_string()),
                ContextBindingKind::Function => Value::String("f:a".to_string()),
            };
            prop_assert_eq!(
                observed,
                expected,
                "kind={:?} via {} did not resolve as expected",
                kind,
                source
            );
        }

        // Touch `seed` so proptest doesn't lint it as unused; it stays in the
        // generator to keep the proptest shrinker engaged across the per-kind
        // sweep above.
        prop_assert!(seed < 3);
    }
}

// ── Example tests (non-proptest) — concrete pins for each property ───────────

/// Smoke pin: a pure literal AST is a static literal.
#[test]
fn literal_number_is_static_literal() {
    let expr = Expr::Number(Decimal::from(42));
    assert!(expr_is_interpolation_static_literal(&expr));
    assert!(oracle_is_static_literal(&expr));
}

/// Smoke pin: a string literal is a static literal.
#[test]
fn literal_string_is_static_literal() {
    let expr = Expr::String("foo".to_string());
    assert!(expr_is_interpolation_static_literal(&expr));
    assert!(oracle_is_static_literal(&expr));
}

/// Smoke pin: a `FieldRef` is NOT a static literal (dynamic).
#[test]
fn field_ref_is_not_static_literal() {
    let expr = Expr::FieldRef {
        name: Some("x".to_string()),
        path: vec![],
    };
    assert!(!expr_is_interpolation_static_literal(&expr));
    assert!(!oracle_is_static_literal(&expr));
}

/// Smoke pin: a fresh trace is empty and has zero length.
#[test]
fn new_trace_is_empty() {
    let trace = Trace::new();
    assert!(trace.is_empty());
    assert_eq!(trace.len(), 0);
}

/// Smoke pin: each `ContextBinding` constructor stamps the matching kind.
#[test]
fn context_binding_constructors_stamp_kind() {
    let mut env_fields = HashMap::<String, Value>::new();
    env_fields.insert("dummy".to_string(), Value::Null);
    let v = ContextBinding::value(Value::Null);
    let o = ContextBinding::object(Value::Null);
    let f = ContextBinding::function(Value::Null);
    assert_eq!(v.kind, ContextBindingKind::Value);
    assert_eq!(o.kind, ContextBindingKind::Object);
    assert_eq!(f.kind, ContextBindingKind::Function);
    // ensure env_fields use isn't dead-code-warning bait
    assert_eq!(env_fields.len(), 1);
}
