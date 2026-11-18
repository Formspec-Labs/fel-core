//! Static builtin catalog data (`BUILTIN_FUNCTIONS`) and reserved names.
#![allow(clippy::missing_docs_in_private_items)]

use std::sync::LazyLock;

use super::types::*;
use crate::ast::Expr;
use crate::error::Diagnostic;

mod aggregate;
mod date;
mod locale;
mod logical;
mod mip;
mod money;
mod numeric;
mod repeat;
mod string;
mod type_;

pub(crate) const RESERVED_WORDS: &[&str] = &[
    "true", "false", "null", "let", "in", "if", "then", "else", "and", "or", "not",
];

pub(crate) static BUILTIN_FUNCTIONS: LazyLock<Vec<BuiltinFunctionCatalogEntry>> =
    LazyLock::new(|| {
        let mut entries = Vec::new();
        entries.extend_from_slice(aggregate::ENTRIES);
        entries.extend_from_slice(string::ENTRIES);
        entries.extend_from_slice(numeric::ENTRIES);
        entries.extend_from_slice(date::ENTRIES);
        entries.extend_from_slice(logical::ENTRIES);
        entries.extend_from_slice(type_::ENTRIES);
        entries.extend_from_slice(money::ENTRIES);
        entries.extend_from_slice(locale::ENTRIES);
        entries.extend_from_slice(mip::ENTRIES);
        entries.extend_from_slice(repeat::ENTRIES);
        entries
    });

/// Slice of all built-in functions.
///
/// Names in this catalog are reserved for
/// [`ExtensionRegistry::register`](crate::extensions::ExtensionRegistry::register).
pub fn builtin_function_catalog() -> &'static [BuiltinFunctionCatalogEntry] {
    BUILTIN_FUNCTIONS.as_slice()
}

/// Catalog filtered to entries reachable from `package`.
///
/// `Package::Formspec` returns the union of `Universal` and `Formspec` entries
/// (formspec hosts can call everything). `Package::Universal` returns only
/// `Universal` entries - appropriate for hosts that use [`crate::MapEnvironment`] or
/// any non-formspec [`crate::Environment`] implementation.
pub fn builtin_function_catalog_for(
    package: Package,
) -> impl Iterator<Item = &'static BuiltinFunctionCatalogEntry> {
    BUILTIN_FUNCTIONS.iter().filter(move |e| match package {
        Package::Universal => matches!(e.package, Package::Universal),
        Package::Formspec => true,
    })
}

/// Look up the catalog-declared arity for `name` as `(min_args, max_args)`.
///
/// Returns `None` when `name` is not a built-in. The evaluator's pre-dispatch gate and
/// [`builtin_arity_mismatches`] both apply these bounds, so every catalog-declared
/// signature is enforced the same way.
pub fn builtin_arity(name: &str) -> Option<(usize, Option<usize>)> {
    BUILTIN_FUNCTIONS
        .iter()
        .find(|e| e.name == name)
        .map(BuiltinFunctionCatalogEntry::arity)
}

/// The arity diagnostic for a built-in `name` called with `got` arguments, if out of bounds.
///
/// The evaluator refuses such a call before dispatch; [`builtin_arity_mismatches`] reports the
/// same refusals without evaluating.
pub(crate) fn builtin_arity_mismatch(name: &str, got: usize) -> Option<Diagnostic> {
    let (min, max) = builtin_arity(name)?;
    (got < min || max.is_some_and(|max| got > max))
        .then(|| Diagnostic::arity_mismatch(name, min, max, got))
}

/// Arity diagnostics for every built-in call in `expr` the evaluator would refuse, in source order.
///
/// Core §3.10.1 makes an arity mismatch a Definition error, so hosts that check a Definition
/// before evaluating it report these up front. Extension calls are checked by their registry.
pub fn builtin_arity_mismatches(expr: &Expr) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    visit_calls(expr, &mut |name, got| {
        found.extend(builtin_arity_mismatch(name, got));
    });
    found
}

/// Call `visit` with the name and argument count of every function call in `expr`.
fn visit_calls(expr: &Expr, visit: &mut impl FnMut(&str, usize)) {
    match expr {
        Expr::FunctionCall { name, args } => {
            visit(name, args.len());
            for arg in args {
                visit_calls(arg, visit);
            }
        }
        Expr::Array(items) => items.iter().for_each(|item| visit_calls(item, visit)),
        Expr::Object(entries) => entries
            .iter()
            .for_each(|(_, value)| visit_calls(value, visit)),
        Expr::UnaryOp { operand, .. } => visit_calls(operand, visit),
        Expr::BinaryOp { left, right, .. } | Expr::NullCoalesce { left, right } => {
            visit_calls(left, visit);
            visit_calls(right, visit);
        }
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
            visit_calls(condition, visit);
            visit_calls(then_branch, visit);
            visit_calls(else_branch, visit);
        }
        Expr::Membership {
            value, container, ..
        } => {
            visit_calls(value, visit);
            visit_calls(container, visit);
        }
        Expr::LetBinding { value, body, .. } => {
            visit_calls(value, visit);
            visit_calls(body, visit);
        }
        Expr::PostfixAccess { expr, .. } => visit_calls(expr, visit),
        Expr::RowLookup { source, key, .. } => {
            visit_calls(source, visit);
            visit_calls(key, visit);
        }
        Expr::Null
        | Expr::Boolean(_)
        | Expr::Number(_)
        | Expr::String(_)
        | Expr::DateLiteral(_)
        | Expr::DateTimeLiteral(_)
        | Expr::FieldRef { .. }
        | Expr::VarRef { .. }
        | Expr::ContextRef { .. } => {}
    }
}
