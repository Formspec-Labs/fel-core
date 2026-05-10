//! LibFuzzer harness: arbitrary bytes → parse → evaluate_with (500 steps, 64 KiB alloc).
//!
//! Asserts that budget-limited evaluation never panics and that the result is consistent:
//! either a valid Value, or Value::Null with a budget exceeded diagnostic.
#![no_main]

use fel_core::{evaluate_with, parse, EvalBudget, EvaluatorOptions, MapEnvironment, Value};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let src = String::from_utf8_lossy(data);

    let Ok(expr) = parse(src.as_ref()) else {
        return;
    };

    let env = MapEnvironment::new();
    let budget = EvalBudget {
        max_steps: 500,
        max_alloc_bytes: 64 * 1024,
        deadline: None,
    };
    let result = evaluate_with(&expr, &env, EvaluatorOptions {
        budget,
        ..EvaluatorOptions::default()
    });

    let has_budget_diag = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("budget exceeded"));

    if has_budget_diag {
        assert!(
            matches!(result.value, Value::Null),
            "Budget exceeded but value is not Null"
        );
    }
});
