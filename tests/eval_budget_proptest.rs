//! Phase 3 P0 proptest #1 — `EvalBudget` + `BudgetExceededKind`. See design at thoughts/2026-05-23-phase-3-ci-gate-design.md.
//!
//! Three properties:
//!
//! 1. **Monotonicity.** Once any usage tuple exceeds a budget for a given
//!    metric, increasing that metric's usage cannot transition back to
//!    `Ok`. Rechecking with more consumption must never succeed if a
//!    smaller check already failed. Pinned per-metric (steps, alloc) so
//!    the failure mode names the metric.
//! 2. **`BudgetExceededKind` discrimination.** Each metric — steps,
//!    alloc, deadline — overflows to a *distinct* `BudgetExceededKind`
//!    variant. Uses an exhaustive match on the kind enum (no
//!    wildcards), so adding a new variant without wiring its overflow
//!    path breaks compilation here.
//! 3. **Sane defaults admit trivial work.** `EvalBudget::unlimited()` —
//!    the constructor delegated to by the public `evaluate` entry
//!    points (see `budget.rs:38`) — accepts the usage tuple consistent
//!    with evaluating a single-literal expression. A smoke property
//!    that pins the "default-grade" budget's relevance to actual
//!    evaluator usage. (Pushback against the brief: there is no
//!    `EvalBudget::default()` impl in `budget.rs`; `unlimited()` is
//!    the de-facto default — what `evaluate` uses when no budget is
//!    configured. See commit body.)

