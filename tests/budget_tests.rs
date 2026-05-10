//! Resource budget enforcement tests.
//!
//! Verifies that step, deadline budgets terminate evaluation
//! without panicking, and that the default evaluate entry points are unaffected.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{
    EvalBudget, ExtensionRegistry, MapEnvironment, Value, evaluate_with_budget,
    evaluate_with_budget_and_extensions, parse,
};
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
    let result = fel_core::evaluate(&parse("1 + 2 + 3").unwrap(), &MapEnvironment::new());
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

// ── R21: duplicate budget diagnostics ────────────────────────

#[test]
fn alloc_breach_emits_exactly_one_diagnostic() {
    let budget = EvalBudget {
        max_steps: u64::MAX,
        max_alloc_bytes: 10,
        deadline: None,
    };
    let result = eval_budget("[1 + 2, 3 + 4, 5 + 6]", &budget);
    let budget_count = result
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("budget exceeded"))
        .count();
    assert_eq!(
        budget_count, 1,
        "expected exactly one budget diagnostic, got {budget_count}: {:?}",
        result.diagnostics
    );
    assert!(matches!(result.value, Value::Null));
}

#[test]
fn step_breach_emits_exactly_one_diagnostic() {
    let budget = EvalBudget {
        max_steps: 5,
        ..EvalBudget::unlimited()
    };
    let chain = "0".to_string() + &" + 1".repeat(200);
    let result = eval_budget(&chain, &budget);
    let budget_count = result
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("budget exceeded"))
        .count();
    assert_eq!(
        budget_count, 1,
        "expected exactly one budget diagnostic, got {budget_count}"
    );
}

// ── R22: Concat alloc budget enforcement ─────────────────────

#[test]
fn concat_respects_alloc_budget() {
    let budget = EvalBudget {
        max_steps: u64::MAX,
        max_alloc_bytes: 12,
        deadline: None,
    };
    // 'hello' (5 bytes) + ' world' (6 bytes) = 11 bytes from literals.
    // The concat tracks 5+6=11, pushing total to 22 > 12 → breach.
    let result = eval_budget("'hello' & ' world'", &budget);
    let has_budget_diag = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("budget exceeded (alloc)"));
    assert!(has_budget_diag);
    assert!(matches!(result.value, Value::Null));
}

#[test]
fn deep_concat_tree_respects_alloc_budget() {
    let budget = EvalBudget {
        max_steps: u64::MAX,
        max_alloc_bytes: 31,
        deadline: None,
    };
    // 'abcde' & 'abcde' (10) & 'abcde' (15) & 'abcde' (20)
    // & 'abcde' (25) & 'abcde' (30) & 'abcde' (35 → breach at 35 > 31)
    let expr = "'abcde' & 'abcde' & 'abcde' & 'abcde' & 'abcde' & 'abcde' & 'abcde'";
    let result = eval_budget(expr, &budget);
    let budget_count = result
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("budget exceeded (alloc)"))
        .count();
    assert_eq!(budget_count, 1);
    assert!(matches!(result.value, Value::Null));
}

// ── R32: extension result alloc budget enforcement ───────────

#[test]
fn extension_result_respects_alloc_budget() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register("bigResult", 0, None, |_args| {
            Value::String("x".repeat(1024))
        })
        .expect("register");
    let budget = EvalBudget {
        max_steps: u64::MAX,
        max_alloc_bytes: 64,
        deadline: None,
    };
    let expr = parse("bigResult()").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate_with_budget_and_extensions(&expr, &env, &registry, &budget);
    let has_budget_diag = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("budget exceeded (alloc)"));
    assert!(has_budget_diag);
    assert!(matches!(result.value, Value::Null));
}

#[test]
fn extension_small_result_within_alloc_budget() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register("smallResult", 0, None, |_args| {
            Value::String("ok".to_string())
        })
        .expect("register");
    let budget = EvalBudget {
        max_steps: u64::MAX,
        max_alloc_bytes: 1024,
        deadline: None,
    };
    let expr = parse("smallResult()").unwrap();
    let env = MapEnvironment::new();
    let result = evaluate_with_budget_and_extensions(&expr, &env, &registry, &budget);
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.value, Value::String("ok".to_string()));
}
