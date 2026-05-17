#![allow(clippy::missing_docs_in_private_items)]

use std::fs;
use std::path::PathBuf;

use fel_core::{MapEnvironment, evaluate, json_object_to_field_map, parse};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct Case {
    expression: String,
    environment: Value,
    #[serde(rename = "expectedValue")]
    expected_value: Value,
    #[serde(rename = "expectedDiagnosticKinds")]
    expected_diagnostic_kinds: Vec<String>,
}

fn diagnostic_kind_name(kind: &fel_core::DiagnosticKind) -> &'static str {
    match kind {
        fel_core::DiagnosticKind::UndefinedFunction { .. } => "UndefinedFunction",
        fel_core::DiagnosticKind::TypeMismatch { .. } => "TypeMismatch",
    }
}

#[test]
fn public_conformance_corpus_matches_runtime() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("conformance")
        .join("fel-conformance.jsonl");
    let raw = fs::read_to_string(root).expect("read public conformance corpus");

    for (index, line) in raw.lines().enumerate() {
        let case: Case = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("line {}: invalid fixture: {e}", index + 1));
        let expr = parse(&case.expression)
            .unwrap_or_else(|e| panic!("line {}: parse failed: {e}", index + 1));
        let fields = json_object_to_field_map(&case.environment);
        let env = MapEnvironment::with_fields(fields);
        let result = evaluate(&expr, &env);
        let value_json = fel_core::fel_to_json(&result.value);
        assert_eq!(
            value_json,
            case.expected_value,
            "line {} expression {}",
            index + 1,
            case.expression
        );

        let kinds: Vec<String> = result
            .diagnostics
            .iter()
            .filter_map(|d| {
                d.kind
                    .as_ref()
                    .map(|kind| diagnostic_kind_name(kind).to_string())
            })
            .collect();
        assert_eq!(
            kinds,
            case.expected_diagnostic_kinds,
            "line {} expression {}",
            index + 1,
            case.expression
        );
    }
}