#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{BudgetExceededKind, EvalBudget, MapEnvironment, Value, evaluate, parse};
use proptest::prelude::*;
use std::time::{Duration, Instant};

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        ..proptest::test_runner::Config::default()
    })]

    /// Property 1a — monotonicity over the `steps` metric.
    ///
    /// For any `max`, if `check(s1, 0) == Err`, then for every `s2 >= s1`,
    /// `check(s2, 0)` must also be `Err`. The strict-`>` boundary at
    /// `budget.rs:77` makes the threshold `s > max`. Once crossed, no
    /// larger `s` can fall back inside.
    #[test]
    fn budget_check_monotone_under_increasing_steps(
        max in 0u64..1_000_000,
        s1 in 0u64..2_000_000,
        delta in 0u64..1_000_000,
    ) {
        let budget = EvalBudget {
            max_steps: max,
            max_alloc_bytes: u64::MAX,
            deadline: None,
        };
        let r1 = budget.check(s1, 0);
        if r1.is_err() {
            let s2 = s1.saturating_add(delta);
            let r2 = budget.check(s2, 0);
            prop_assert!(
                r2.is_err(),
                "monotonicity violated: check({s1}, 0) = {r1:?} but check({s2}, 0) = {r2:?} with max_steps={max}",
            );
        }
    }

    /// Property 1b — monotonicity over the `alloc_bytes` metric.
    ///
    /// Same shape as the steps property, but on the second tuple
    /// position and against `max_alloc_bytes` (boundary at
    /// `budget.rs:80`). Holds steps at 0 so any `Err` is necessarily
    /// `Err(Alloc)` — keeps the property scoped to one metric.
    #[test]
    fn budget_check_monotone_under_increasing_alloc(
        max in 0u64..1_000_000,
        a1 in 0u64..2_000_000,
        delta in 0u64..1_000_000,
    ) {
        let budget = EvalBudget {
            max_steps: u64::MAX,
            max_alloc_bytes: max,
            deadline: None,
        };
        let r1 = budget.check(0, a1);
        if r1.is_err() {
            let a2 = a1.saturating_add(delta);
            let r2 = budget.check(0, a2);
            prop_assert!(
                r2.is_err(),
                "monotonicity violated: check(0, {a1}) = {r1:?} but check(0, {a2}) = {r2:?} with max_alloc_bytes={max}",
            );
        }
    }

    /// Property 2 — distinct kind per metric.
    ///
    /// Construct three budgets, each tight on exactly one metric. The
    /// overflow on each yields a `BudgetExceededKind` matched
    /// exhaustively (no wildcard). A future variant added to
    /// `BudgetExceededKind` without overflow wiring will fail to
    /// compile this test — the structural guard the brief requires.
    ///
    /// The deadline budget uses `Instant::now() - 1s` (saturating) so
    /// the deadline has already passed at construction.
    #[test]
    fn budget_exceeded_kind_is_distinct_per_metric(
        steps_over in 1u64..1_000_000,
        alloc_over in 1u64..1_000_000,
    ) {
        let steps_budget = EvalBudget {
            max_steps: 0,
            max_alloc_bytes: u64::MAX,
            deadline: None,
        };
        let alloc_budget = EvalBudget {
            max_steps: u64::MAX,
            max_alloc_bytes: 0,
            deadline: None,
        };
        let deadline_budget = EvalBudget {
            max_steps: u64::MAX,
            max_alloc_bytes: u64::MAX,
            deadline: Some(
                Instant::now()
                    .checked_sub(Duration::from_secs(1))
                    .unwrap_or_else(Instant::now),
            ),
        };

        let steps_kind = steps_budget
            .check(steps_over, 0)
            .expect_err("steps_over > max_steps=0 must Err");
        let alloc_kind = alloc_budget
            .check(0, alloc_over)
            .expect_err("alloc_over > max_alloc_bytes=0 must Err");
        let deadline_kind = deadline_budget
            .check(0, 0)
            .expect_err("expired deadline must Err");

        // Exhaustive match — no wildcards. Adding a new variant breaks
        // compilation here and forces an update to this test.
        match steps_kind {
            BudgetExceededKind::Steps => {}
            BudgetExceededKind::Alloc => prop_assert!(false, "steps overflow gave Alloc kind"),
            BudgetExceededKind::Deadline => {
                prop_assert!(false, "steps overflow gave Deadline kind")
            }
        }
        match alloc_kind {
            BudgetExceededKind::Steps => prop_assert!(false, "alloc overflow gave Steps kind"),
            BudgetExceededKind::Alloc => {}
            BudgetExceededKind::Deadline => {
                prop_assert!(false, "alloc overflow gave Deadline kind")
            }
        }
        match deadline_kind {
            BudgetExceededKind::Steps => prop_assert!(false, "deadline overflow gave Steps kind"),
            BudgetExceededKind::Alloc => prop_assert!(false, "deadline overflow gave Alloc kind"),
            BudgetExceededKind::Deadline => {}
        }

        // Discrimination is *pairwise* distinct.
        prop_assert_ne!(steps_kind, alloc_kind);
        prop_assert_ne!(steps_kind, deadline_kind);
        prop_assert_ne!(alloc_kind, deadline_kind);
    }

    /// Property 3 — default-grade budget admits trivial work.
    ///
    /// `EvalBudget::unlimited()` is the constructor that `evaluate`
    /// (no-budget entry point) delegates to. It must accept the usage
    /// tuple consistent with evaluating a trivial literal. Sanity
    /// property: pins the default's relevance to actual evaluator
    /// behavior. The `n` parameter varies the literal to widen
    /// coverage without changing the semantic shape.
    #[test]
    fn default_budget_admits_trivial_literal_evaluation(n in any::<i32>()) {
        let src = n.to_string();
        let expr = parse(&src).expect("integer literal parses");
        let env = MapEnvironment::new();
        let result = evaluate(&expr, &env);

        // The default-grade budget (delegated to by `evaluate`) must
        // not emit any budget-exceeded diagnostic for a trivial literal.
        let budget_diag = result
            .diagnostics
            .iter()
            .find(|d| d.message.contains("budget exceeded"));
        prop_assert!(
            budget_diag.is_none(),
            "default budget emitted budget diagnostic for trivial literal `{src}`: {budget_diag:?}",
        );
        // And the literal must round-trip to its numeric value.
        prop_assert_eq!(result.value, Value::Number(n.into()));

        // Also: direct `check()` on `unlimited()` with the trivial
        // usage cost (1 step, ~size_of::<Value>() bytes) is Ok.
        let unlimited = EvalBudget::unlimited();
        prop_assert!(unlimited.check(1, 64).is_ok());
    }
}
