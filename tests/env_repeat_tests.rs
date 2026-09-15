/// FormspecEnvironment + evaluator integration tests — repeat-group context.
///
/// Addresses audit finding: "No tests for FormspecEnvironment + evaluator integration"
///
/// These tests use FormspecEnvironment (not just MapEnvironment) to verify
/// repeat context (`@current`, `@index`, `@count`, `prev()`, `next()`, `parent()`)
/// works through the evaluator.
mod common;

use common::{num, obj, s};
use fel_core::*;

fn eval_with_env(input: &str, env: &FormspecEnvironment) -> EvalResult {
    let expr = parse(input).unwrap();
    evaluate(&expr, env)
}

fn eval_value(input: &str, env: &FormspecEnvironment) -> Value {
    eval_with_env(input, env).value
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

/// A host that binds bare `$` to the node under evaluation (a Bind constraint on a repeat row's field)
/// keeps that binding inside the row; with no binding, bare `$` is the row itself.
#[test]
fn explicit_self_binding_wins_over_the_repeat_row() {
    let mut env = FormspecEnvironment::new();
    let row = obj(vec![("position".to_string(), num(2))]);
    env.push_repeat(row.clone(), 2, 2, vec![row.clone(), row.clone()]);
    assert_eq!(eval_value("$", &env), row);

    env.set_field("", num(2));
    assert_eq!(eval_value("$ = @index", &env), Value::Boolean(true));
}

/// Correctness: repeat context with object values
#[test]
fn repeat_context_with_object_current() {
    let mut env = FormspecEnvironment::new();
    let item = obj(vec![
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

    assert_eq!(eval_value("prev()", &env), Value::Null);
}

/// Correctness: next() at last item returns null
#[test]
fn repeat_next_at_last_returns_null() {
    let mut env = FormspecEnvironment::new();
    let items = vec![num(10), num(20)];
    env.push_repeat(num(20), 2, 2, items);

    assert_eq!(eval_value("next()", &env), Value::Null);
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

// ── Interpolation rule 3a: what counts as reading instance data ──

/// Locale §3.3.1 rule 3a keeps a null result literal only when the expression reads no instance data.
/// Navigation across rows reads it, so a boundary row renders empty rather than the raw template.
#[test]
fn repeat_navigation_counts_as_reading_instance_data() {
    let reads = |source: &str| expr_references_instance_data(&parse(source).unwrap());

    assert!(reads("prev().qty"));
    assert!(reads("next().qty"));
    assert!(reads("parent().label"));
    assert!(reads("instance('claimant').name"));
    assert!(reads("coalesce(prev().qty, 0)"));
    assert!(reads("$qty"));
    assert!(reads("@index"));

    // An author's typo or a call that reads nothing keeps the template, as the rule intends.
    assert!(!reads("nosuchthing"));
    assert!(!reads("format('{0}', 1)"));
    assert!(!reads("1 + 1"));
}
