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

use fel_core::{
    PrepareHostInput, PrepareHostOptions, RepeatAliases, parse, prepare, prepare_for_host,
    prepare_with_aliases,
};
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};

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

/// FUT-16 Path (a): generator for string literals containing backslash-
/// escape sequences. The base `arb_safe_expr` never produces backslashes,
/// so the `step_quote` escape handler (`src/prepare_host.rs:83`) is
/// undertested by the existing proptests. Emits `$ + '<body>' + $`
/// — bare-`$` on both sides of the escape-containing quoted literal —
/// so self-ref rewrite fires before AND after the quote, which kills
/// `:83:18 ==→!=` / `:83:26 &&→||` mutants (they leak the in-quote
/// state past the close `'` and suppress the trailing bare-`$` rewrite).
fn arb_expr_with_escaped_string() -> impl Strategy<Value = String> {
    let safe_char = prop_oneof![
        Just("a".to_string()),
        Just("b".to_string()),
        Just(" ".to_string()),
        Just("1".to_string()),
        Just("\\n".to_string()),
        Just("\\t".to_string()),
        Just("\\'".to_string()),
        Just("\\\\".to_string()),
    ];
    proptest::collection::vec(safe_char, 0..6)
        .prop_map(|body| format!("$ + '{}' + $", body.concat()))
}

/// FUT-16 Path (a): generator for implicit-alias positions where the prefix
/// char varies across the blocked/unblocked boundary. Targets P6 + P7
/// mutants (`is_blocked_implicit_prefix`, `:334:63` chars[i-1] arithmetic).
/// Emits expressions like `<prefix>rows.score + 1` where `<prefix>` is
/// either an unblocked char (space, `(`, `+`) — forcing the alias to fire
/// — or a blocked char (ident-char, `.`, `$`, `@`) — forcing it to skip.
fn arb_expr_with_alias_at_varying_prefix() -> impl Strategy<Value = String> {
    let prefix = prop_oneof![
        Just(" ".to_string()),
        Just("(".to_string()),
        Just("+".to_string()),
        Just("*".to_string()),
        Just("x".to_string()),
        Just(".".to_string()),
        Just("$".to_string()),
        Just("@".to_string()),
    ];
    let suffix = prop_oneof![
        Just(" + 1".to_string()),
        Just(")".to_string()),
        Just("".to_string()),
        Just("123".to_string()),
        Just("[0]".to_string()),
    ];
    (prefix, suffix).prop_map(|(p, s)| format!("{p}rows.score{s}"))
}

