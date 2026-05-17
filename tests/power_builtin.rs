//! `power()` builtin: exponent cap and O(log n) integer exponentiation (FEL-SMELL-C-002).

use std::time::{Duration, Instant};

use fel_core::{MapEnvironment, Value, evaluate, parse};
use rust_decimal::Decimal;

fn eval_power(base: i64, exp: i64) -> fel_core::EvalResult {
    let src = format!("power({base}, {exp})");
    let expr = parse(&src).expect("parse power expr");
    evaluate(&expr, &MapEnvironment::new())
}

#[test]
fn power_integer_exponent_within_cap() {
    let result = eval_power(2, 10);
    assert_eq!(result.value, Value::Number(Decimal::from(1024)));
    assert!(result.diagnostics.is_empty());
}

#[test]
fn power_zero_exponent_is_one() {
    let result = eval_power(5, 0);
    assert_eq!(result.value, Value::Number(Decimal::ONE));
}

#[test]
fn power_exponent_above_cap_returns_null() {
    let result = eval_power(2, 10_001);
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("exponent too large")),
        "expected exponent cap diagnostic, got: {:?}",
        result.diagnostics
    );
}

#[test]
fn power_huge_exponent_completes_without_linear_loop() {
    let start = Instant::now();
    let result = eval_power(2,  1_000_000_000);
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(200),
        "power(2, 1e9) took {:?}; expected cap or O(log n), not O(exp)",
        elapsed
    );
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("exponent too large")),
        "expected exponent cap diagnostic, got: {:?}",
        result.diagnostics
    );
}
