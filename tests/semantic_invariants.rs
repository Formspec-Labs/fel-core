//! Semantic invariant property table for FEL.
//!
//! One `proptest!` block per algebraic law, consuming C1's `arb_value`/`arb_expr`.
//! Dense table — not narrative tests.
#![cfg(feature = "proptest-strategies")]
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{
    MapEnvironment, Value,
    ast::{BinaryOp, Expr},
    evaluate, parse,
};
use proptest::prelude::*;

fn eval(src: &str) -> Value {
    let expr = parse(src).unwrap();
    let env = MapEnvironment::new();
    evaluate(&expr, &env).value
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        ..Default::default()
    })]

    // ── Null propagation ────────────────────────────────────────
    /// For every binary op (except whitelisted Eq/NotEq/Coalesce), `op(null, x)` must yield null.
    #[test]
    fn null_prop_binary_ops_null_left(
        op in prop::sample::select(vec![
            BinaryOp::Add, BinaryOp::Sub, BinaryOp::Mul, BinaryOp::Div,
            BinaryOp::Mod, BinaryOp::Concat,
            BinaryOp::Lt, BinaryOp::Gt, BinaryOp::LtEq, BinaryOp::GtEq,
            BinaryOp::And, BinaryOp::Or,
        ]),
        x in -500i64..500,
    ) {
        let expr = Expr::BinaryOp {
            op,
            left: Box::new(Expr::Null),
            right: Box::new(Expr::Number(x.into())),
        };
        let env = MapEnvironment::new();
        let result = evaluate(&expr, &env);
        prop_assert!(matches!(result.value, Value::Null),
            "op={op:?} with null left did not propagate null, got {:?}", result.value);
    }

    /// Equality does not propagate null.
    #[test]
    fn eq_does_not_propagate_null(x in -500i64..500) {
        prop_assert_eq!(eval("null = null"), Value::Boolean(true));
        let src = format!("null = {x}");
        prop_assert_eq!(eval(&src), Value::Boolean(false));
    }

    // ── Commutativity ──────────────────────────────────────────
    #[test]
    fn commutativity_add(l in -500i64..500, r in -500i64..500) {
        prop_assert_eq!(eval(&format!("{l} + {r}")), eval(&format!("{r} + {l}")));
    }

    #[test]
    fn commutativity_mul(l in -100i64..100, r in -100i64..100) {
        prop_assert_eq!(eval(&format!("{l} * {r}")), eval(&format!("{r} * {l}")));
    }

    #[test]
    fn commutativity_eq(l in -500i64..500, r in -500i64..500) {
        prop_assert_eq!(eval(&format!("{l} = {r}")), eval(&format!("{r} = {l}")));
    }

    #[test]
    fn commutativity_neq(l in -500i64..500, r in -500i64..500) {
        prop_assert_eq!(eval(&format!("{l} != {r}")), eval(&format!("{r} != {l}")));
    }

    // ── Associativity ──────────────────────────────────────────
    #[test]
    fn assoc_add(a in -100i64..100, b in -100i64..100, c in -100i64..100) {
        let lhs = eval(&format!("({a} + {b}) + {c}"));
        let rhs = eval(&format!("{a} + ({b} + {c})"));
        prop_assert_eq!(lhs, rhs);
    }

    #[test]
    fn assoc_and(a in any::<bool>(), b in any::<bool>(), c in any::<bool>()) {
        let lhs = eval(&format!("({a} and {b}) and {c}"));
        let rhs = eval(&format!("{a} and ({b} and {c})"));
        prop_assert_eq!(lhs, rhs);
    }

    #[test]
    fn assoc_or(a in any::<bool>(), b in any::<bool>(), c in any::<bool>()) {
        let lhs = eval(&format!("({a} or {b}) or {c}"));
        let rhs = eval(&format!("{a} or ({b} or {c})"));
        prop_assert_eq!(lhs, rhs);
    }

    /// Three-valued logic associativity: when any operand can be `null`,
    /// `and`/`or` still associate. Generator picks each operand from
    /// {true, false, null}; the property is that grouping doesn't shift
    /// the result, even when the result is `null` per spec §3 (null
    /// propagation through logical operators that aren't short-circuit-
    /// determined by the non-null operand).
    #[test]
    fn assoc_and_with_nulls(
        a in proptest::sample::select(&["true", "false", "null"][..]).prop_map(|s| s.to_string()),
        b in proptest::sample::select(&["true", "false", "null"][..]).prop_map(|s| s.to_string()),
        c in proptest::sample::select(&["true", "false", "null"][..]).prop_map(|s| s.to_string()),
    ) {
        let lhs = eval(&format!("({a} and {b}) and {c}"));
        let rhs = eval(&format!("{a} and ({b} and {c})"));
        prop_assert_eq!(lhs, rhs);
    }

    #[test]
    fn assoc_or_with_nulls(
        a in proptest::sample::select(&["true", "false", "null"][..]).prop_map(|s| s.to_string()),
        b in proptest::sample::select(&["true", "false", "null"][..]).prop_map(|s| s.to_string()),
        c in proptest::sample::select(&["true", "false", "null"][..]).prop_map(|s| s.to_string()),
    ) {
        let lhs = eval(&format!("({a} or {b}) or {c}"));
        let rhs = eval(&format!("{a} or ({b} or {c})"));
        prop_assert_eq!(lhs, rhs);
    }

    // ── Identity ───────────────────────────────────────────────
    #[test]
    fn identity_add_zero(n in -500i64..500) {
        prop_assert_eq!(eval(&format!("{n} + 0")), Value::Number(n.into()));
        prop_assert_eq!(eval(&format!("0 + {n}")), Value::Number(n.into()));
    }

    #[test]
    fn identity_mul_one(n in -100i64..100) {
        prop_assert_eq!(eval(&format!("{n} * 1")), Value::Number(n.into()));
    }

    #[test]
    fn identity_and_true(b in any::<bool>()) {
        prop_assert_eq!(eval(&format!("{b} and true")), Value::Boolean(b));
    }

    #[test]
    fn identity_or_false(b in any::<bool>()) {
        prop_assert_eq!(eval(&format!("{b} or false")), Value::Boolean(b));
    }

    // ── Conditional ────────────────────────────────────────────
    #[test]
    fn conditional_if_true_x_y_is_x(x in -100i64..100, y in -100i64..100) {
        prop_assert_eq!(eval(&format!("if(true, {x}, {y})")), Value::Number(x.into()));
    }

    #[test]
    fn conditional_if_false_x_y_is_y(x in -100i64..100, y in -100i64..100) {
        prop_assert_eq!(eval(&format!("if(false, {x}, {y})")), Value::Number(y.into()));
    }

    #[test]
    fn conditional_if_c_x_x_is_x(c in any::<bool>(), x in -100i64..100) {
        prop_assert_eq!(eval(&format!("if({c}, {x}, {x})")), Value::Number(x.into()));
    }

    // ── De Morgan ──────────────────────────────────────────────
    #[test]
    fn de_morgan_not_and(a in any::<bool>(), b in any::<bool>()) {
        let lhs = eval(&format!("not ({a} and {b})"));
        let rhs = eval(&format!("(not {a}) or (not {b})"));
        prop_assert_eq!(lhs, rhs);
    }

    #[test]
    fn de_morgan_not_or(a in any::<bool>(), b in any::<bool>()) {
        let lhs = eval(&format!("not ({a} or {b})"));
        let rhs = eval(&format!("(not {a}) and (not {b})"));
        prop_assert_eq!(lhs, rhs);
    }

    // ── Double negation ────────────────────────────────────────
    #[test]
    fn double_negation(b in any::<bool>()) {
        prop_assert_eq!(eval(&format!("not (not {b})")), Value::Boolean(b));
    }

    // ── Coalesce ───────────────────────────────────────────────
    #[test]
    fn coalesce_null_returns_right(x in any::<bool>()) {
        prop_assert_eq!(eval(&format!("null ?? {x}")), Value::Boolean(x));
    }

    #[test]
    fn coalesce_non_null_returns_left(x in any::<bool>()) {
        prop_assert_eq!(eval(&format!("{x} ?? false")), Value::Boolean(x));
    }

    // ── Decimal-precision-aware commutativity / associativity ────
    /// `a + b == b + a` over large i64 values (covers most Decimal range safely).
    #[test]
    fn decimal_commutativity_add(
        a in -1_000_000_000i64..1_000_000_000,
        b in -1_000_000_000i64..1_000_000_000,
    ) {
        let lhs = eval(&format!("{a} + {b}"));
        let rhs = eval(&format!("{b} + {a}"));
        prop_assert_eq!(lhs, rhs);
    }

    /// `a * b == b * a` over safe i64 range.
    #[test]
    fn decimal_commutativity_mul(
        a in -1_000_000i64..1_000_000,
        b in -1_000_000i64..1_000_000,
    ) {
        let lhs = eval(&format!("{a} * {b}"));
        let rhs = eval(&format!("{b} * {a}"));
        prop_assert_eq!(lhs, rhs);
    }

    /// `(a + b) + c == a + (b + c)` for non-overflowing triples.
    /// Restricts to `i64` range since Decimal fixed-point associativity
    /// can break with extreme magnitude differences (e.g. 9e18 + -9e18 + 1e-10).
    #[test]
    fn decimal_assoc_add(
        a in -1_000_000i64..1_000_000,
        b in -1_000_000i64..1_000_000,
        c in -1_000_000i64..1_000_000,
    ) {
        let lhs = eval(&format!("({a} + {b}) + {c}"));
        let rhs = eval(&format!("{a} + ({b} + {c})"));
        prop_assert_eq!(lhs, rhs);
    }
}

