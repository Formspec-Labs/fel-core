//! Conformance harness against the sibling formspec runtime's
//! `fel-function-semantics.json` fixture.
//!
//! Like `schema_round_trip`, this test relies on a sibling submodule
//! (`../formspec/`). Outside the stack-root checkout (e.g. cargo-mutants'
//! scratch tree), the fixture is absent — the test skips gracefully with
//! a notice. In the normal stack-root checkout, the fixture resolves and
//! every row runs.

use std::fs;
use std::path::PathBuf;

use fel_core::{MapEnvironment, evaluate, parse};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    expr: String,
    data: Value,
    expected_value: Value,
    #[serde(default)]
    expected_diagnostic_codes: Vec<String>,
}

fn fixture_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("FEL_CORE_FUNCTION_SEMANTICS_FIXTURE") {
        return PathBuf::from(p).canonicalize().ok();
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("formspec")
        .join("tests")
        .join("conformance")
        .join("fel-function-semantics.json")
        .canonicalize()
        .ok()
}

#[test]
fn fel_function_semantics_fixture_matches_runtime() {
    let Some(path) = fixture_path() else {
        eprintln!(
            "function_semantics_conformance: skipped — sibling fixture not \
             found at ../formspec/tests/conformance/fel-function-semantics.json. \
             Set FEL_CORE_FUNCTION_SEMANTICS_FIXTURE to override."
        );
        return;
    };
    let raw = fs::read_to_string(&path).expect("read fel-function-semantics fixture");
    let cases: Vec<Case> =
        serde_json::from_str(&raw).expect("parse fel-function-semantics fixture");

    for case in cases {
        let expr = parse(&case.expr).unwrap_or_else(|e| panic!("{}: parse failed: {e}", case.id));
        let fields = fel_core::json_object_to_field_map(&case.data);
        let env = MapEnvironment::with_fields(fields);
        let result = evaluate(&expr, &env);
        let value_json = fel_core::fel_to_ui_json(&result.value);
        assert_eq!(value_json, case.expected_value, "{}", case.id);

        let codes: Vec<String> = result
            .diagnostics
            .iter()
            .filter_map(|d| d.code.clone())
            .collect();
        assert_eq!(codes, case.expected_diagnostic_codes, "{}", case.id);
    }
}
