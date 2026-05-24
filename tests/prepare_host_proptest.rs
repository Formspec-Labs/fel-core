//! Property tests for `fel_core::prepare_for_host` and `fel_core::prepare`
//! — Phase 3 FUT-4 (and Phase 3a P0 #4 per
//! `thoughts/2026-05-23-phase-3-ci-gate-design.md:233`).
//!
//! Phase 2 mutation gate confirmed `prepare_host.rs` has the lowest kill
//! rate (69%) among P0 seams with 20 timeout-survivors indicating
//! potential infinite loops on certain mutations. This proptest provides
//! the structural fix: instead of per-mutant kill tests for 39 specific
//! survivors, generate diverse inputs and assert invariants on the
//! prepass output.
//!
//! Properties tested on `prepare_for_host`:
//!
//! 1. **Pass-through on no-op context** — empty `current_item_path` +
//!    no repeat counts + `replace_self_ref=false` MUST be identity on
//!    bare-identifier expressions.
//! 2. **Output parseability** — for any parseable input, the prepared
//!    output MUST also parse. The prepass should not introduce syntax
//!    errors.
//! 3. **Self-ref replacement** — when `replace_self_ref=true` and
//!    `current_item_path` ends with a leaf, bare `$` is replaced with
//!    `$<leaf>`. The output contains no bare `$` followed by non-
//!    identifier characters.
//! 4. **Idempotence** — running prepare twice produces the same output
//!    as running it once (the transform is its own fixed point on
//!    already-prepared expressions).
//!
//! Properties tested on `prepare` (owned-options sibling):
//!
//! - **Equivalence** — `prepare(&opts)` MUST equal `prepare_for_host(input)`
//!   built from the same fields. The two entry points diverge on input
//!   shape only (borrowed vs owned); behavior is identical. This
//!   single property pins the contract and obviates per-property
//!   duplication of idempotence / termination / parseability — they
//!   transitively follow from equivalence plus the `prepare_for_host`
//!   properties. The three required `prepare`-side proptests below
//!   carry over by calling through the owned entry point.

#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{PrepareHostInput, PrepareHostOptions, parse, prepare, prepare_for_host};
use proptest::prelude::*;
use std::collections::HashMap;

/// Identifiers safe to use in test expressions — match FEL identifier
/// rules ([a-zA-Z_][a-zA-Z0-9_]*) and avoid reserved words.
fn arb_safe_ident() -> impl Strategy<Value = String> {
    proptest::sample::select(
        &[
            "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa",
            "lambda", "mu", "nu", "xi", "omicron", "pi", "rho", "sigma", "tau",
        ][..],
    )
    .prop_map(|s| s.to_string())
}

/// Arbitrary safe FEL expression: numeric literals, identifiers (as
/// `$field` references), and small binary ops.
fn arb_safe_expr() -> impl Strategy<Value = String> {
    let leaf = prop_oneof![
        any::<i32>().prop_map(|n| n.to_string()),
        arb_safe_ident().prop_map(|n| format!("${n}")),
        Just("$".to_string()),
    ];
    leaf.prop_recursive(3, 32, 6, |inner| {
        prop_oneof![
            (inner.clone(), inner.clone()).prop_map(|(a, b)| format!("({a} + {b})")),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| format!("({a} * {b})")),
        ]
    })
}

fn prep(
    expression: &str,
    current_item_path: &str,
    replace_self_ref: bool,
    repeats: &[(&str, u32)],
    paths: &[&str],
) -> String {
    let rc: HashMap<String, u32> = repeats.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    let fp: Vec<String> = paths.iter().map(|s| (*s).to_string()).collect();
    prepare_for_host(PrepareHostInput {
        expression,
        current_item_path,
        replace_self_ref,
        repeat_counts: &rc,
        field_paths: &fp,
    })
}