/// Documented break in Decimal associativity at extreme magnitude
/// differences. This is the case the `decimal_assoc_add` proptest above
/// hedges around — see review HIGH from the algebraic-tests reviewer.
///
/// `rust_decimal` (28-digit fixed-point) cannot represent the loss-of-
/// precision case `9e18 + (-9e18) + 1e-10` faithfully: the small term is
/// absorbed in the magnitude difference. We pin the *current* behavior
/// rather than the mathematical ideal so future Decimal-precision
/// changes (e.g. moving to higher-precision arithmetic) surface as a
/// test diff for explicit review.
#[test]
fn decimal_assoc_add_extreme_magnitude_is_documented_break() {
    // (9e18 + -9e18) + 1e-10 — the bracketed sum is exactly 0; the small
    // term survives. With non-extreme arithmetic this would be 1e-10.
    let lhs = eval("(9000000000000000000 + -9000000000000000000) + 0.0000000001");
    // a + (b + c) — Decimal can't represent 9e18 + 1e-10 exactly; the
    // small term is absorbed by the magnitude difference, then -9e18
    // brings the total back to 0 (or near-0 with precision loss).
    let rhs = eval("9000000000000000000 + (-9000000000000000000 + 0.0000000001)");
    // The break: lhs == 0.0000000001, rhs == 0 (precision loss).
    // Both are observable; the test asserts they DIFFER — which is the
    // documented limitation, not a bug.
    assert_ne!(
        lhs, rhs,
        "associativity break expected at extreme magnitude; if lhs == rhs the \
         underlying Decimal precision has changed and decimal_assoc_add's i64 \
         restriction may be removable"
    );
}
