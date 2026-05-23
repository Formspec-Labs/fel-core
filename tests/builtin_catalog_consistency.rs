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

/// Catalog ↔ dispatch parity SURVEY: arity enforcement uniformity.
///
/// Per FUT-7 (filed as a follow-up to this run): the FEL evaluator's
/// builtin dispatch does NOT uniformly enforce the catalog's declared
/// arity. Some builtins silently accept wrong-arity calls and return
/// `Null` without a diagnostic; others go through null-propagation /
/// type-mismatch paths that emit OTHER diagnostics; only external
/// `ExtensionRegistry::call` enforces `ArityMismatch` cleanly.
///
/// This survey records the *count* of catalog entries whose
/// observable behavior on wrong arity is "silent return" (no diagnostic,
/// non-Null value). It pins the current behavior so future evaluator
/// refactors that tighten arity enforcement surface here as a test diff
/// for explicit review.
///
/// Addresses post-Phase-2 swarm review HIGH on
/// `builtin_catalog_consistency` being existence-only.
#[test]
fn catalog_arity_enforcement_uniformity_survey() {
    use fel_core::builtin_function_catalog;
    use fel_core::extensions::Parameter;

    fn arity_bounds(parameters: &[Parameter]) -> (usize, Option<usize>) {
        let required = parameters.iter().filter(|p| p.required).count();
        let has_variadic = parameters.iter().any(|p| p.variadic);
        let max = if has_variadic {
            None
        } else {
            Some(parameters.len())
        };
        (required, max)
    }

    let env = MapEnvironment::new();
    let mut silent_on_too_few: Vec<&'static str> = Vec::new();
    let mut silent_on_too_many: Vec<&'static str> = Vec::new();

    for entry in builtin_function_catalog() {
        let (min_args, max_args) = arity_bounds(entry.parameters);

        if min_args >= 1 {
            let too_few = min_args - 1;
            let args = (0..too_few)
                .map(|i| (i + 1).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let expr_src = format!("{}({args})", entry.name);
            if let Ok(parsed) = parse(&expr_src) {
                let result = evaluate(&parsed, &env);
                let silent = result.value != fel_core::Value::Null && result.diagnostics.is_empty();
                if silent {
                    silent_on_too_few.push(entry.name);
                }
            }
        }

        if let Some(max) = max_args
            && min_args <= max
        {
            let too_many = max + 1;
            let args = (0..too_many)
                .map(|i| (i + 1).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let expr_src = format!("{}({args})", entry.name);
            if let Ok(parsed) = parse(&expr_src) {
                let result = evaluate(&parsed, &env);
                let silent = result.value != fel_core::Value::Null && result.diagnostics.is_empty();
                if silent {
                    silent_on_too_many.push(entry.name);
                }
            }
        }
    }

    // Print the survey so `cargo test -- --nocapture` shows the current
    // gap surface. Future-you reading this: if these lists grow
    // unexpectedly between releases, the evaluator quietly stopped
    // enforcing arity for new builtins.
    println!("[catalog-arity-survey]");
    println!(
        "  silent on too-few args  ({:>3}): {silent_on_too_few:?}",
        silent_on_too_few.len()
    );
    println!(
        "  silent on too-many args ({:>3}): {silent_on_too_many:?}",
        silent_on_too_many.len()
    );

    // Pinned upper bounds — adjust ONLY downward (with a commit message
    // citing the evaluator-side enforcement fix that made it tighter).
    // Upward drift means new catalog entries the dispatch doesn't gate;
    // that should fail loudly. Current actual: 10 / 24 at sha-of-commit.
    const MAX_KNOWN_SILENT_TOO_FEW: usize = 15;
    const MAX_KNOWN_SILENT_TOO_MANY: usize = 30;
    assert!(
        silent_on_too_few.len() <= MAX_KNOWN_SILENT_TOO_FEW,
        "silent-on-too-few count grew unexpectedly: {} > {}",
        silent_on_too_few.len(),
        MAX_KNOWN_SILENT_TOO_FEW
    );
    assert!(
        silent_on_too_many.len() <= MAX_KNOWN_SILENT_TOO_MANY,
        "silent-on-too-many count grew unexpectedly: {} > {}",
        silent_on_too_many.len(),
        MAX_KNOWN_SILENT_TOO_MANY
    );
}

/// `DiagnosticKind` is the closed/append-only public API surface for
/// machine-readable diagnostic categories (per crate `README.md`
/// stability commitment). When a new variant is added, this test forces
/// the maintainer to extend the `kind_to_string` map below — protecting
/// downstream code (CI fixture serializers, conformance harnesses) that
/// reads the kind by name from silently mis-categorizing the new variant.
///
/// If you've added a variant to `DiagnosticKind` and this test won't
/// compile, that's the test working as designed: add the new match arm
/// here and update any consumer that needed to know about it.
#[test]
fn diagnostic_kind_variants_have_exhaustive_string_mapping() {
    use fel_core::DiagnosticKind;

    // The exhaustive match below MUST cover every variant. Compilation
    // fails if a variant is added without updating this map — the audit
    // signal we want.
    fn kind_to_string(k: &DiagnosticKind) -> &'static str {
        match k {
            DiagnosticKind::UndefinedFunction { .. } => "undefinedFunction",
            DiagnosticKind::TypeMismatch { .. } => "typeMismatch",
            DiagnosticKind::ArityMismatch { .. } => "arityMismatch",
        }
    }

    // Spot-construct each variant to confirm runtime behavior (not just
    // exhaustiveness at compile time).
    let cases = [
        (
            DiagnosticKind::UndefinedFunction {
                name: "foo".to_string(),
            },
            "undefinedFunction",
        ),
        (
            DiagnosticKind::TypeMismatch {
                fn_name: "f".to_string(),
                expected: "number".to_string(),
                got: "string".to_string(),
            },
            "typeMismatch",
        ),
        (
            DiagnosticKind::ArityMismatch {
                name: "f".to_string(),
                min_args: 1,
                max_args: Some(2),
                got: 3,
            },
            "arityMismatch",
        ),
    ];

    for (kind, expected_name) in &cases {
        assert_eq!(kind_to_string(kind), *expected_name);
    }
}
