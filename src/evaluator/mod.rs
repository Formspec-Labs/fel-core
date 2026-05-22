//! FEL tree-walking evaluator with base-10 decimal arithmetic and null propagation.
//!
//! Non-fatal errors produce a Diagnostic + FelNull (never panic).
//! Null propagation follows spec §3: most ops propagate, equality does NOT.
//!
//! The [`Evaluator`] owns `let` scopes and builtins; private `eval` / `fn_*` methods implement the tree walk.
#![allow(clippy::missing_docs_in_private_items)]

mod budget;
mod builtins;
mod core;
mod util;

pub use self::budget::{BudgetExceededKind, EvalBudget};
pub use self::core::{
    ContextBinding, ContextBindingCatalog, ContextBindingKind, EmptyCatalog, Environment,
    EvalResult, Evaluator, EvaluatorOptions, MapEnvironment, UNBOUND_CONTEXT_REF_CODE, evaluate,
    evaluate_with, evaluate_with_catalog,
};

use std::collections::HashMap;

use crate::error::Error;
use crate::parser;
use crate::types::Value;

/// Parses and evaluates FEL with a flat field map.
pub fn eval_with_fields(input: &str, fields: HashMap<String, Value>) -> Result<EvalResult, Error> {
    let expr = parser::parse(input)?;
    let env = MapEnvironment::with_fields(fields);
    Ok(evaluate(&expr, &env))
}
