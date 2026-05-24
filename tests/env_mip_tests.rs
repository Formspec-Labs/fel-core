/// FormspecEnvironment + evaluator integration tests — MIP state queries.
///
/// Addresses audit finding: "No tests for FormspecEnvironment + evaluator integration"
///
/// These tests use FormspecEnvironment (not just MapEnvironment) to verify
/// MIP state queries work through the evaluator.
mod common;

use common::s;
use fel_core::*;

fn eval_with_env(input: &str, env: &FormspecEnvironment) -> EvalResult {
    let expr = parse(input).unwrap();
    evaluate(&expr, env)
}

fn eval_value(input: &str, env: &FormspecEnvironment) -> Value {
    eval_with_env(input, env).value
}

// ── MIP state queries with FormspecEnvironment ──────────────────

/// Spec: core/spec.llm.md L226 — "valid($path), relevant($path), readonly($path), required($path)"
#[test]
fn mip_valid_returns_false_for_invalid_field() {
    let mut env = FormspecEnvironment::new();
    env.set_field("email", s("bad"));
    env.set_mip(
        "email",
        MipState {
            valid: false,
            relevant: true,
            readonly: false,
            required: true,
        },
    );

    assert_eq!(eval_value("valid($email)", &env), Value::Boolean(false));
}

/// Spec: core/spec.llm.md L226 — relevant() queries MIP state
#[test]
fn mip_relevant_returns_configured_state() {
    let mut env = FormspecEnvironment::new();
    env.set_mip(
        "hiddenField",
        MipState {
            valid: true,
            relevant: false,
            readonly: false,
            required: false,
        },
    );

    assert_eq!(
        eval_value("relevant($hiddenField)", &env),
        Value::Boolean(false)
    );
}

/// Spec: core/spec.llm.md L226 — readonly() queries MIP state
#[test]
fn mip_readonly_returns_configured_state() {
    let mut env = FormspecEnvironment::new();
    env.set_mip(
        "lockedField",
        MipState {
            valid: true,
            relevant: true,
            readonly: true,
            required: false,
        },
    );

    assert_eq!(
        eval_value("readonly($lockedField)", &env),
        Value::Boolean(true)
    );
}

/// Spec: core/spec.llm.md L226 — required() queries MIP state
#[test]
fn mip_required_returns_configured_state() {
    let mut env = FormspecEnvironment::new();
    env.set_mip(
        "name",
        MipState {
            valid: true,
            relevant: true,
            readonly: false,
            required: true,
        },
    );

    assert_eq!(eval_value("required($name)", &env), Value::Boolean(true));
}

/// Correctness: MIP defaults for unknown fields
#[test]
fn mip_defaults_for_unknown_field() {
    let env = FormspecEnvironment::new();

    assert_eq!(eval_value("valid($unknown)", &env), Value::Boolean(true));
    assert_eq!(eval_value("relevant($unknown)", &env), Value::Boolean(true));
    assert_eq!(
        eval_value("readonly($unknown)", &env),
        Value::Boolean(false)
    );
    assert_eq!(
        eval_value("required($unknown)", &env),
        Value::Boolean(false)
    );
}

/// Correctness: MIP queries combined with conditional logic
#[test]
fn mip_in_conditional() {
    let mut env = FormspecEnvironment::new();
    env.set_field("email", s("test@example.com"));
    env.set_mip(
        "email",
        MipState {
            valid: true,
            relevant: true,
            readonly: false,
            required: true,
        },
    );

    assert_eq!(
        eval_value(
            "if required($email) and present($email) then 'ok' else 'missing'",
            &env
        ),
        s("ok")
    );
}
