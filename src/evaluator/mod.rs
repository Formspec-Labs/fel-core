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
    evaluate_with_trace,
};
