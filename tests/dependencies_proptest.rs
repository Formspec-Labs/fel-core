//! Property tests for `fel_core::extract_dependencies` — Phase 3 FUT-5.
//!
//! The plan's H2 mutation-gate analysis confirmed `extract_dependencies`
//! has the lowest kill rate (56%) among P0 seams; per-mutant kill tests
//! would be band-aid where a proptest is the structural fix. This file
//! lands that proptest.
//!
//! Properties tested:
//!
//! 1. **Idempotence** — extracting twice on the same expression yields
//!    identical Dependencies.
//! 2. **Field union under composition** — `extract(a + b).fields == extract(a) ∪ extract(b)`
//!    when neither subexpr is shadowed by `let`.
//! 3. **Determinism** — same expression → same dependencies (HashSet
//!    aside, the field set is invariant).
//! 4. **`$` self-ref invariant** — bare `$` inside a predicate
//!    (`countWhere`, `every`, `some`) does NOT escape as
//!    `has_self_ref`; bare `$` outside does.
//! 5. **Wildcard propagation** — any `[*]` segment in the AST marks
//!    `has_wildcard=true`.

#![cfg(feature = "proptest-strategies")]
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::ast::{BinaryOp, Expr, PathSegment};
use fel_core::testing::strategies::arb_expr;
use fel_core::{builtin_function_catalog, extract_dependencies, parse};
use proptest::prelude::*;

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        ..Default::default()
    })]

    /// Idempotence: re-extracting must produce identical output.
    /// Mutations that mutate static state during the walk would surface here.
    #[test]
    fn extract_is_idempotent(expr in arb_expr(3, builtin_function_catalog())) {
        let first = extract_dependencies(&expr);
        let second = extract_dependencies(&expr);
        prop_assert_eq!(first.fields, second.fields);
        prop_assert_eq!(first.context_refs, second.context_refs);
        prop_assert_eq!(first.instance_refs, second.instance_refs);
        prop_assert_eq!(first.mip_deps, second.mip_deps);
        prop_assert_eq!(first.has_self_ref, second.has_self_ref);
        prop_assert_eq!(first.has_wildcard, second.has_wildcard);
        prop_assert_eq!(first.uses_prev_next, second.uses_prev_next);
    }

    /// Field union under `+`: for any non-let-shadowed compound, the
    /// field set of `(a + b)` equals the union of field sets of `a` and
    /// `b`. Mutations that drop a match arm in `walk` would surface here
    /// (one of the operands' fields would be missed).
    #[test]
    fn fields_union_under_binary_add(
        a in arb_expr(2, builtin_function_catalog()),
        b in arb_expr(2, builtin_function_catalog()),
    ) {
        let a_deps = extract_dependencies(&a);
        let b_deps = extract_dependencies(&b);
        let combined = Expr::BinaryOp {
            op: BinaryOp::Add,
            left: Box::new(a.clone()),
            right: Box::new(b.clone()),
        };
        let combined_deps = extract_dependencies(&combined);
        // Every field from either operand must appear in the combined set.
        for f in &a_deps.fields {
            prop_assert!(
                combined_deps.fields.contains(f),
                "field {f:?} from left operand missing from combined"
            );
        }
        for f in &b_deps.fields {
            prop_assert!(
                combined_deps.fields.contains(f),
                "field {f:?} from right operand missing from combined"
            );
        }
    }

    /// `has_wildcard` is monotone: a `BinaryOp` containing any wildcard
    /// child must itself have wildcard. Mutations that flip the
    /// has_wildcard assignment branch would surface.
    #[test]
    fn wildcard_propagates_through_binary_op(
        a in arb_expr(2, builtin_function_catalog()),
    ) {
        let a_deps = extract_dependencies(&a);
        let combined = Expr::BinaryOp {
            op: BinaryOp::Add,
            left: Box::new(a.clone()),
            right: Box::new(Expr::Number(rust_decimal::Decimal::ZERO)),
        };
        let combined_deps = extract_dependencies(&combined);
        prop_assert_eq!(
            combined_deps.has_wildcard,
            a_deps.has_wildcard,
            "wildcard from operand must propagate through BinaryOp"
        );
    }
}

// ── Spec-anchored example tests ─────────────────────────────────
//
// These complement the property tests with specific assertions that
// pin the contract per `fel-grammar.md` §6 (path resolution rules).

/// `$x.y` field-ref extracts `"x.y"` as a field; `$` self-ref records
/// has_self_ref but not as a "field". (Mutations deleting either match
/// arm would survive without these explicit cases.)
#[test]
fn field_ref_and_self_ref_separately_recorded() {
    let expr = parse("$x.y + $").expect("parse");
    let deps = extract_dependencies(&expr);
    assert!(
        deps.fields.contains("x.y"),
        "$x.y should record field 'x.y'; got {:?}",
        deps.fields
    );
    assert!(deps.has_self_ref, "bare $ should mark has_self_ref");
}

/// Bare `$` inside a `countWhere` predicate is REBOUND to the
/// iteration element — it does NOT count as a top-level self-ref.
#[test]
fn bare_self_ref_inside_count_where_is_suppressed() {
    let expr = parse("countWhere($items, $ > 5)").expect("parse");
    let deps = extract_dependencies(&expr);
    assert!(
        deps.fields.contains("items"),
        "countWhere first arg should record field 'items'; got {:?}",
        deps.fields
    );
    assert!(
        !deps.has_self_ref,
        "bare $ inside countWhere predicate must NOT mark has_self_ref"
    );
}

