//! Phase 3b Tier-2 — sibling-equivalence proptest for the public evaluator
//! entry points: `evaluate`, `evaluate_with`, `evaluate_with_catalog`,
//! `eval_with_fields`, plus the `EmptyCatalog` no-op `ContextBindingCatalog`
//! and the `EvaluatorOptions` configuration shape.
//!
//! Six public symbols pinned:
//!
//! 1. `evaluate_with`               — config'd entry point (parsed `Expr` + `Environment` + `EvaluatorOptions`).
//! 2. `evaluate_with_catalog`       — catalog-aware sibling that adds a `ContextBindingCatalog`.
//! 3. `eval_with_fields`            — string-input convenience that parses + wraps `MapEnvironment`.
//! 4. `Evaluator`                   — the tree-walker struct (instantiated indirectly through the entry points; see Property 4 note).
//! 5. `EvaluatorOptions`            — the `Default`-having options shape that `evaluate` and `eval_with_fields` use under the hood.
//! 6. `EmptyCatalog`                — the unit `ContextBindingCatalog` (rejects every `@name`).
//!
//! ## Properties
//!
//! 1. **Sibling equivalence on the common subset.** For any `arb_expr`
//!    against a no-op environment, the three entry points
//!    (`evaluate`, `evaluate_with(default)`, `evaluate_with_catalog(default, &EmptyCatalog)`)
//!    produce byte-equal values AND the same diagnostic *bag* (compared
//!    field-by-field, order-preserved — the evaluator pushes diagnostics
//!    in deterministic walk order and the three siblings share that
//!    walk).
//!
//! 2. **`eval_with_fields` string-shape equivalence.** For any field-only
//!    environment, `eval_with_fields(src, fields)` equals
//!    `evaluate_with(parse(src), &MapEnvironment::with_fields(fields), EvaluatorOptions::default())`.
//!    The convenience function adds nothing beyond parse + env-wrap; the
//!    proptest pins that.
//!
//! 3. **`EmptyCatalog` rejects every `@name` context ref.** For any
//!    identifier `name`, `evaluate_with_catalog` against an
//!    `Expr::ContextRef { name, … }` produces a diagnostic with
//!    `UNBOUND_CONTEXT_REF_CODE`. Conversely, on a context-ref-free
//!    expression the catalog branch never fires and the result matches
//!    `evaluate_with(default)` byte-for-byte. **Pushback against the
//!    brief:** the brief specified "produces an UndefinedFunction
//!    diagnostic for any function call in expr" — but `EmptyCatalog` is
//!    a `ContextBindingCatalog`, *not* a function catalog. Built-in
//!    functions dispatch through the static builtin name list inside
//!    `eval_function` regardless of whether a context-binding catalog is
//!    supplied. `EmptyCatalog`'s actual rejection surface is `@name`
//!    context references (FEL-UNBOUND-CONTEXT). The property is
//!    rewritten to test the real surface.
//!
//! 4. **Three-way agreement.** The entry-point trio agrees not only
//!    pairwise but transitively. This is folded into Property 1 (a single
//!    triple-equality assertion); see also the **pushback** below for
//!    why a standalone "directly-construct Evaluator" property is not
//!    implementable from outside the crate.
//!
//! ### Property 4 pushback
//!
//! The brief asked for "instantiate an `Evaluator` from
//! `EvaluatorOptions::default()` + builtin catalog, call its eval method on
//! arb_expr, assert byte-equal to `evaluate_with_catalog(...)`." That isn't
//! possible from an integration test: `Evaluator` is declared `pub`, but
//! *every* field is `pub(super)` and there is no public constructor, no
//! public `eval` method, and no `Default` impl. From outside the crate the
//! only way to "instantiate" an `Evaluator` is through `evaluate*`. Property
//! 1's triple-equality check is the correct fence — it pins that the three
//! public entry points instantiate the underlying `Evaluator` the same way.
//! Filed as the explicit pushback in the commit body.

