//! Helpers for `{{expression}}` message interpolation (locale spec §3.3.1).
#![allow(missing_docs)]
#![allow(clippy::missing_docs_in_private_items)]

use crate::ast::{Expr, UnaryOp};

/// Functions that read response or instance data while spelling no `$` or `@` sigil: the row before, the
/// row after, the enclosing row, and a named secondary instance. An expression that calls one reads data,
/// so a `null` from it is an absence to render, not an author's typo (locale §3.3.1 rule 3a).
const DATA_READING_FUNCTIONS: [&str; 4] = ["prev", "next", "parent", "instance"];

/// True when the AST reads instance data: a `$field`, an `@context`, or a repeat navigation call
/// (locale §3.3.1 rule 3a).
///
/// The rule keeps a `null` result literal only for an expression that reads none — an author's typo or a
/// broken operator sequence — so that a missing `$field` still renders as empty text. Navigation across
/// rows belongs on the reading side: `{{prev().total}}` is null on the first row for a good reason, and a
/// respondent should read an empty space there, not the template.
pub fn expr_references_instance_data(expr: &Expr) -> bool {
    match expr {
        Expr::FieldRef { .. } | Expr::ContextRef { .. } => true,
        Expr::FunctionCall { name, args } => {
            DATA_READING_FUNCTIONS.contains(&name.as_str())
                || args.iter().any(expr_references_instance_data)
        }
        Expr::Null
        | Expr::Boolean(_)
        | Expr::Number(_)
        | Expr::String(_)
        | Expr::DateLiteral(_)
        | Expr::DateTimeLiteral(_)
        | Expr::VarRef { .. } => false,
        Expr::Array(elems) => elems.iter().any(expr_references_instance_data),
        Expr::Object(entries) => entries.iter().any(|(_, v)| expr_references_instance_data(v)),
        Expr::UnaryOp { operand, .. } => expr_references_instance_data(operand),
        Expr::PostfixAccess { expr, .. } => expr_references_instance_data(expr),
        Expr::BinaryOp { left, right, .. }
        | Expr::Membership {
            value: left,
            container: right,
            ..
        }
        | Expr::NullCoalesce { left, right } => {
            expr_references_instance_data(left) || expr_references_instance_data(right)
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
            expr_references_instance_data(condition)
                || expr_references_instance_data(then_branch)
                || expr_references_instance_data(else_branch)
        }
        Expr::LetBinding { value, body, .. } => {
            expr_references_instance_data(value) || expr_references_instance_data(body)
        }
    }
}

/// True when the AST is only literals and unary `not`/`!`/`-` on such (locale §3.3.1).
pub fn expr_is_interpolation_static_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Null
        | Expr::Boolean(_)
        | Expr::Number(_)
        | Expr::String(_)
        | Expr::DateLiteral(_)
        | Expr::DateTimeLiteral(_) => true,
        Expr::Array(elems) => elems.iter().all(expr_is_interpolation_static_literal),
        Expr::Object(entries) => entries
            .iter()
            .all(|(_, v)| expr_is_interpolation_static_literal(v)),
        Expr::UnaryOp {
            op: UnaryOp::Not | UnaryOp::Neg,
            operand,
            ..
        } => expr_is_interpolation_static_literal(operand),
        Expr::VarRef { .. }
        | Expr::ContextRef { .. }
        | Expr::FunctionCall { .. }
        | Expr::PostfixAccess { .. }
        | Expr::BinaryOp { .. }
        | Expr::Ternary { .. }
        | Expr::IfThenElse { .. }
        | Expr::Membership { .. }
        | Expr::NullCoalesce { .. }
        | Expr::LetBinding { .. }
        | Expr::FieldRef { .. } => false,
    }
}
