//! Catalog → dispatch consistency tests.
//!
//! Asserts every entry in `BUILTIN_FUNCTIONS` is recognized by the evaluator's
//! `eval_function` dispatch AND that catalog-declared arity is uniformly
//! enforced — FUT-7. Drift between the catalog and the dispatcher would
//! silently break tooling that consumes the catalog (wos-lint, WASM surfaces,
//! IDE autocomplete).

use fel_core::error::DiagnosticKind;
use fel_core::{MapEnvironment, builtin_arity, builtin_function_catalog, evaluate, parse};

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

/// Every catalog entry's `arity()` derivation matches the underlying parameter slice.
///
/// `min_args` counts leading required, non-variadic params plus one when the
/// trailing param is `required: true, variadic: true` (the contract is "at
/// least one of these"). `max_args` is `None` when the trailing param is
/// variadic, otherwise total parameter count. The lookup helper must agree.
#[test]
fn catalog_arity_helper_matches_entries() {
    for entry in builtin_function_catalog() {
        let (min, max) = entry.arity();
        // Sanity: min cannot exceed max when bounded.
        if let Some(m) = max {
            assert!(min <= m, "{}: min={min} > max={m}", entry.name);
        }
        // The lookup helper must agree with the method derivation.
        let via_lookup = builtin_arity(entry.name)
            .unwrap_or_else(|| panic!("builtin_arity({}) returned None", entry.name));
        assert_eq!(
            via_lookup,
            (min, max),
            "{}: arity lookup mismatch",
            entry.name
        );
    }
}

/// Uniformity guarantee (FUT-7): every catalog-declared builtin rejects both
/// "too few" and "too many" arg counts with the structured arity diagnostic.
/// No builtin silently accepts under-arity (defaulting missing positions to
/// null) or over-arity (ignoring trailing args).
///
/// This test is the closure of the previous survey-style audit: counts of
/// silently-accepting builtins must be exactly zero.
#[test]
fn catalog_arity_uniformly_enforced_at_dispatch() {
    let env = MapEnvironment::new();
    let mut silent_on_too_few = Vec::<String>::new();
    let mut silent_on_too_many = Vec::<String>::new();

    for entry in builtin_function_catalog() {
        let (min, max) = entry.arity();

        // Probe under-arity if the function takes at least one required arg.
        if min > 0 {
            // Call with `min - 1` null args (always parseable as `null` literals).
            let arg_list = vec!["null"; min - 1].join(", ");
            let src = format!("{}({})", entry.name, arg_list);
            let parsed = parse(&src).unwrap_or_else(|e| {
                panic!(
                    "under-arity probe for {} failed to parse `{}`: {e}",
                    entry.name, src
                )
            });
            let result = evaluate(&parsed, &env);
            let rejected = result.diagnostics.iter().any(|d| {
                matches!(
                    &d.kind,
                    Some(DiagnosticKind::ArityMismatch { name, .. }) if name == entry.name
                )
            });
            if !rejected {
                silent_on_too_few.push(entry.name.to_string());
            }
        }

        // Probe over-arity if the function has a bounded upper limit.
        if let Some(m) = max {
            let arg_list = vec!["null"; m + 1].join(", ");
            let src = format!("{}({})", entry.name, arg_list);
            let parsed = parse(&src).unwrap_or_else(|e| {
                panic!(
                    "over-arity probe for {} failed to parse `{}`: {e}",
                    entry.name, src
                )
            });
            let result = evaluate(&parsed, &env);
            let rejected = result.diagnostics.iter().any(|d| {
                matches!(
                    &d.kind,
                    Some(DiagnosticKind::ArityMismatch { name, .. }) if name == entry.name
                )
            });
            if !rejected {
                silent_on_too_many.push(entry.name.to_string());
            }
        }
    }

    assert!(
        silent_on_too_few.is_empty(),
        "FUT-7 regression: {} builtins silently accept too-few args: {:?}",
        silent_on_too_few.len(),
        silent_on_too_few
    );
    assert!(
        silent_on_too_many.is_empty(),
        "FUT-7 regression: {} builtins silently accept too-many args: {:?}",
        silent_on_too_many.len(),
        silent_on_too_many
    );
}

