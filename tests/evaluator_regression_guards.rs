//! Evaluator regression guards — pin specific historical panics so they
//! don't recur.
//!
//! These tests are deliberately bespoke (manual AST construction,
//! `std::mem::forget` to dodge recursive drop on deep trees, etc.) and do
//! NOT fit any table-driven shape. They protect against:
//!
//!   - Decimal overflow path (LibFuzzer-discovered): must yield
//!     `Value::Null` + overflow diagnostic, not panic.
//!   - Evaluation-depth limit (LibFuzzer-discovered stack-overflow
//!     guard): a 200-frame deep left-associated binary tree must hit the
//!     depth cap and return null + diagnostic, not blow the stack.

use fel_core::ast::{BinaryOp as AstBinaryOp, Expr};
use fel_core::*;
use rust_decimal::Decimal;

fn eval_result(input: &str) -> EvalResult {
    let expr = parse(input).unwrap();
    let env = MapEnvironment::new();
    evaluate(&expr, &env)
}

/// LibFuzzer regression: decimal-multiplication overflow yields
/// `Value::Null` + diagnostic, never panic.
#[test]
fn decimal_multiplication_overflow_is_null_not_panic() {
    let result = eval_result("1+2266*75555555555555555555555555555*67");
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("overflow")),
        "expected overflow diagnostic, got {:?}",
        result.diagnostics
    );
}

/// LibFuzzer regression: extremely deep left-associated binary AST hits
/// the evaluation-depth cap (stack-overflow guard).
#[test]
fn evaluation_depth_limit_returns_null_with_diagnostic() {
    let mut e = Expr::Number(Decimal::ZERO);
    for _ in 0..200 {
        e = Expr::BinaryOp {
            op: AstBinaryOp::Add,
            left: Box::new(e),
            right: Box::new(Expr::Number(Decimal::ONE)),
        };
    }
    let out = evaluate(&e, &MapEnvironment::new());
    assert_eq!(out.value, Value::Null);
    assert!(
        out.diagnostics.iter().any(|d| d.message.contains("depth")),
        "{:?}",
        out.diagnostics
    );
    // Avoid recursive drop on a deep Expr tree (would overflow the test
    // thread stack).
    std::mem::forget(e);
}
