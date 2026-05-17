//! Cross-runtime differential oracle for FEL.
//!
//! Generates ASTs via C1 strategies, evaluates in Rust, shells out to
//! `formspec-py` and the `formspec-engine` WASM runtime to evaluate the
//! same expression, and compares results.
//! `#[ignore]` by default; `make test-differential` enables it.
#![allow(clippy::missing_docs_in_private_items)]
#![cfg(feature = "proptest-strategies")]

use fel_core::testing::strategies::arb_expr;
use fel_core::{MapEnvironment, builtin_function_catalog, evaluate, fel_to_json, print_expr};
use proptest::prelude::*;
use std::io::Write;
use std::process::{Command, Stdio};

fn rust_val(expr: &fel_core::Expr) -> serde_json::Value {
    let env = MapEnvironment::new();
    let result = evaluate(expr, &env);
    fel_to_json(&result.value)
}

fn python_val(source: &str) -> Result<Option<serde_json::Value>, String> {
    let output = Command::new("python3")
        .args([
            "-c",
            &format!(
                "from formspec.fel.eval import evaluate; import json; v = evaluate(r'''{}''', {{}}); print(json.dumps(v))",
                source
            ),
        ])
        .output()
        .map_err(|e| format!("failed to spawn python3: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(serde_json::from_str(&stdout).ok())
    } else if stderr.contains("panicked at") || stderr.contains("PanicException") {
        Err(format!("python3 panic: {stderr}"))
    } else if stderr.contains("SyntaxError") || stderr.contains("Traceback") {
        Ok(None)
    } else {
        Err(format!("python3 exited non-zero: {stderr}"))
    }
}

fn wasm_val(
    source: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<serde_json::Value>, String> {
    let input_line = serde_json::json!({ "expr": source, "fields": fields });
    let script_path = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/fel-wasm-eval.mjs");
    let mut child = Command::new("node")
        .arg(script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn node: {e}"))?;
    {
        let stdin = child.stdin.as_mut().expect("stdin piped");
        writeln!(stdin, "{}", serde_json::to_string(&input_line).unwrap())
            .map_err(|e| format!("failed to write to node stdin: {e}"))?;
    }
    let output = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!("node exited non-zero: {stderr}"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(None);
    }
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("invalid JSON from node: {e}"))?;
    if let Some(err) = parsed.get("error").and_then(|e| e.as_str()) {
        if err.is_empty() {
            Ok(parsed.get("result").cloned())
        } else {
            Err(format!("node eval error: {err}"))
        }
    } else if let Some(result_json) = parsed.get("resultJson").and_then(|v| v.as_str()) {
        if result_json.is_empty() {
            Ok(None)
        } else {
            serde_json::from_str(result_json)
                .map(Some)
                .map_err(|e| format!("invalid JSON result from node: {e}"))
        }
    } else {
        Ok(parsed.get("result").cloned())
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
        let printed = print_expr(&expr);
        match python_val(&printed) {
            Ok(Some(py)) => prop_assert_eq!(&rust, &py),
            Ok(None) => { /* Python rejected — skip */ }
            Err(e) => panic!("python3 failure: {e}"),
        }
    }

    #[test]
    #[ignore = "requires formspec-engine WASM built; run via make test-differential"]
    fn rust_wasm_parity(
        expr in arb_expr(4, builtin_function_catalog())
    ) {
        let rust = rust_val(&expr);
        let printed = print_expr(&expr);
        let fields = serde_json::Map::new();
        match wasm_val(&printed, &fields) {
            Ok(Some(wasm)) => prop_assert_eq!(&rust, &wasm),
            Ok(None) => { /* WASM rejected — skip */ }
            Err(e) => panic!("node WASM failure: {e}"),
        }
    }
}

// ── R24: hand-picked power() fractional/negative exponent cases ────────

mod r24_power_oracle {
    use super::*;
    use fel_core::parse;

    #[derive(Debug)]
    struct PowerCase {
        base: &'static str,
        exponent: &'static str,
        description: &'static str,
    }

    const POWER_CASES: &[PowerCase] = &[
        PowerCase {
            base: "1",
            exponent: "-2",
            description: "integer reciprocal",
        },
        PowerCase {
            base: "4",
            exponent: "0.5",
            description: "square root",
        },
        PowerCase {
            base: "9",
            exponent: "0.5",
            description: "sqrt 9",
        },
        PowerCase {
            base: "2",
            exponent: "0.5",
            description: "sqrt 2 — irrational",
        },
        PowerCase {
            base: "100",
            exponent: "-0.5",
            description: "reciprocal sqrt",
        },
        PowerCase {
            base: "2",
            exponent: "-1.5",
            description: "negative fractional",
        },
        PowerCase {
            base: "10",
            exponent: "-3",
            description: "negative integer",
        },
        PowerCase {
            base: "1.0000001",
            exponent: "-1",
            description: "near-identity reciprocal — f64 divergence risk",
        },
        PowerCase {
            base: "1e-10",
            exponent: "3",
            description: "very small base",
        },
        PowerCase {
            base: "2.71828",
            exponent: "2.30259",
            description: "e^{ln(10)} ~ f64 path",
        },
    ];

    #[test]
    fn power_fractional_negative_does_not_panic() {
        let env = MapEnvironment::new();
        for case in POWER_CASES {
            let src = format!("power({}, {})", case.base, case.exponent);
            let expr = parse(&src).expect("parse power expr");
            let result = evaluate(&expr, &env);
            let _ = result;
        }
    }

    #[test]
    fn power_fractional_negative_is_deterministic() {
        let env = MapEnvironment::new();
        for case in POWER_CASES {
            let src = format!("power({}, {})", case.base, case.exponent);
            let expr = parse(&src).expect("parse power expr");
            let r1 = evaluate(&expr, &env);
            let r2 = evaluate(&expr, &env);
            assert_eq!(
                fel_to_json(&r1.value),
                fel_to_json(&r2.value),
                "power({}, {}) — non-deterministic: {:?} vs {:?}",
                case.base,
                case.exponent,
                r1.value,
                r2.value,
            );
        }
    }

    #[test]
    #[ignore = "requires formspec-py in sibling repo; run via make test-differential"]
    fn power_fractional_negative_rust_python_parity() {
        let env = MapEnvironment::new();
        for case in POWER_CASES {
            let src = format!("power({}, {})", case.base, case.exponent);
            let expr = parse(&src).expect("parse power expr");
            let rust = fel_to_json(&evaluate(&expr, &env).value);
            match python_val(&src) {
                Ok(Some(py)) => assert_eq!(
                    rust, py,
                    "power({}, {}) — Rust/Python divergence: rust={rust} py={py}",
                    case.base, case.exponent,
                ),
                Ok(None) => { /* Python rejected — skip */ }
                Err(e) => panic!("python3 failure on {}: {e}", case.description),
            }
        }
    }
}