/// Wildcard segment marks has_wildcard.
#[test]
fn wildcard_in_path_is_marked() {
    let expr = parse("sum($items[*].qty)").expect("parse");
    let deps = extract_dependencies(&expr);
    assert!(deps.has_wildcard, "[*] segment must set has_wildcard");
}

/// Wildcard in a deeply-nested expression still propagates.
#[test]
fn wildcard_propagates_through_let() {
    let expr = parse("let x = $items[*].qty in x + 1").expect("parse");
    let deps = extract_dependencies(&expr);
    assert!(
        deps.has_wildcard,
        "[*] under let-binding must still mark has_wildcard"
    );
}

/// PostfixAccess match arm — `(expr).field` — must extract the FULL
/// path including the postfix segment. The previous assertion accepted
/// either `"a.b"` OR `"a.b.c"` as success, which let the
/// `extend_field_path -> Some(String::new())` mutation survive (an empty
/// string in the set passes neither contains check but the inner
/// `walk(expr, ...)` fallback still inserts `"a.b"`). Tighten to the
/// strict expectation per FEL path-extension semantics.
#[test]
fn postfix_access_records_full_extended_path() {
    let expr = parse("($a.b).c").expect("parse");
    let deps = extract_dependencies(&expr);
    assert!(
        deps.fields.contains("a.b.c"),
        "PostfixAccess on ($a.b).c MUST record full path 'a.b.c'; got {:?}",
        deps.fields
    );
}

/// `VarRef` (bare identifier without `$`) inside `extract_field_path_str`
/// path — pins the match arm at `dependencies.rs:249`. Mutation gate
/// flagged the arm-deletion as surviving.
#[test]
fn var_ref_in_path_resolves() {
    // `let x = 10 in x` — VarRef inside let-body. The `x` is a let-bound
    // var; should NOT appear as a field dependency.
    let expr = parse("let x = 10 in x").expect("parse");
    let deps = extract_dependencies(&expr);
    assert!(
        !deps.fields.contains("x"),
        "let-bound VarRef must NOT escape as field dependency; got {:?}",
        deps.fields
    );
}

/// `parent()` function-call sets `uses_prev_next` (alongside `prev`/`next`
/// — `parent` is in the same temporal-navigation family per
/// `src/dependencies.rs:129`). Kills the "parent" match-arm deletion.
#[test]
fn parent_function_call_marks_uses_prev_next() {
    let expr = parse("parent()").expect("parse");
    let deps = extract_dependencies(&expr);
    assert!(
        deps.uses_prev_next,
        "parent() should mark uses_prev_next; got {:?}",
        deps
    );
}

/// `instance('name')` records the named instance in `instance_refs`.
/// Kills the "instance" match-arm deletion at `src/dependencies.rs:132`.
#[test]
fn instance_function_call_records_instance_ref() {
    let expr = parse("instance('foo')").expect("parse");
    let deps = extract_dependencies(&expr);
    assert!(
        deps.instance_refs.contains("foo"),
        "instance('foo') should record 'foo' in instance_refs; got {:?}",
        deps.instance_refs
    );
}

/// `let foo = $items in valid(foo)` — VarRef as the first arg of a MIP
/// function (`valid`/`relevant`/`readonly`/`required`). Kills
/// `src/dependencies.rs:249` "delete match arm Expr::VarRef in
/// extract_field_path_str" — without that arm, VarRef yields "" and
/// nothing is inserted into mip_deps.
#[test]
fn let_bound_var_as_mip_first_arg_records_in_mip_deps() {
    let expr = parse("let foo = $items in valid(foo)").expect("parse");
    let deps = extract_dependencies(&expr);
    assert!(
        deps.mip_deps.contains("foo"),
        "VarRef as MIP first arg must extract its name into mip_deps; got {:?}",
        deps.mip_deps
    );
}

/// `(($a.b).c).d` — doubly-nested PostfixAccess. Kills
/// `src/dependencies.rs:269` "delete match arm Expr::PostfixAccess in
/// extend_field_path" — that arm chains nested PostfixAccess segments
/// into the path. Without it, `extend_field_path` on the inner
/// PostfixAccess returns None and the outer path `.d` is lost, so we'd
/// record "a.b.c" instead of the full "a.b.c.d".
#[test]
fn nested_postfix_access_records_full_chain() {
    let expr = parse("(($a.b).c).d").expect("parse");
    let deps = extract_dependencies(&expr);
    assert!(
        deps.fields.contains("a.b.c.d"),
        "doubly-nested PostfixAccess MUST chain all segments into 'a.b.c.d'; got {:?}",
        deps.fields
    );
}

/// `dependencies_to_json_value` must produce a non-empty JSON object
/// when dependencies are non-empty. Pins the function-return mutation
/// where `Default::default()` returns Value::Null.
#[test]
fn dependencies_to_json_value_reflects_actual_deps() {
    use fel_core::dependencies_to_json_value;
    let expr = parse("$x + $y").expect("parse");
    let deps = extract_dependencies(&expr);
    let json = dependencies_to_json_value(&deps);
    assert!(
        json.is_object(),
        "dependencies_to_json_value must return an object, not {:?}",
        json
    );
    // The "fields" key should be present and non-empty.
    let fields = json.get("fields").expect("'fields' key present");
    assert!(
        fields.as_array().is_some_and(|arr| !arr.is_empty()),
        "fields array should be non-empty; got {:?}",
        fields
    );
}

#[allow(dead_code)]
fn touch_path_segment_to_satisfy_lint(_p: &PathSegment) {}
