//! Catalog → dispatch consistency test.
//!
//! Asserts every entry in `BUILTIN_FUNCTIONS` is recognized by the evaluator's
//! `eval_function` dispatch. Drift between the catalog and the dispatcher would
//! silently break tooling that consumes the catalog (wos-lint, WASM surfaces,
//! IDE autocomplete).

use fel_core::{MapEnvironment, builtin_function_catalog, evaluate, parse};

#[test]
#[should_panic(expected = "has no dispatch arm — diagnostic")]
fn gate_fires_for_fake_entry() {
    // Use a name that is definitely not in the catalog to verify the assertion logic.
    let env = MapEnvironment::new();
    let fake_name = "thisDoesNotExist";
    let expr_src = format!("{}()", fake_name);
    let parsed = parse(&expr_src).expect("parseable");
    let result = evaluate(&parsed, &env);
    for diag in &result.diagnostics {
        let s = format!("{:?}", diag);
        assert!(
            !s.to_lowercase().contains("undefined function"),
            "Catalog entry '{}' has no dispatch arm — diagnostic: {}",
            fake_name,
            s
        );
    }
}

#[test]
fn every_catalog_entry_is_dispatched() {
    /// Names that cannot appear as a bare `ident()` call token (lexer keywords, etc.).
    /// Document each entry when extending — silent skips hide dispatch drift.
    const NOT_PARSEABLE_AS_IDENT_CALL: &[&str] = &[];

    let env = MapEnvironment::new();
    for entry in builtin_function_catalog() {
        if NOT_PARSEABLE_AS_IDENT_CALL.contains(&entry.name) {
            continue;
        }

        let expr_src = format!("{}()", entry.name);
        let parsed = parse(&expr_src).unwrap_or_else(|e| {
            panic!(
                "catalog entry '{}' must parse as `{}()` so dispatch can be exercised: {e}",
                entry.name, entry.name
            )
        });
        let result = evaluate(&parsed, &env);

        for diag in &result.diagnostics {
            let s = format!("{:?}", diag);
            assert!(
                !s.to_lowercase().contains("undefined function"),
                "Catalog entry '{}' has no dispatch arm — diagnostic: {}",
                entry.name,
                s
            );
        }
    }
}
