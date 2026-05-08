//! Resource-budget enforcement for FEL evaluation.
//!
//! [`EvalBudget`] is threaded through `evaluate_with_budget` / `evaluate_with_budget_and_extensions`;
//! existing `evaluate` entry points delegate with [`EvalBudget::unlimited`] — no caller migration
//! required.
//!
//! The evaluator treats allocation over [`EvalBudget::max_alloc_bytes`] like step and deadline
//! limits: it records `budget exceeded (alloc)` and yields [`crate::types::Value::Null`] for the
//! affected literal, array, object, or `let` node (estimates are best-effort; see field docs).
use std::time::Instant;

/// Hard cap on evaluation resource consumption. Exceeding any limit returns `Err(BudgetExceededKind)`.
#[derive(Debug, Clone)]
pub struct EvalBudget {
    /// Maximum number of node evaluations before returning `BudgetExceeded { kind: Steps }`.
    pub max_steps: u64,
    /// Approximate allocation ceiling (bytes) before returning `BudgetExceeded { kind: Alloc }`.
    /// Tracked best-effort; does not account for internal `Vec`/`String` overhead.
    pub max_alloc_bytes: u64,
    /// Wall-clock deadline for interactive/UI use (clock-bound).
    /// Leave as `None` for throughput-bound batch / projection consumers.
    /// Expiration returns `BudgetExceeded { kind: Deadline }`.
    pub deadline: Option<Instant>,
}

/// Which resource limit was hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetExceededKind {
    /// Step count exceeded [`EvalBudget::max_steps`].
    Steps,
    /// Allocation exceeded [`EvalBudget::max_alloc_bytes`].
    Alloc,
    /// Wall-clock deadline expired.
    Deadline,
}

impl EvalBudget {
    /// Sentinel value that never triggers a limit — used by existing `evaluate*` entry points.
    pub const fn unlimited() -> Self {
        Self {
            max_steps: u64::MAX,
            max_alloc_bytes: u64::MAX,
            deadline: None,
        }
    }

    /// Smallest budget guaranteed to allow at least one evaluation step.
    pub const fn min_viable() -> Self {
        Self {
            max_steps: 1,
            max_alloc_bytes: 1024,
            deadline: None,
        }
    }

    /// Batch / projection use — no deadline, limited steps and allocation.
    pub const fn for_batch(steps: u64, alloc: u64) -> Self {
        Self {
            max_steps: steps,
            max_alloc_bytes: alloc,
            deadline: None,
        }
    }

    /// Interactive / UI use — unlimited steps and allocation, clock-bound.
    pub fn for_interactive(deadline: Instant) -> Self {
        Self {
            max_steps: u64::MAX,
            max_alloc_bytes: u64::MAX,
            deadline: Some(deadline),
        }
    }

    /// Check whether any limit has been exceeded.
    #[inline]
    pub fn check(&self, steps: u64, alloc_bytes: u64) -> Result<(), BudgetExceededKind> {
        if steps > self.max_steps {
            return Err(BudgetExceededKind::Steps);
        }
        if alloc_bytes > self.max_alloc_bytes {
            return Err(BudgetExceededKind::Alloc);
        }
        if let Some(deadline) = self.deadline {
            if Instant::now() >= deadline {
                return Err(BudgetExceededKind::Deadline);
            }
        }
        Ok(())
    }
}
