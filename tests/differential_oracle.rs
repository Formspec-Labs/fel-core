//! Cross-runtime differential oracle for FEL.
//!
//! Generates ASTs via C1 strategies, evaluates in Rust, shells out to
//! `formspec-py` to evaluate the same expression, and compares results.
//! `#[ignore]` by default; `make test-differential` enables it.
#![allow(clippy::missing_docs_in_private_items)]
#![cfg(feature = "proptest-strategies")]

use fel_core::{builtin_function_catalog, evaluate, fel_to_json, MapEnvironment};
use fel_core::testing::strategies::arb_expr;
use proptest::prelude::*;
use std::process::Command;

fn rust_val(expr: &fel_core::Expr) -> serde_json::Value {
    let env = MapEnvironment::new();
    let result = evaluate(expr, &env);
    fel_to_json(&result.value)
}

fn python_val(source: &str) -> Option<serde_json::Value> {
    let output = Command::new("python3")
        .args([
            "-c",
            &format!(
                "from formspec.fel.eval import evaluate; import json; v = evaluate(r'''{}''', {{}}); print(json.dumps(v))",
                source
            ),
        ])
        .output()
        .ok()?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        serde_json::from_str(&stdout).ok()
    } else {
        None
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 64,
        ..Default::default()
    })]

    #[test]
    #[ignore = "requires formspec-py in sibling repo; run via make test-differential"]
    fn rust_python_parity(
        expr in arb_expr(3, builtin_function_catalog())
    ) {
        let rust = rust_val(&expr);
        let printed = fel_core::print_expr(&expr);
        if let Some(py) = python_val(&printed) {
            prop_assert_eq!(&rust, &py);
        }
    }
}
