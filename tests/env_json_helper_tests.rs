/// FormspecEnvironment + evaluator integration tests — variables, named instances,
/// field resolution, and the WASM-shaped JSON helper (`formspec_environment_from_json_map`).
///
/// Addresses audit finding: "No tests for FormspecEnvironment + evaluator integration"
///
/// These tests use FormspecEnvironment (not just MapEnvironment) to verify
/// variables, named instances, basic field resolution, and end-to-end JSON-map
/// construction work through the evaluator.
mod common;

use common::{num, obj, s};
use fel_core::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromStr;
use serde_json::json;

fn eval_with_env(input: &str, env: &FormspecEnvironment) -> EvalResult {
    let expr = parse(input).unwrap();
    evaluate(&expr, env)
}

fn eval_value(input: &str, env: &FormspecEnvironment) -> Value {
    eval_with_env(input, env).value
}

// ── Variables ───────────────────────────────────────────────────

/// Correctness: definition variables via @variableName
#[test]
fn variable_resolution() {
    let mut env = FormspecEnvironment::new();
    env.set_variable("taxRate", Value::Number(Decimal::from_str("0.08").unwrap()));
    env.set_field("subtotal", num(100));

    assert_eq!(
        eval_value("$subtotal * @taxRate", &env),
        Value::Number(Decimal::from_str("8.00").unwrap())
    );
}

/// Correctness: undefined variable returns null
#[test]
fn undefined_variable_returns_null() {
    let env = FormspecEnvironment::new();
    assert_eq!(eval_value("@undefinedVar", &env), Value::Null);
}

/// Correctness: variable with object value and dot access
#[test]
fn variable_with_nested_object() {
    let mut env = FormspecEnvironment::new();
    let config = obj(vec![
        ("maxItems".to_string(), num(10)),
        ("label".to_string(), s("Settings")),
    ]);
    env.set_variable("config", config);

    assert_eq!(eval_value("@config.maxItems", &env), num(10));
    assert_eq!(eval_value("@config.label", &env), s("Settings"));
}

// ── Named instances ─────────────────────────────────────────────

/// Correctness: @instance('name') resolution
#[test]
fn named_instance_resolution() {
    let mut env = FormspecEnvironment::new();
    let lookup = obj(vec![
        ("us".to_string(), s("United States")),
        ("uk".to_string(), s("United Kingdom")),
    ]);
    env.set_instance("countries", lookup);

    assert_eq!(
        eval_value("@instance('countries').us", &env),
        s("United States")
    );
}

/// Correctness: unknown instance returns null
#[test]
fn unknown_instance_returns_null() {
    let env = FormspecEnvironment::new();
    assert_eq!(eval_value("@instance('missing')", &env), Value::Null);
}

// ── Field resolution with FormspecEnvironment ────────────────────

/// Correctness: basic field resolution
#[test]
fn field_resolution_basic() {
    let mut env = FormspecEnvironment::new();
    env.set_field("name", s("Alice"));

    assert_eq!(eval_value("$name", &env), s("Alice"));
}

/// Correctness: nested field resolution (object walk)
#[test]
fn field_resolution_nested_object() {
    let mut env = FormspecEnvironment::new();
    let addr = obj(vec![
        ("city".to_string(), s("NYC")),
        ("zip".to_string(), s("10001")),
    ]);
    env.set_field("address", addr);

    assert_eq!(eval_value("$address.city", &env), s("NYC"));
}

/// Correctness: flat dotted key lookup
#[test]
fn field_resolution_flat_dotted_key() {
    let mut env = FormspecEnvironment::new();
    env.set_field("address.city", s("Boston"));

    assert_eq!(eval_value("$address.city", &env), s("Boston"));
}

/// Correctness: missing field returns null
#[test]
fn missing_field_returns_null() {
    let env = FormspecEnvironment::new();
    assert_eq!(eval_value("$missing", &env), Value::Null);
}

/// Correctness: bare $ in repeat context returns current
#[test]
fn bare_dollar_in_repeat_returns_current() {
    let mut env = FormspecEnvironment::new();
    let items = vec![num(42)];
    env.push_repeat(num(42), 1, 1, items);

    assert_eq!(eval_value("$", &env), num(42));
}

// ── WASM-shaped context JSON (`formspec_environment_from_json_map`) ──

/// End-to-end: JSON map like WASM `evalFELWithContext` → evaluate.
#[test]
fn wasm_shaped_json_context_evaluates_field_variable_mip_and_locale() {
    let ctx = json!({
        "nowIso": "2026-03-20T12:00:00",
        "fields": { "score": 10 },
        "variables": { "bonus": 2 },
        "locale": "en",
        "mipStates": {
            "score": {
                "valid": true,
                "relevant": true,
                "readonly": false,
                "required": false
            }
        },
        "meta": { "runId": "smoke" }
    });
    let map = ctx.as_object().unwrap().clone();
    let env = formspec_environment_from_json_map(&map);

    let expr = parse("$score + @bonus").unwrap();
    let out = evaluate(&expr, &env);
    assert_eq!(out.value, Value::Number(Decimal::from(12)));

    let valid_expr = parse("valid($score)").unwrap();
    assert_eq!(evaluate(&valid_expr, &env).value, Value::Boolean(true));

    assert_eq!(
        evaluate(&parse("locale()").unwrap(), &env).value,
        Value::String("en".into())
    );

    let plural = evaluate(&parse("pluralCategory(1)").unwrap(), &env).value;
    assert!(
        matches!(plural, Value::String(ref s) if s == "one" || s == "other"),
        "unexpected pluralCategory: {plural:?}"
    );
}