#![cfg(feature = "proptest-strategies")]
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::ast::{Expr, PathSegment};
use fel_core::error::{Diagnostic, DiagnosticKind, Severity};
use fel_core::testing::strategies::arb_expr;
use fel_core::types::Value;
use fel_core::{
    EmptyCatalog, EvalResult, EvaluatorOptions, MapEnvironment, UNBOUND_CONTEXT_REF_CODE,
    builtin_function_catalog, eval_with_fields, evaluate, evaluate_with, evaluate_with_catalog,
    parse, print_expr,
};
use proptest::prelude::*;
use std::collections::HashMap;
use std::ops::Range;

// ── Diagnostic equivalence ────────────────────────────────────────
//
// `Diagnostic` does not implement `PartialEq` (it carries a heterogeneous
// payload: severity, free-form message, optional code, structured kind,
// optional span). For sibling-equivalence we compare *all* observable
// fields. Any divergence between two sibling entry points on any field
// would be a real semantic difference, not noise.

#[derive(Debug, PartialEq, Eq)]
struct DiagSnapshot<'a> {
    severity: Severity,
    message: &'a str,
    code: Option<&'a str>,
    kind: Option<&'a DiagnosticKind>,
    span: Option<&'a Range<usize>>,
}

fn snapshot(d: &Diagnostic) -> DiagSnapshot<'_> {
    DiagSnapshot {
        severity: d.severity,
        message: d.message.as_str(),
        code: d.code.as_deref(),
        kind: d.kind.as_ref(),
        span: d.span.as_ref(),
    }
}

fn snapshots(diags: &[Diagnostic]) -> Vec<DiagSnapshot<'_>> {
    diags.iter().map(snapshot).collect()
}

/// True when `expr` contains any `Expr::ContextRef` node anywhere in its
/// tree. `arb_expr` does not currently emit `ContextRef` so this is a
/// guard against future strategy additions silently invalidating
/// Property 1 / Property 3's context-free precondition.
fn has_context_ref(expr: &Expr) -> bool {
    match expr {
        Expr::Null
        | Expr::Boolean(_)
        | Expr::Number(_)
        | Expr::String(_)
        | Expr::DateLiteral(_)
        | Expr::DateTimeLiteral(_)
        | Expr::FieldRef { .. }
        | Expr::VarRef { .. } => false,
        Expr::ContextRef { .. } => true,
        Expr::Array(elems) => elems.iter().any(has_context_ref),
        Expr::Object(entries) => entries.iter().any(|(_, v)| has_context_ref(v)),
        Expr::UnaryOp { operand, .. } => has_context_ref(operand),
        Expr::BinaryOp { left, right, .. } => has_context_ref(left) || has_context_ref(right),
        Expr::Ternary {
            condition,
            then_branch,
            else_branch,
        }
        | Expr::IfThenElse {
            condition,
            then_branch,
            else_branch,
        } => {
            has_context_ref(condition)
                || has_context_ref(then_branch)
                || has_context_ref(else_branch)
        }
        Expr::Membership {
            value, container, ..
        } => has_context_ref(value) || has_context_ref(container),
        Expr::NullCoalesce { left, right } => has_context_ref(left) || has_context_ref(right),
        Expr::LetBinding { value, body, .. } => has_context_ref(value) || has_context_ref(body),
        Expr::FunctionCall { args, .. } => args.iter().any(has_context_ref),
        Expr::PostfixAccess { expr, .. } => has_context_ref(expr),
    }
}

/// Asserts two `EvalResult`s agree on value AND on the full diagnostic
/// bag (ordered, field-by-field). Returns the violated invariant as a
/// `proptest` failure when divergent.
fn assert_results_equal(a: &EvalResult, b: &EvalResult, label: &str) -> Result<(), TestCaseError> {
    prop_assert_eq!(&a.value, &b.value, "{} value divergence", label);
    prop_assert_eq!(
        snapshots(&a.diagnostics),
        snapshots(&b.diagnostics),
        "{} diagnostic divergence",
        label
    );
    Ok(())
}

