/// FormspecEnvironment + evaluator integration tests.
///
/// Addresses audit finding: "No tests for FormspecEnvironment + evaluator integration"
///
/// These tests use FormspecEnvironment (not just MapEnvironment) to verify
/// MIP state queries, repeat context, and variables work through the evaluator.
use fel_core::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

fn num(n: i64) -> FelValue {
    FelValue::Number(Decimal::from(n))
}

fn s(v: &str) -> FelValue {
    FelValue::String(v.to_string())
}

fn eval_with_env(input: &str, env: &FormspecEnvironment) -> EvalResult {
    let expr = parse(input).unwrap();
    evaluate(&expr, env)
}

fn eval_value(input: &str, env: &FormspecEnvironment) -> FelValue {
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

    assert_eq!(eval_value("valid($email)", &env), FelValue::Boolean(false));
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
        FelValue::Boolean(false)
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
        FelValue::Boolean(true)
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

    assert_eq!(
        eval_value("required($name)", &env),
        FelValue::Boolean(true)
    );
}

/// Correctness: MIP defaults for unknown fields
#[test]
fn mip_defaults_for_unknown_field() {
    let env = FormspecEnvironment::new();

    assert_eq!(
        eval_value("valid($unknown)", &env),
        FelValue::Boolean(true)
    );
    assert_eq!(
        eval_value("relevant($unknown)", &env),
        FelValue::Boolean(true)
    );
    assert_eq!(
        eval_value("readonly($unknown)", &env),
        FelValue::Boolean(false)
    );
    assert_eq!(
        eval_value("required($unknown)", &env),
        FelValue::Boolean(false)
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

// ── Repeat context integration ──────────────────────────────────

/// Spec: core/spec.llm.md — repeat context: @current, @index, @count
#[test]
fn repeat_context_current_index_count() {
    let mut env = FormspecEnvironment::new();
    let items = vec![num(10), num(20), num(30)];
    env.push_repeat(num(20), 2, 3, items);

    assert_eq!(eval_value("@current", &env), num(20));
    assert_eq!(eval_value("@index", &env), num(2));
    assert_eq!(eval_value("@count", &env), num(3));
}

/// Correctness: repeat context with object values
#[test]
fn repeat_context_with_object_current() {
    let mut env = FormspecEnvironment::new();
    let item = FelValue::Object(vec![
        ("name".to_string(), s("Alice")),
        ("age".to_string(), num(30)),
    ]);
    env.push_repeat(item.clone(), 1, 1, vec![item]);

    assert_eq!(eval_value("@current.name", &env), s("Alice"));
    assert_eq!(eval_value("@current.age", &env), num(30));
}

/// Correctness: prev() and next() navigation
#[test]
fn repeat_prev_next_navigation() {
    let mut env = FormspecEnvironment::new();
    let items = vec![num(10), num(20), num(30)];
    env.push_repeat(num(20), 2, 3, items);

    assert_eq!(eval_value("prev()", &env), num(10));
    assert_eq!(eval_value("next()", &env), num(30));
}

/// Correctness: prev() at first item returns null
#[test]
fn repeat_prev_at_first_returns_null() {
    let mut env = FormspecEnvironment::new();
    let items = vec![num(10), num(20)];
    env.push_repeat(num(10), 1, 2, items);

    assert_eq!(eval_value("prev()", &env), FelValue::Null);
}

/// Correctness: next() at last item returns null
#[test]
fn repeat_next_at_last_returns_null() {
    let mut env = FormspecEnvironment::new();
    let items = vec![num(10), num(20)];
    env.push_repeat(num(20), 2, 2, items);

    assert_eq!(eval_value("next()", &env), FelValue::Null);
}

/// Correctness: nested repeat — parent() returns outer current
#[test]
fn nested_repeat_parent_navigation() {
    let mut env = FormspecEnvironment::new();
    let outer = vec![s("row_a"), s("row_b")];
    env.push_repeat(s("row_a"), 1, 2, outer);

    let inner = vec![num(1), num(2)];
    env.push_repeat(num(2), 2, 2, inner);

    assert_eq!(eval_value("@current", &env), num(2));
    assert_eq!(eval_value("parent()", &env), s("row_a"));
    assert_eq!(eval_value("@index", &env), num(2));
}

/// Correctness: @current in expressions
#[test]
fn repeat_current_in_arithmetic() {
    let mut env = FormspecEnvironment::new();
    let items = vec![num(5), num(10), num(15)];
    env.push_repeat(num(10), 2, 3, items);

    assert_eq!(eval_value("@current * 2", &env), num(20));
    assert_eq!(eval_value("@current + @index", &env), num(12));
}

// ── Variables ───────────────────────────────────────────────────

/// Correctness: definition variables via @variableName
#[test]
fn variable_resolution() {
    let mut env = FormspecEnvironment::new();
    env.set_variable("taxRate", FelValue::Number(Decimal::from_str("0.08").unwrap()));
    env.set_field("subtotal", num(100));

    assert_eq!(
        eval_value("$subtotal * @taxRate", &env),
        FelValue::Number(Decimal::from_str("8.00").unwrap())
    );
}

/// Correctness: undefined variable returns null
#[test]
fn undefined_variable_returns_null() {
    let env = FormspecEnvironment::new();
    assert_eq!(eval_value("@undefinedVar", &env), FelValue::Null);
}

/// Correctness: variable with object value and dot access
#[test]
fn variable_with_nested_object() {
    let mut env = FormspecEnvironment::new();
    let config = FelValue::Object(vec![
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
    let lookup = FelValue::Object(vec![
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
    assert_eq!(
        eval_value("@instance('missing')", &env),
        FelValue::Null
    );
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
    let addr = FelValue::Object(vec![
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
    assert_eq!(eval_value("$missing", &env), FelValue::Null);
}

/// Correctness: bare $ in repeat context returns current
#[test]
fn bare_dollar_in_repeat_returns_current() {
    let mut env = FormspecEnvironment::new();
    let items = vec![num(42)];
    env.push_repeat(num(42), 1, 1, items);

    assert_eq!(eval_value("$", &env), num(42));
}