/// Pinned diagnostic shape for under-arity (representative cases).
/// Exercising one bounded-equal (`min==max`) and one bounded-range function
/// guards against regression of the catalog-driven message phrasing.
#[test]
fn under_arity_rejection_pins_diagnostic_shape() {
    let env = MapEnvironment::new();

    // `power(base, exponent)` is bounded 2..=2 — "requires exactly 2 arguments".
    let result = evaluate(&parse("power(2)").unwrap(), &env);
    let diag = result
        .diagnostics
        .iter()
        .find_map(|d| match &d.kind {
            Some(DiagnosticKind::ArityMismatch {
                name,
                min_args,
                max_args,
                got,
            }) => Some((name.clone(), *min_args, *max_args, *got)),
            _ => None,
        })
        .expect("power(2) must emit ArityMismatch");
    assert_eq!(diag, ("power".to_string(), 2, Some(2), 1));

    // `substring(value, start, length?)` is bounded 2..=3 — "requires at least 2".
    let result = evaluate(&parse("substring('x')").unwrap(), &env);
    let diag = result
        .diagnostics
        .iter()
        .find_map(|d| match &d.kind {
            Some(DiagnosticKind::ArityMismatch {
                name,
                min_args,
                max_args,
                got,
            }) => Some((name.clone(), *min_args, *max_args, *got)),
            _ => None,
        })
        .expect("substring('x') must emit ArityMismatch");
    assert_eq!(diag, ("substring".to_string(), 2, Some(3), 1));

    // `coalesce(...values)` is `1..` (variadic required) — "requires at least 1".
    let result = evaluate(&parse("coalesce()").unwrap(), &env);
    let diag = result
        .diagnostics
        .iter()
        .find_map(|d| match &d.kind {
            Some(DiagnosticKind::ArityMismatch {
                name,
                min_args,
                max_args,
                got,
            }) => Some((name.clone(), *min_args, *max_args, *got)),
            _ => None,
        })
        .expect("coalesce() must emit ArityMismatch");
    assert_eq!(diag, ("coalesce".to_string(), 1, None, 0));
}

/// Pinned diagnostic shape for over-arity (previously silently dropped).
/// `round(value, precision?)` is bounded 1..=2; a third arg used to be ignored.
#[test]
fn over_arity_rejection_pins_diagnostic_shape() {
    let env = MapEnvironment::new();

    let result = evaluate(&parse("round(1, 2, 3)").unwrap(), &env);
    let diag = result
        .diagnostics
        .iter()
        .find_map(|d| match &d.kind {
            Some(DiagnosticKind::ArityMismatch {
                name,
                min_args,
                max_args,
                got,
            }) => Some((name.clone(), *min_args, *max_args, *got)),
            _ => None,
        })
        .expect("round(1, 2, 3) must emit ArityMismatch");
    assert_eq!(diag, ("round".to_string(), 1, Some(2), 3));

    // `length(value)` is exact 1..=1 — extra args now rejected (used to be dropped).
    let result = evaluate(&parse("length('a', 'b')").unwrap(), &env);
    let diag = result
        .diagnostics
        .iter()
        .find_map(|d| match &d.kind {
            Some(DiagnosticKind::ArityMismatch {
                name,
                min_args,
                max_args,
                got,
            }) => Some((name.clone(), *min_args, *max_args, *got)),
            _ => None,
        })
        .expect("length('a', 'b') must emit ArityMismatch");
    assert_eq!(diag, ("length".to_string(), 1, Some(1), 2));

    // `today()` is exact 0..=0 — a stray arg now rejected.
    let result = evaluate(&parse("today(1)").unwrap(), &env);
    let diag = result
        .diagnostics
        .iter()
        .find_map(|d| match &d.kind {
            Some(DiagnosticKind::ArityMismatch {
                name,
                min_args,
                max_args,
                got,
            }) => Some((name.clone(), *min_args, *max_args, *got)),
            _ => None,
        })
        .expect("today(1) must emit ArityMismatch");
    assert_eq!(diag, ("today".to_string(), 0, Some(0), 1));
}

/// Variadic catalog declarations remain unbounded above: `coalesce`, `format`,
/// `min`, `max`. Calls with many args must not be rejected.
#[test]
fn variadic_calls_remain_unbounded() {
    let env = MapEnvironment::new();
    for src in [
        "coalesce(1, 2, 3, 4, 5, 6, 7, 8, 9, 10)",
        "format('{0}-{1}-{2}-{3}', 'a', 'b', 'c', 'd')",
        "min(5, 4, 3, 2, 1)",
        "max(1, 2, 3, 4, 5)",
    ] {
        let parsed = parse(src).unwrap_or_else(|e| panic!("parse {src}: {e}"));
        let result = evaluate(&parsed, &env);
        let arity_diag = result
            .diagnostics
            .iter()
            .any(|d| matches!(&d.kind, Some(DiagnosticKind::ArityMismatch { .. })));
        assert!(
            !arity_diag,
            "variadic call `{src}` must not be rejected by the arity gate"
        );
    }
}