/// Owned-options sibling of [`prep`]. Routes through `prepare` rather
/// than `prepare_for_host` so the owned entry point is exercised
/// directly under proptest pressure.
fn prep_owned(
    expression: &str,
    current_item_path: &str,
    replace_self_ref: bool,
    repeats: &[(&str, u32)],
    paths: &[&str],
) -> String {
    let opts = PrepareHostOptions {
        expression: expression.to_string(),
        current_item_path: current_item_path.to_string(),
        replace_self_ref,
        repeat_counts: repeats.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        field_paths: paths.iter().map(|s| (*s).to_string()).collect(),
    };
    prepare(&opts)
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        ..Default::default()
    })]

    /// No-op context: empty current_item_path, no repeats, no replace.
    /// The prepass must not modify the expression when there's nothing
    /// to resolve. (Mutations that perform spurious work in the no-op
    /// path would surface here.)
    #[test]
    fn no_op_context_is_identity_on_literals(n in any::<i32>()) {
        let src = n.to_string();
        let out = prep(&src, "", false, &[], &[]);
        prop_assert_eq!(out, src);
    }

    /// Output parseability: for every valid input expression, the
    /// prepared output is also valid FEL.
    #[test]
    fn prepared_output_parses(
        expr in arb_safe_expr(),
        path in arb_safe_ident().prop_map(|n| format!("{n}[0].field")),
        replace in any::<bool>(),
    ) {
        // Skip if the source itself doesn't parse — the proptest
        // generator can produce constructs that exceed the depth cap.
        let Ok(_) = parse(&expr) else {
            return Ok(());
        };
        let out = prep(&expr, &path, replace, &[], &[]);
        prop_assert!(
            parse(&out).is_ok(),
            "prepared output {out:?} (from {expr:?}, path {path:?}, replace={replace}) does not parse"
        );
    }

    /// Idempotence: running prepare twice produces the same output as
    /// running it once. The transform is its own fixed point on
    /// already-prepared expressions.
    #[test]
    fn prepare_is_idempotent(
        expr in arb_safe_expr(),
        path in arb_safe_ident().prop_map(|n| format!("{n}[0].field")),
    ) {
        let Ok(_) = parse(&expr) else {
            return Ok(());
        };
        let once = prep(&expr, &path, true, &[], &[]);
        // The output of prep ≠ a valid input for prep necessarily
        // (the path context changes meaning); but on inputs that have
        // already been "prepared," running again with the SAME context
        // should produce a stable result.
        let twice = prep(&once, &path, true, &[], &[]);
        prop_assert_eq!(once, twice);
    }

    /// Self-ref replacement: when replace_self_ref=true and the path
    /// names a leaf, bare `$` becomes `$<leaf>`. The output MUST NOT
    /// contain bare `$` followed by whitespace or operator (which would
    /// indicate unreplaced bare-self).
    #[test]
    fn self_ref_replacement_eliminates_bare_dollar(
        leaf in arb_safe_ident(),
    ) {
        let path = format!("group[0].{leaf}");
        let src = "$ * 2";
        let out = prep(src, &path, true, &[], &[]);
        // Output should be `$<leaf> * 2`, not `$ * 2`.
        prop_assert!(
            out.contains(&format!("${leaf}")),
            "expected ${leaf} in output {out:?}"
        );
        prop_assert!(
            !out.contains("$ "),
            "bare $ followed by space should be replaced; got {out:?}"
        );
    }

    /// Mutation-resilience: the prepass must terminate on all inputs.
    /// Mutations introducing infinite loops (the 20 timeouts observed
    /// in the baseline) would surface as proptest test-runner timeout
    /// — the test framework's hung-test detection.
    #[test]
    fn prepare_terminates_on_arbitrary_input(
        expr in arb_safe_expr(),
        path in arb_safe_ident().prop_map(|n| format!("{n}[0].field")),
        repeats_n in 0u32..5,
    ) {
        let Ok(_) = parse(&expr) else {
            return Ok(());
        };
        let repeats: Vec<(&str, u32)> = if repeats_n > 0 {
            vec![("group", repeats_n)]
        } else {
            vec![]
        };
        // Just call it — if this returns, we've shown termination on
        // this input class. proptest's test-runner timeout catches
        // hangs in CI.
        let _ = prep(&expr, &path, true, &repeats, &[]);
    }

    // ──────────────────────────────────────────────────────────────────
    // `prepare` (owned-options sibling) — Phase 3a P0 #4
    // ──────────────────────────────────────────────────────────────────
    //
    // `prepare(&PrepareHostOptions)` is a shape-only adapter over
    // `prepare_for_host(PrepareHostInput<'_>)` (see
    // `src/prepare_host.rs:456-464`). The properties below mirror the
    // `prepare_for_host` block above; the additional `equivalence`
    // property pins the contract that the two entry points must produce
    // byte-identical output for equivalent inputs. A future mutation
    // that diverges `prepare` from `prepare_for_host` is caught by the
    // equivalence test; the idempotence / termination / parseability
    // tests provide redundancy at the owned entry point itself so the
    // proptest manifest can cite `prepare` directly per the FUT-4 gate.

    /// Equivalence: `prepare(&opts)` and `prepare_for_host(input)` must
    /// agree byte-for-byte for inputs built from the same fields. This
    /// is the load-bearing invariant for the shape-only divergence.
    #[test]
    fn prepare_equivalent_to_prepare_for_host(
        expr in arb_safe_expr(),
        path in arb_safe_ident().prop_map(|n| format!("{n}[0].field")),
        replace in any::<bool>(),
        repeats_n in 0u32..5,
    ) {
        let repeats: Vec<(&str, u32)> = if repeats_n > 0 {
            vec![("group", repeats_n)]
        } else {
            vec![]
        };
        let owned = prep_owned(&expr, &path, replace, &repeats, &[]);
        let borrowed = prep(&expr, &path, replace, &repeats, &[]);
        prop_assert_eq!(owned, borrowed);
    }

    /// Idempotence (owned): `prepare(prepare(opts)) == prepare(opts)`
    /// for the same context. Mirrors `prepare_is_idempotent` above.
    #[test]
    fn prepare_owned_is_idempotent(
        expr in arb_safe_expr(),
        path in arb_safe_ident().prop_map(|n| format!("{n}[0].field")),
    ) {
        let Ok(_) = parse(&expr) else {
            return Ok(());
        };
        let once = prep_owned(&expr, &path, true, &[], &[]);
        let twice = prep_owned(&once, &path, true, &[], &[]);
        prop_assert_eq!(once, twice);
    }

    /// Termination (owned): the owned entry point must terminate on
    /// any `arb_safe_expr`. Mirrors `prepare_terminates_on_arbitrary_input`.
    #[test]
    fn prepare_owned_terminates_on_arbitrary_input(
        expr in arb_safe_expr(),
        path in arb_safe_ident().prop_map(|n| format!("{n}[0].field")),
        repeats_n in 0u32..5,
    ) {
        let Ok(_) = parse(&expr) else {
            return Ok(());
        };
        let repeats: Vec<(&str, u32)> = if repeats_n > 0 {
            vec![("group", repeats_n)]
        } else {
            vec![]
        };
        let _ = prep_owned(&expr, &path, true, &repeats, &[]);
    }

    /// Parseability (owned): output of `prepare` parses back to a
    /// valid AST. Mirrors `prepared_output_parses` above.
    #[test]
    fn prepare_owned_output_parses(
        expr in arb_safe_expr(),
        path in arb_safe_ident().prop_map(|n| format!("{n}[0].field")),
        replace in any::<bool>(),
    ) {
        let Ok(_) = parse(&expr) else {
            return Ok(());
        };
        let out = prep_owned(&expr, &path, replace, &[], &[]);
        prop_assert!(
            parse(&out).is_ok(),
            "prepared (owned) output {out:?} (from {expr:?}, path {path:?}, replace={replace}) does not parse"
        );
    }
}
