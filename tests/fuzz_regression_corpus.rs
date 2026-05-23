//! Fuzz-regression corpus (FEL-SMELL-C-001).
//!
//! Replays fuzz-discovered inputs from `tests/corpus/fuzz_regression.jsonl`
//! through `parse` + `evaluate`. Each row carries:
//!
//!   - `id`: stable identifier for the case.
//!   - `expression`: FEL source.
//!   - `mustParse`: whether the parser MUST accept this input. Rows with
//!     `mustParse: false` guard against accidental grammar loosening on
//!     junk inputs.
//!   - `displayOracle` (optional): pins `Value::Display` for parseable
//!     rows. After intentional display changes, run
//!     `make fuzz-regression-refresh` (see `tests/corpus/README.md`).
//!
//! Lives in its own file (not `evaluator_regression_guards.rs`) because
//! it's corpus-driven, not bespoke-AST-construction.

use fel_core::*;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct FuzzRegressionCase {
    id: String,
    expression: String,
    #[serde(rename = "mustParse")]
    must_parse: bool,
    #[serde(default, rename = "displayOracle")]
    display_oracle: Option<String>,
}

#[test]
fn fuzz_regression_corpus() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/fuzz_regression.jsonl");
    let raw = fs::read_to_string(&path).expect("read fuzz regression corpus");

    for (index, line) in raw.lines().enumerate() {
        let line_no = index + 1;
        let case: FuzzRegressionCase = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("line {line_no}: invalid corpus row: {e}"));

        match parse(&case.expression) {
            Ok(expr) => {
                assert!(
                    case.must_parse,
                    "line {line_no} id {}: parse succeeded but mustParse is false",
                    case.id
                );
                let result = evaluate(&expr, &MapEnvironment::new());
                let display = format!("{}", result.value);
                if let Some(expected) = &case.display_oracle {
                    assert_eq!(
                        display, *expected,
                        "line {line_no} id {} expression {:?}",
                        case.id, case.expression
                    );
                }
            }
            Err(err) => {
                assert!(
                    !case.must_parse,
                    "line {line_no} id {}: parse regression: {err}",
                    case.id
                );
            }
        }
    }
}
