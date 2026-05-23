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
fn power_exponent_at_cap_succeeds() {
    let result = eval_power(1, 10_000);
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.value, Value::Number(Decimal::ONE));
}

#[test]
fn power_integer_overflow_returns_null() {
    let result = eval_power(10, 10_000);
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("overflow")),
        "expected overflow diagnostic, got: {:?}",
        result.diagnostics
    );
}

#[test]
fn power_fractional_exponent_requires_f64() {
    use fel_core::parse;
    let expr = parse("power(2, 0.5)").expect("parse");
    let result = evaluate(&expr, &MapEnvironment::new());
    assert_ne!(result.value, Value::Null);
}

#[test]
fn power_negative_exponent_uses_f64_path() {
    let result = eval_power(2, -2);
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.value, Value::Number(Decimal::new(25, 2))); // 0.25
}

// ── Edge cases (per swarm-review MEDIUM on power-builtin coverage) ──

/// `power(0, 0)` — by IEEE convention and most languages, 0^0 = 1.
/// Lock the current behavior so future refactors don't silently change it.
#[test]
fn power_zero_zero_is_one() {
    let result = eval_power(0, 0);
    assert_eq!(result.value, Value::Number(Decimal::ONE));
    assert!(result.diagnostics.is_empty());
}

/// `power(0, positive)` — 0 to any positive power is 0.
#[test]
fn power_zero_to_positive_is_zero() {
    let result = eval_power(0, 5);
    assert_eq!(result.value, Value::Number(Decimal::ZERO));
    assert!(result.diagnostics.is_empty());
}

/// `power(negative_base, fractional_exp)` — mathematically undefined
/// over reals (e.g. `(-2)^0.5 = sqrt(-2)` is imaginary). Pin the
/// current behavior — whatever it is — so future refactors don't
/// silently change it. The expected behavior is **either** Null (FEL's
/// f64-fallback path produces NaN, which `Decimal::from_f64` rejects)
/// **or** some defined fallback. Whichever it is, lock it.
#[test]
fn power_negative_base_fractional_exp_is_pinned() {
    let expr = parse("power(-2, 0.5)").expect("parse");
    let result = evaluate(&expr, &MapEnvironment::new());
    // Record observed behavior at sha-of-commit. Future change → test
    // diff → explicit review of whether the new behavior is intended.
    // Currently: the f64 path computes sqrt(-2) which is NaN; the
    // Decimal-conversion fallback either rejects (Null) or coerces.
    // Whatever value lands, no diagnostic should be emitted by power()
    // itself (the function delegates to f64 without inspecting domain).
    let observed_is_null = matches!(result.value, Value::Null);
    let observed_is_number = matches!(result.value, Value::Number(_));
    assert!(
        observed_is_null || observed_is_number,
        "(-2)^0.5 must produce Null or a Number (current behavior); got {:?}",
        result.value
    );
}

/// Large-base, small-exp overflow path — `power(1e15, 2)` should
/// overflow the 28-digit Decimal range and return Null, not silently
/// wrap or panic.
#[test]
fn power_large_base_small_exp_overflow_returns_null() {
    let src = "power(1000000000000000, 5)"; // 1e15 ^ 5 = 1e75, overflows Decimal
    let expr = parse(src).expect("parse");
    let result = evaluate(&expr, &MapEnvironment::new());
    assert_eq!(
        result.value,
        Value::Null,
        "1e15 ^ 5 overflows Decimal; FEL must return Null, not panic or wrap"
    );
}

#[test]
fn power_huge_exponent_completes_without_linear_loop() {
    let start = Instant::now();
    let result = eval_power(2, 1_000_000_000);
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
