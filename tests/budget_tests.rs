//! Resource budget enforcement tests.
//!
//! Verifies that step, deadline budgets terminate evaluation
//! without panicking, and that the default evaluate entry points are unaffected.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{evaluate_with_budget, parse, EvalBudget, MapEnvironment, Value};
use std::time::Instant;

fn eval_budget(src: &str, budget: &EvalBudget) -> fel_core::EvalResult {
    let expr = parse(src).unwrap();
    let env = MapEnvironment::new();
    evaluate_with_budget(&expr, &env, budget)
}

#[test]
fn step_budget_stops_long_addition_chain() {
    let budget = EvalBudget {
        max_steps: 100,
        ..EvalBudget::unlimited()
    };
    let chain = "0".to_string() + &" + 1".repeat(200);
    let result = eval_budget(&chain, &budget);
    let has_budget_diag = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("budget exceeded"));
    assert!(has_budget_diag);
}

#[test]
fn step_budget_stops_deep_function_nesting() {
    let budget = EvalBudget {
        max_steps: 5,
        ..EvalBudget::unlimited()
    };
    // 20 levels of upper() nesting, each level costs 2-3 steps.
    // With max_steps=5, this should be caught.
    let mut expr = String::from("'hello'");
    for _ in 0..20 {
        expr = format!("upper({})", expr);
    }
    let result = eval_budget(&expr, &budget);
    let has_budget_diag = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("budget exceeded"));
    assert!(has_budget_diag);
}

#[test]
fn unlimited_budget_completes_small_expr() {
    let result = eval_budget("1 + 2", &EvalBudget::unlimited());
    assert_eq!(result.value, Value::Number(3.into()));
    assert!(result.diagnostics.is_empty());
}

#[test]
fn deadline_budget_stops_long_eval() {
    let budget = EvalBudget {
        max_steps: u64::MAX,
        max_alloc_bytes: u64::MAX,
        deadline: Some(Instant::now()),
    };
    let chain = "0".to_string() + &" + 1".repeat(5000);
    let result = eval_budget(&chain, &budget);
    let has_budget_diag = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("budget exceeded"));
    assert!(has_budget_diag);
}

#[test]
fn budget_never_panics_on_pathological_input() {
    let budget = EvalBudget {
        max_steps: 50,
        ..EvalBudget::unlimited()
    };
    let large = "0".to_string() + &" + 1".repeat(10000);
    let result = eval_budget(&large, &budget);
    let _ = result;
}

#[test]
fn budget_does_not_affect_normal_callers() {
    let result = fel_core::evaluate(
        &parse("1 + 2 + 3").unwrap(),
        &MapEnvironment::new(),
    );
    assert_eq!(result.value, Value::Number(6.into()));
}

#[test]
fn step_budget_elem_consistency() {
    let budget = EvalBudget {
        max_steps: 3,
        ..EvalBudget::unlimited()
    };
    let result = eval_budget("1 + 2 + 3 + 4 + 5", &budget);
    let has_budget_diag = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("budget exceeded"));
    assert!(has_budget_diag);
}

#[test]
fn alloc_budget_stops_array_construction() {
    let budget = EvalBudget {
        max_steps: u64::MAX,
        max_alloc_bytes: 10,
        deadline: None,
    };
    // Array of 1 element → track_alloc(1 * 16) = 16 bytes > 10 → exceeded
    let result = eval_budget("[1]", &budget);
    let has_budget_diag = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("budget exceeded (alloc)"));
    assert!(has_budget_diag);
    assert!(matches!(result.value, Value::Null));
}

#[test]
fn alloc_budget_stops_object_construction() {
    let budget = EvalBudget {
        max_steps: u64::MAX,
        max_alloc_bytes: 10,
        deadline: None,
    };
    // Object of 1 entry → track_alloc(1 * 40) = 40 bytes > 10 → exceeded
    let result = eval_budget("{'a': 1}", &budget);
    let has_budget_diag = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("budget exceeded (alloc)"));
    assert!(has_budget_diag);
    assert!(matches!(result.value, Value::Null));
}

#[test]
fn alloc_budget_stops_let_binding_overhead() {
    let budget = EvalBudget {
        max_steps: u64::MAX,
        max_alloc_bytes: 50,
        deadline: None,
    };
    // `let` charges a fixed allocation estimate (64 bytes) after the bound value — exceeds 50.
    let result = eval_budget("let x = 1 in x", &budget);
    let has_budget_diag = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("budget exceeded (alloc)"));
    assert!(has_budget_diag);
    assert!(matches!(result.value, Value::Null));
}

#[test]
fn alloc_budget_stops_string_literal() {
    let budget = EvalBudget {
        max_steps: u64::MAX,
        max_alloc_bytes: 3,
        deadline: None,
    };
    // String of 4 bytes → track_alloc(4) > 3 → exceeded
    let result = eval_budget("'test'", &budget);
    let has_budget_diag = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("budget exceeded (alloc)"));
    assert!(has_budget_diag);
    assert!(matches!(result.value, Value::Null));
}
