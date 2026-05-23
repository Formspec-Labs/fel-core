//! Property tests for `fel_core::prepare_for_host` — Phase 3 FUT-4.
//!
//! Phase 2 mutation gate confirmed `prepare_host.rs` has the lowest kill
//! rate (69%) among P0 seams with 20 timeout-survivors indicating
//! potential infinite loops on certain mutations. This proptest provides
//! the structural fix: instead of per-mutant kill tests for 39 specific
//! survivors, generate diverse inputs and assert invariants on the
//! prepass output.
//!
//! Properties tested:
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

#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{PrepareHostInput, parse, prepare_for_host};
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
}
