//! FEL parser, evaluator, and dependency analysis with base-10 decimal arithmetic.
//!
//! Uses `rust_decimal` for base-10 arithmetic per spec S3.4.1 (minimum 18 significant digits).
//!
//! ## Docs
//!
//! - Human overview: crate `README.md` (architecture, pipeline, module map).
//! - API reference: `cargo doc -p fel-core --no-deps --open`.
//! - Markdown API export: `docs/rustdoc-md/API.md` (see crate README).
#![deny(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

pub mod ast;
pub(crate) mod context_json;
pub mod convert;
pub mod dependencies;
pub mod environment;
pub mod error;
pub mod evaluator;
pub mod extensions;
pub(crate) mod interpolation;
pub(crate) mod iso_duration;
pub mod lexer;
pub mod parser;
pub mod prepare_host;
pub mod printer;
pub(crate) mod trace;
pub mod types;
pub(crate) mod wire_style;

#[cfg(any(test, feature = "proptest-strategies"))]
pub mod testing;

// Re-export key types
pub use ast::Expr;
pub use context_json::formspec_environment_from_json_map;
pub use convert::{
    fel_to_json, fel_to_ui_json, fel_to_wire_json, field_map_from_json_str, json_object_to_field_map,
    json_to_fel,
};
pub use dependencies::{
    Dependencies, dependencies_to_json_value, dependencies_to_json_value_styled,
    extract_dependencies,
};
pub use environment::{FormspecEnvironment, MipState, RepeatContext};
pub use error::{
    Diagnostic, DiagnosticKind, Error, ParseError, Severity, fel_diagnostics_to_json_value,
    fel_diagnostics_to_json_value_styled, has_error_diagnostics, reject_undefined_functions,
    undefined_function_names_from_diagnostics,
};
pub use evaluator::{
    BudgetExceededKind, Environment, EvalBudget, EvalResult, Evaluator, EvaluatorOptions,
    MapEnvironment, evaluate, evaluate_with, evaluate_with_budget,
    evaluate_with_budget_and_extensions, evaluate_with_extensions, evaluate_with_trace,
    evaluate_with_trace_and_budget, evaluate_with_trace_and_extensions,
    evaluate_with_trace_and_extensions_and_budget, eval_with_fields,
};
pub use indexmap::IndexMap;
pub use extensions::{
    ExtensionError, ExtensionFn, ExtensionFunc, ExtensionRegistry, Package,
    builtin_function_catalog, builtin_function_catalog_for, builtin_function_catalog_json_value,
    builtin_function_catalog_json_value_for,
};
pub use interpolation::expr_is_interpolation_static_literal;
pub use iso_duration::{IsoDurationParse, parse_iso8601_duration, parse_iso8601_duration_ms};
pub use lexer::{
    is_valid_fel_identifier, sanitize_fel_identifier, PositionedToken, tokenize,
    tokenize_to_json_value, tokenize_to_json_value_styled,
};
pub use parser::parse;
pub use prepare_host::{
    PrepareHostInput, PrepareHostOptions, host_options_from_json, prepare, prepare_for_host,
};
pub use printer::print_expr;
pub use trace::{Trace, TraceStep};
pub use types::{
    CurrencyCode, Date, Money, Value, parse_date_literal, parse_datetime_literal,
    value_size_estimate,
};
pub use wire_style::JsonWireStyle;