// ── Properties ────────────────────────────────────────────────────

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        ..proptest::test_runner::Config::default()
    })]

    /// **Property 1 — sibling equivalence on the common subset.**
    ///
    /// `evaluate`, `evaluate_with(default)`, and
    /// `evaluate_with_catalog(default, &EmptyCatalog)` MUST agree
    /// byte-for-byte (value + diagnostics) over `arb_expr`. The catalog
    /// branch is gated on `context_bindings.is_some()` so on
    /// context-ref-free expressions (which `arb_expr` always produces)
    /// the catalog-aware entry point reduces to the non-catalog path.
    ///
    /// This is the structural fence the brief calls "three entry points
    /// instantiate the underlying `Evaluator` the same way" — see
    /// Property 4 pushback at the top of this file.
    #[test]
    fn evaluate_siblings_agree_on_common_subset(
        expr in arb_expr(3, builtin_function_catalog())
    ) {
        // Guard against future `arb_expr` revisions silently emitting
        // ContextRef nodes — they would break the EmptyCatalog branch
        // and require dropping the catalog leg of this property.
        prop_assume!(!has_context_ref(&expr));

        let env = MapEnvironment::new();

        let a = evaluate(&expr, &env);
        let b = evaluate_with(&expr, &env, EvaluatorOptions::default());
        let c = evaluate_with_catalog(
            &expr,
            &env,
            EvaluatorOptions::default(),
            &EmptyCatalog,
        );

        assert_results_equal(&a, &b, "evaluate vs evaluate_with(default)")?;
        assert_results_equal(&b, &c, "evaluate_with(default) vs evaluate_with_catalog(EmptyCatalog)")?;
        // Transitivity (third leg of the triangle — explicit so a future
        // diagnostic-equality relaxation in `assert_results_equal` would
        // surface here too).
        assert_results_equal(&a, &c, "evaluate vs evaluate_with_catalog(EmptyCatalog)")?;
    }

    /// **Property 2 — `eval_with_fields` string-shape equivalence.**
    ///
    /// For any `arb_expr` printed back to FEL source via `print_expr`,
    /// `eval_with_fields(src, fields)` MUST equal
    /// `evaluate_with(parse(src), &MapEnvironment::with_fields(fields), default)`.
    /// The convenience function's contract is parse-then-evaluate-on-MapEnv;
    /// any drift would mean it silently picks up extra config.
    ///
    /// `print_expr` round-trip can fail to re-parse for pathological
    /// generated trees (operator precedence the printer doesn't
    /// re-parenthesize, identifier shadowing, etc.). Such inputs are
    /// out of scope here: this property tests the *function-shape
    /// equivalence*, not the printer round-trip. `prop_assume!` skips
    /// non-reparseable trees.
    #[test]
    fn eval_with_fields_matches_evaluate_with_default(
        expr in arb_expr(3, builtin_function_catalog()),
        seeds in prop::collection::vec(
            ("[a-z]{1,4}", any::<i32>().prop_map(|n| Value::Number(n.into()))),
            0..4,
        ),
    ) {
        prop_assume!(!has_context_ref(&expr));

        let src = print_expr(&expr);

        // Skip non-reparseable printer outputs — out of scope for this
        // property. (See doc comment above.)
        let Ok(parsed) = parse(&src) else {
            return Ok(());
        };

        let fields: HashMap<String, Value> = seeds.into_iter().collect();

        // Path A: convenience function (string input → result).
        let via_convenience = eval_with_fields(&src, fields.clone())
            .expect("parse succeeded above, so eval_with_fields parse must succeed too");

        // Path B: hand-rolled (parse → MapEnvironment::with_fields →
        // evaluate_with(default)). Should be byte-identical to A.
        let env = MapEnvironment::with_fields(fields);
        let via_lowlevel = evaluate_with(&parsed, &env, EvaluatorOptions::default());

        assert_results_equal(&via_convenience, &via_lowlevel, "eval_with_fields vs evaluate_with(default)")?;
    }

    /// **Property 3 — `EmptyCatalog` rejects every `@name` context ref.**
    ///
    /// For any FEL identifier `name`, an `Expr::ContextRef { name,
    /// called: false, arg: None, tail: vec![] }` evaluated through
    /// `evaluate_with_catalog(..., &EmptyCatalog)` MUST produce:
    ///   - `value == Value::Null`, AND
    ///   - exactly one diagnostic with
    ///     `code == Some(UNBOUND_CONTEXT_REF_CODE)`.
    ///
    /// Grammar-reserved names (`current` / `index` / `count` /
    /// `instance`) bypass the catalog branch entirely (per
    /// `is_grammar_reserved_context` in `core.rs`), so this property
    /// excludes them via `prop_assume!` — they fall through to
    /// `env.resolve_context` and return null without diagnostics.
    ///
    /// **Pushback against the brief**: the brief described
    /// `EmptyCatalog` as rejecting "any function call". `EmptyCatalog`
    /// is the unit `ContextBindingCatalog`, not a function catalog;
    /// `eval_function` dispatches through the static builtin name list
    /// + optional `ExtensionRegistry`, independent of the
    /// context-binding catalog. The real rejection surface — and the
    /// one this property pins — is `@name` context references.
    #[test]
    fn empty_catalog_rejects_unbound_context_ref(
        name in "[a-z][a-z0-9_]{0,6}",
    ) {
        // Grammar-reserved context names route to the environment, not
        // the catalog; their behavior is tested by the existing
        // evaluator_tests, not here.
        prop_assume!(!matches!(name.as_str(), "current" | "index" | "count" | "instance"));

        let expr = Expr::ContextRef {
            name: name.clone(),
            called: false,
            arg: None,
            tail: vec![],
        };
        let env = MapEnvironment::new();

        let result = evaluate_with_catalog(
            &expr,
            &env,
            EvaluatorOptions::default(),
            &EmptyCatalog,
        );

        prop_assert_eq!(&result.value, &Value::Null);
        let unbound: Vec<_> = result.diagnostics.iter()
            .filter(|d| d.code.as_deref() == Some(UNBOUND_CONTEXT_REF_CODE))
            .collect();
        prop_assert_eq!(
            unbound.len(),
            1,
            "expected exactly one UNBOUND_CONTEXT_REF diagnostic for @{}, got {:?}",
            name,
            result.diagnostics,
        );
    }

    /// **Property 3 (companion) — `EmptyCatalog` is transparent on
    /// context-ref-free expressions.**
    ///
    /// The catalog branch only fires for `Expr::ContextRef` nodes (and
    /// `Expr::PostfixAccess` over them). On any other expression,
    /// `evaluate_with_catalog(..., &EmptyCatalog)` MUST match
    /// `evaluate_with(default)` byte-for-byte. This complements the
    /// rejection-side property above and makes Property 3 a complete
    /// description of `EmptyCatalog` semantics.
    #[test]
    fn empty_catalog_transparent_on_context_free_exprs(
        expr in arb_expr(3, builtin_function_catalog())
    ) {
        prop_assume!(!has_context_ref(&expr));

        let env = MapEnvironment::new();
        let without_catalog = evaluate_with(&expr, &env, EvaluatorOptions::default());
        let with_empty_catalog = evaluate_with_catalog(
            &expr,
            &env,
            EvaluatorOptions::default(),
            &EmptyCatalog,
        );

        assert_results_equal(
            &without_catalog,
            &with_empty_catalog,
            "evaluate_with(default) vs evaluate_with_catalog(EmptyCatalog) on context-free",
        )?;
    }

    /// **Property 4 (folded into 1) — three-way transitive agreement.**
    ///
    /// Explicit transitive triple-equality reassertion as a separate
    /// test case so a future regression in just one sibling (e.g.
    /// `eval_with_fields` adding a non-default budget) doesn't escape
    /// behind a passing pairwise check.
    ///
    /// Routes the `arb_expr` through `print_expr` → `parse` to also
    /// exercise the string-input sibling (`eval_with_fields`) as the
    /// fourth leg. Same printer-round-trip caveat as Property 2.
    #[test]
    fn all_four_siblings_agree_via_print_round_trip(
        expr in arb_expr(3, builtin_function_catalog())
    ) {
        prop_assume!(!has_context_ref(&expr));

        let src = print_expr(&expr);
        let Ok(parsed) = parse(&src) else {
            return Ok(());
        };

        let env = MapEnvironment::new();

        let r_eval = evaluate(&parsed, &env);
        let r_eval_with = evaluate_with(&parsed, &env, EvaluatorOptions::default());
        let r_eval_with_cat = evaluate_with_catalog(
            &parsed,
            &env,
            EvaluatorOptions::default(),
            &EmptyCatalog,
        );
        let r_eval_with_fields = eval_with_fields(&src, HashMap::new())
            .expect("parse succeeded; eval_with_fields parse must too");

        assert_results_equal(&r_eval, &r_eval_with, "evaluate vs evaluate_with")?;
        assert_results_equal(&r_eval_with, &r_eval_with_cat, "evaluate_with vs evaluate_with_catalog")?;
        assert_results_equal(&r_eval_with_cat, &r_eval_with_fields, "evaluate_with_catalog vs eval_with_fields")?;
    }
}

