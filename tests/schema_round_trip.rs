//! Round-trip test: fel-core's emitted FEL schema matches the canonical
//! `formspec/schemas/fel-functions.schema.json` byte-for-byte (up to JSON
//! semantic equivalence — key order in objects doesn't matter).
//!
//! The canonical schema lives in a sibling submodule (`../formspec/`).
//! When this test compiles in an isolated tree (e.g. cargo-mutants' scratch
//! copy of fel-core, or a `cargo publish` dry-run), the sibling is absent
//! and the test skips gracefully with a notice. In the normal stack-root
//! checkout the sibling resolves and the assertion runs.

use std::path::{Path, PathBuf};

fn canonical_schema_path() -> Option<PathBuf> {
    // Explicit env-var override takes priority — useful for CI or custom
    // checkouts that place the sibling somewhere non-default.
    if let Some(p) = std::env::var_os("FEL_CORE_CANONICAL_SCHEMA") {
        let path = PathBuf::from(p);
        return path.canonicalize().ok();
    }
    let default =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../formspec/schemas/fel-functions.schema.json");
    default.canonicalize().ok()
}

#[test]
fn emitted_schema_matches_canonical() {
    let Some(path) = canonical_schema_path() else {
        // `println!` (not `eprintln!`) so `cargo test -- --nocapture`
        // surfaces this skip — silent passes mask the case where the
        // sibling has moved, been deinitialized, or has a renamed path.
        println!(
            "schema_round_trip: SKIPPED — canonical schema not found at \
             ../formspec/schemas/fel-functions.schema.json. \
             Set FEL_CORE_CANONICAL_SCHEMA to override. (This test \
             requires the sibling formspec/ submodule.)"
        );
        return;
    };
    let canonical_str = std::fs::read_to_string(&path).expect("read canonical schema");
    let canonical: serde_json::Value =
        serde_json::from_str(&canonical_str).expect("parse canonical schema");
    let emitted = fel_core::extensions::emit_schema_json();
    assert_eq!(
        emitted,
        canonical,
        "fel-core emission diverges from canonical schema at {}",
        path.display()
    );
}

#[test]
fn count_example_matches_conformance() {
    let expression = "count([1, null, 3])";
    let example = fel_core::builtin_function_catalog()
        .iter()
        .find(|entry| entry.name == "count")
        .expect("count in catalog")
        .examples
        .iter()
        .find(|example| example.expression == expression)
        .expect("count example with a null slot");
    let fixture = include_str!("../conformance/fel-conformance.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("conformance fixture"))
        .find(|fixture| fixture["expression"] == expression)
        .expect("conformance case for the count example");
    let documented: serde_json::Value =
        serde_json::from_str(example.result_json).expect("catalog result");
    assert_eq!(documented, fixture["expectedValue"], "{expression}");
}
