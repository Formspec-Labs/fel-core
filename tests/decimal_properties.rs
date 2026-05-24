//! Decimal arithmetic property coverage.
//!
//! Example-based assertions over Decimal arithmetic behavior: overflow,
//! identity, subprecision, and money currency rules. JSON coercion
//! round-trips live in `decimal_coercion_examples.rs`.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{MapEnvironment, Value, evaluate, parse};

fn eval(src: &str) -> Value {
    let expr = parse(src).unwrap();
    let env = MapEnvironment::new();
    evaluate(&expr, &env).value
}

#[test]
fn decimal_overflow_addition_emits_error() {
    let max = "79228162514264337593543950335";
    let src = format!("{max} + 1");
    let result = eval(&src);
    assert!(matches!(result, Value::Null));
}

#[test]
fn decimal_underflow_subtraction_emits_error() {
    let min = "-79228162514264337593543950335";
    let src = format!("{min} - 1");
    let result = eval(&src);
    assert!(matches!(result, Value::Null));
}

#[test]
fn decimal_multiplication_overflow_emits_error() {
    let max = i64::MAX;
    let src = format!("{max} * {max}");
    let result = eval(&src);
    assert!(matches!(result, Value::Null));
}

#[test]
fn decimal_division_by_zero_emits_error() {
    let result = eval("1 / 0");
    assert!(matches!(result, Value::Null));
}

#[test]
fn decimal_modulo_by_zero_emits_error() {
    let result = eval("1 % 0");
    assert!(matches!(result, Value::Null));
}

#[test]
fn decimal_identity_edge() {
    assert_eq!(eval("0"), Value::Number(0.into()));
}

#[test]
fn decimal_subprecision_operations() {
    let result = eval("0.0000001 + 0.0000002");
    match result {
        Value::Number(n) => {
            assert_eq!(n.to_string(), "0.0000003");
        }
        other => panic!("expected number, got {other:?}"),
    }
}

#[test]
fn money_same_currency_add_works() {
    let result = eval("money(10, 'USD') + money(5, 'USD')");
    match result {
        Value::Money(m) => {
            assert_eq!(m.currency.as_str(), "USD");
            assert_eq!(m.amount.to_string(), "15");
        }
        other => panic!("expected Money, got {other:?}"),
    }
}

#[test]
fn money_mixed_currency_add_errors() {
    let expr = parse("money(10, 'USD') + money(5, 'EUR')").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    assert!(matches!(result.value, Value::Null));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("currency mismatch"))
    );
}