// ── Example-style pins ────────────────────────────────────────────
//
// Concrete kills for the property cases above. Test the same invariants
// the proptests test, but on fixed inputs the proptests are unlikely to
// hit (named context refs, postfix on context ref, dotted-tail context
// ref). Without these, a regression that only fires on hand-written
// context-ref expressions would escape until a downstream caller hit it.

#[test]
fn empty_catalog_rejects_dotted_context_ref() {
    let expr = Expr::ContextRef {
        name: "user".to_string(),
        called: false,
        arg: None,
        tail: vec!["email".to_string()],
    };
    let env = MapEnvironment::new();
    let result = evaluate_with_catalog(&expr, &env, EvaluatorOptions::default(), &EmptyCatalog);
    assert_eq!(result.value, Value::Null);
    assert_eq!(
        result.diagnostics.len(),
        1,
        "expected exactly one diagnostic"
    );
    assert_eq!(
        result.diagnostics[0].code.as_deref(),
        Some(UNBOUND_CONTEXT_REF_CODE)
    );
}

#[test]
fn empty_catalog_rejects_postfix_on_context_ref() {
    // `@user.profile.name` — exercises the `PostfixAccess { expr:
    // ContextRef, .. }` branch in `eval_impl` (the "merged path"
    // dispatch into `eval_context_ref_path`).
    let expr = Expr::PostfixAccess {
        expr: Box::new(Expr::ContextRef {
            name: "user".to_string(),
            called: false,
            arg: None,
            tail: vec!["profile".to_string()],
        }),
        path: vec![PathSegment::Dot("name".to_string())],
    };
    let env = MapEnvironment::new();
    let result = evaluate_with_catalog(&expr, &env, EvaluatorOptions::default(), &EmptyCatalog);
    assert_eq!(result.value, Value::Null);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some(UNBOUND_CONTEXT_REF_CODE)),
        "expected an UNBOUND_CONTEXT_REF diagnostic, got {:?}",
        result.diagnostics
    );
}