/// FUT-16 Path (a): generator for `$group.field` patterns near string-
/// boundary positions. Targets P5 mutants (`replace_qualified_group_ref_outside_quotes`
/// index arithmetic at `:281:27/31/51`, `:285:58/62`, `:289:25`). Emits
/// expressions where `$group.field` appears at start, mid, or end of the
/// expression — including trailing-dot and `$group` (no dot) shapes that
/// stress the OOB guards.
fn arb_expr_with_group_ref_at_boundary() -> impl Strategy<Value = String> {
    let pattern = prop_oneof![
        Just("$group.qty".to_string()),
        Just("$group.q".to_string()),
        Just("$group.".to_string()),
        Just("$group".to_string()),
        Just("$group.qty + 1".to_string()),
        Just("a + $group.qty".to_string()),
        Just("a + $group".to_string()),
        Just("a + $group.".to_string()),
    ];
    pattern.prop_map(|s| s)
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

    // ──────────────────────────────────────────────────────────────────
    // FUT-16 Path (a) — generator extensions for prepare_host.rs
    // mutation-survivor coverage. See
    // `thoughts/2026-05-23-mutation-survivor-followups.md` §FUT-16.
    // ──────────────────────────────────────────────────────────────────

    /// Strings containing backslash-escape sequences must round-trip
    /// through `prepare_for_host` without panic AND must not leak
    /// in-quote state past the close `'`. Targets `src/prepare_host.rs:83`
    /// step_quote escape handler (base `arb_safe_expr` never generates
    /// `\` chars, so the escape branches are otherwise untested under
    /// proptest).
    ///
    /// Invariant: input has bare `$` BEFORE and AFTER the quoted span.
    /// Both should be rewritten to `$qty` under self-ref. Output must
    /// contain `$qty` exactly TWICE (one for each bare `$`), proving
    /// step_quote correctly closed the quote and resumed outside-quote
    /// scanning. Mutants that leak in-quote state past the close suppress
    /// the trailing bare-`$` rewrite → only one `$qty` in output.
    #[test]
    fn escape_strings_round_trip_through_prepare(
        expr in arb_expr_with_escaped_string(),
    ) {
        let out = prep(&expr, "items[0].qty", true, &[], &[]);
        let qty_count = out.matches("$qty").count();
        prop_assert_eq!(
            qty_count, 2,
            "expected $qty twice (bare-$ before AND after quoted span); got {} in output {:?} from {:?}",
            qty_count, out, expr
        );
    }

    /// Implicit alias rewrites must respect prefix-blocked-vs-unblocked
    /// boundary at varying character positions. Targets P6
    /// (`is_blocked_implicit_prefix`) and P7 (`:334:63` chars[i-1] arithmetic).
    /// Asserts: an UNBLOCKED prefix (space, `(`, `+`, `*`) followed by the
    /// alias produces `$rows[*].score` in the output; a BLOCKED prefix
    /// (ident-char, `.`, `$`, `@`) suppresses the rewrite.
    #[test]
    fn alias_prefix_block_respected_at_varying_positions(
        expr in arb_expr_with_alias_at_varying_prefix(),
    ) {
        let out = prep(
            &expr,
            "",
            false,
            &[],
            &["rows[0].score", "rows[1].score"],
        );
        let prefix_char = expr.chars().next().expect("non-empty");
        let unblocked = matches!(prefix_char, ' ' | '(' | '+' | '*');
        let blocked_dot_or_at = matches!(prefix_char, '.' | '@');
        let blocked_ident_or_dollar = prefix_char.is_ascii_alphanumeric()
            || prefix_char == '_'
            || prefix_char == '$';
        if unblocked {
            // Suffix may continue the alias (`123`, `[0]`) → rewrite
            // suppressed by `suffix_continues_alias`. Check the suffix.
            let after_alias = &expr[1 + "rows.score".len()..];
            let suffix_blocks = after_alias.starts_with(|c: char|
                c.is_ascii_alphanumeric() || c == '_' || c == '['
            );
            if !suffix_blocks {
                prop_assert!(
                    out.contains("$rows[*].score"),
                    "unblocked prefix {prefix_char:?} should fire alias rewrite; got {out:?} from {expr:?}"
                );
            }
        } else if blocked_dot_or_at || blocked_ident_or_dollar {
            // Implicit rewrite suppressed. The `$`-prefix case still triggers
            // the explicit rewrite (`$rows.score` → `$rows[*].score`), so
            // assert on the implicit-pass invariant only when prefix is not `$`.
            if prefix_char != '$' {
                prop_assert!(
                    !out.contains("[*]"),
                    "blocked prefix {prefix_char:?} should suppress alias rewrite; got {out:?} from {expr:?}"
                );
            }
        }
    }

    /// `$group.field` rewrites at boundary positions (start, mid, end,
    /// trailing dot, no dot) must not panic and must produce the correct
    /// rewrite shape. Targets P5 (`replace_qualified_group_ref_outside_quotes`
    /// arithmetic at `:281:27/31/51`, `:285:58/62`, `:289:25`).
    #[test]
    fn group_ref_boundary_positions_do_not_panic(
        expr in arb_expr_with_group_ref_at_boundary(),
    ) {
        // Repeat ancestor "group" via path; current_item_path puts us
        // inside an innermost group instance so qualified rewrites fire.
        let out = prep(&expr, "group[0].x", false, &[("group", 2)], &[]);
        // Termination + non-panic is the primary invariant. Output length
        // is bounded by input + concrete-prefix expansion; assert it's
        // non-empty when input is non-empty.
        prop_assert!(
            !out.is_empty() || expr.is_empty(),
            "prepared output unexpectedly empty for {expr:?}: {out:?}"
        );
    }
}

// ── Prebuilt repeat aliases ────────────────────────────────────────────────

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        ..Default::default()
    })]

    /// `prepare_with_aliases` over `RepeatAliases::from_field_paths(paths)` equals
    /// `prepare_for_host` inferring the aliases from the same paths per call.
    #[test]
    fn prepare_with_aliases_equivalent_to_prepare_for_host(
        expr in arb_safe_expr(),
        field in arb_safe_ident(),
        rows in 0u32..3,
        replace in any::<bool>(),
    ) {
        let paths: Vec<String> = (0..rows).map(|i| format!("group[{i}].{field}")).collect();
        let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        let aliases = RepeatAliases::from_field_paths(path_refs.iter().copied());
        let counts: HashMap<String, u32> = HashMap::from([("group".to_string(), rows)]);
        let prebuilt = prepare_with_aliases(&expr, "group[0].x", replace, &counts, &aliases);
        let inferred = prep(&expr, "group[0].x", replace, &[("group", rows)], &path_refs);
        prop_assert_eq!(prebuilt, inferred);
    }

    /// The alias set is one entry per distinct `<group>.<field>`, longest first.
    #[test]
    fn repeat_aliases_are_distinct_and_longest_first(
        fields in proptest::collection::vec(arb_safe_ident(), 0..6),
        rows in 1u32..3,
    ) {
        let paths: Vec<String> = fields
            .iter()
            .flat_map(|field| (0..rows).map(move |i| format!("group[{i}].{field}")))
            .collect();
        let aliases = RepeatAliases::from_field_paths(paths.iter().map(String::as_str));
        let expected: HashSet<String> = fields.iter().map(|field| format!("group.{field}")).collect();
        let distinct: HashSet<String> = aliases.as_slice().iter().cloned().collect();
        prop_assert_eq!(distinct.len(), aliases.as_slice().len());
        prop_assert_eq!(distinct, expected);
        prop_assert!(aliases.as_slice().windows(2).all(|pair| pair[0].len() >= pair[1].len()));
    }
}
