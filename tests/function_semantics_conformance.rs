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

fn load_cases() -> Vec<Case> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("formspec")
        .join("tests")
        .join("conformance")
        .join("fel-function-semantics.json");
    let raw = fs::read_to_string(root).expect("read fel-function-semantics fixture");
    serde_json::from_str(&raw).expect("parse fel-function-semantics fixture")
}

#[test]
fn fel_function_semantics_fixture_matches_runtime() {
    for case in load_cases() {
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