#[test]
fn empty_catalog_does_not_intercept_builtin_function_calls() {
    // Pushback-driven kill: confirms the brief's misframing —
    // `EmptyCatalog` does NOT reject builtin function calls. `sum` is
    // a builtin; it must evaluate normally regardless of which
    // `ContextBindingCatalog` is installed.
    let expr = parse("sum([1, 2, 3])").expect("parse");
    let env = MapEnvironment::new();
    let with_catalog =
        evaluate_with_catalog(&expr, &env, EvaluatorOptions::default(), &EmptyCatalog);
    assert_eq!(
        with_catalog.value,
        Value::Number(6.into()),
        "builtins must dispatch independently of the context-binding catalog"
    );
    assert!(
        with_catalog.diagnostics.is_empty(),
        "no diagnostics expected, got {:?}",
        with_catalog.diagnostics
    );
}

#[test]
fn evaluator_options_default_is_unlimited_budget_no_trace_no_extensions() {
    // Pin the `EvaluatorOptions::default()` shape. If a future commit
    // adds a tight default budget, this test fails immediately and
    // forces a review — Property 1's "byte-equal across siblings"
    // would still pass but the *meaning* would change silently.
    let opts = EvaluatorOptions::default();
    // Steps default is `u64::MAX` via `EvalBudget::unlimited()`.
    assert_eq!(opts.budget.max_steps, u64::MAX);
    assert_eq!(opts.budget.max_alloc_bytes, u64::MAX);
    assert!(opts.budget.deadline.is_none());
    assert!(opts.extensions.is_none());
    assert!(opts.trace.is_none());
}
