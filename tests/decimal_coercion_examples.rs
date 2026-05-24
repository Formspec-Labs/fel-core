//! Decimal coercion examples.
//!
//! Example-based round-trips between FEL Value::Number and JSON, including the
//! string-fallback behavior for integers above the JS-safe range and
//! sub-precise decimals. Behavioral arithmetic properties (overflow, identity,
//! money currency rules) live in `decimal_properties.rs`.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{Value, fel_to_json, json_to_fel};

#[test]
fn decimal_coercion_roundtrip() {
    let v = 42_i64;
    let num = Value::Number(v.into());
    let json = fel_to_json(&num);
    let back = json_to_fel(&json);
    assert_eq!(back, num);
}

#[test]
fn decimal_json_string_fallback_for_integer_above_js_safe_range() {
    let v = 9_223_372_036_854_775_807_i64;
    let num = Value::Number(v.into());
    let json = fel_to_json(&num);
    assert_eq!(json, serde_json::json!("9223372036854775807"));
}

#[test]
fn decimal_json_string_fallback_for_subprecise() {
    let v = Value::Number("0.000000000000000000000000001".parse().unwrap());
    let json = fel_to_json(&v);
    let back = json_to_fel(&json);
    assert_eq!(back, v);
}
