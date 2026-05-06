//! FEL tree-walking evaluator with base-10 decimal arithmetic and null propagation.
//!
//! Non-fatal errors produce a Diagnostic + FelNull (never panic).
//! Null propagation follows spec §3: most ops propagate, equality does NOT.
//!
//! The [`Evaluator`] owns `let` scopes and builtins; private `eval` / `fn_*` methods implement the tree walk.
#![allow(clippy::missing_docs_in_private_items)]

mod builtins;
mod core;
mod util;

pub use self::core::{
    Environment, EvalResult, Evaluator, MapEnvironment, evaluate, evaluate_with_extensions,
    evaluate_with_trace, evaluate_with_trace_and_extensions,
};

use std::collections::HashMap;

use crate::error::Error;
use crate::parser;
use crate::types::Value;

/// Parses and evaluates FEL with a flat field map.
pub fn eval_with_fields(
    input: &str,
    fields: HashMap<String, Value>,
) -> Result<EvalResult, Error> {
    let expr = parser::parse(input)?;
    let env = MapEnvironment::with_fields(fields);
    Ok(evaluate(&expr, &env))
}
