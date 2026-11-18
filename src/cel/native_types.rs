//! Checks the types and operations shared by the two source languages.
// Rust guideline compliant 2026-02-21

use super::InputType;
use super::bindings::Bindings;
use super::numeric::{integer, shared_fraction, shared_quotient, truncation};
use crate::ast::{BinaryOp as Op, PathSegment, UnaryOp};
use crate::{Expr, print_expr};

/// Types for which native conversion defines reversible operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    /// The null literal.
    Null,
    /// A non-null Boolean.
    Bool,
    /// An in-range signed integer.
    Int,
    /// An exactly representable fractional value.
    Fraction,
    /// A non-null string.
    String,
    /// A field declared as string or null.
    NullableString,
    /// The element type of an empty collection, inferred from its use.
    Unspecified,
    /// The element type of a heterogeneous collection or a declared form list.
    Dynamic,
    /// A non-null list with its CEL element type.
    List(Box<Kind>),
    /// A string-keyed object with its CEL value type.
    Object(Box<Kind>),
}

impl Kind {
    /// Combine compatible CEL types, retaining nesting and typed empty collections.
    fn common(self, next: Self) -> Option<Self> {
        use Kind::*;
        match (self, next) {
            (Unspecified, value) | (value, Unspecified) => Some(value),
            (Dynamic, _) | (_, Dynamic) => Some(Dynamic),
            (List(a), List(b)) => Some(List(Box::new(a.common(*b)?))),
            (Object(a), Object(b)) => Some(Object(Box::new(a.common(*b)?))),
            // The CEL checker retains the first string type when unifying wrappers.
            (first @ String, NullableString) | (first @ NullableString, String) => Some(first),
            (Null, next @ NullableString) | (NullableString, next @ Null) => Some(next),
            (a, b) if a == b => Some(a),
            _ => None,
        }
    }
}

/// Include the FEL expression that failed the native type rules.
fn unsupported(expr: &Expr, reason: &str) -> String {
    format!("cannot convert {}: {reason}", print_expr(expr))
}

/// Check the selected item and its siblings once, even for nested literal indexes.
fn indexed_kind(expr: &Expr, mut path: Vec<usize>, bindings: &Bindings) -> Result<Kind, String> {
    if let Expr::PostfixAccess {
        expr: base,
        path: steps,
    } = expr
    {
        for step in steps.iter().rev() {
            let PathSegment::Index(at) = step else {
                return Err(unsupported(
                    expr,
                    "index conversion requires a literal index",
                ));
            };
            path.push(*at);
        }
        return indexed_kind(base, path, bindings);
    }
    let Some(index) = path.pop() else {
        return kind(expr, bindings);
    };
    let Expr::Array(items) = expr else {
        return Err(unsupported(
            expr,
            "index conversion requires a literal list",
        ));
    };
    let at = index
        .checked_sub(1)
        .filter(|&i| i < items.len())
        .ok_or_else(|| {
            unsupported(
                expr,
                "out-of-bounds list access returns null in FEL and an error in CEL",
            )
        })?;
    for (i, item) in items.iter().enumerate() {
        if i != at {
            kind(item, bindings)?;
        }
    }
    indexed_kind(&items[at], path, bindings)
}

/// Infer the common element type of an ordered CEL list or map literal.
fn element_kind<'a>(
    values: impl Iterator<Item = &'a Expr>,
    bindings: &Bindings,
) -> Result<Kind, String> {
    let mut element = Kind::Unspecified;
    for value in values {
        element = element
            .common(kind(value, bindings)?)
            .unwrap_or(Kind::Dynamic);
    }
    Ok(element)
}

/// Check every operand and return the shared result type.
pub(super) fn kind(expr: &Expr, bindings: &Bindings) -> Result<Kind, String> {
    use Kind::*;
    let result = match expr {
        Expr::Null => Null,
        Expr::Boolean(_) => Bool,
        Expr::String(_) => String,
        Expr::Number(_) if shared_fraction(expr) => Fraction,
        Expr::UnaryOp {
            op: UnaryOp::Neg,
            operand,
            ..
        } if shared_fraction(operand) => Fraction,
        Expr::Number(_)
        | Expr::UnaryOp {
            op: UnaryOp::Neg, ..
        } => {
            integer(expr).ok_or_else(|| {
                unsupported(expr, "the numeric literal requires a signed 64-bit integer or an exactly representable binary fraction")
            })?;
            Int
        }
        Expr::FieldRef { .. } | Expr::ContextRef { .. } | Expr::VarRef { .. } => {
            let input = bindings
                .inputs
                .iter()
                .find(|input| input.reference == *expr)
                .ok_or_else(|| {
                    format!("FEL reference {} has no CEL binding here", print_expr(expr))
                })?;
            match input.kind {
                InputType::NullableString => NullableString,
                InputType::String => String,
                InputType::Int => Int,
                InputType::Bool => Bool,
                InputType::List => List(Box::new(Dynamic)),
            }
        }
        Expr::Array(values) => List(Box::new(element_kind(values.iter(), bindings)?)),
        Expr::Object(entries) => {
            let mut seen = std::collections::HashSet::new();
            for (key, _) in entries {
                if !seen.insert(key) {
                    return Err(unsupported(expr, "duplicate object key"));
                }
            }
            Object(Box::new(element_kind(
                entries.iter().map(|(_, value)| value),
                bindings,
            )?))
        }
        Expr::PostfixAccess { .. } => indexed_kind(expr, Vec::new(), bindings)?,
        Expr::UnaryOp {
            op: UnaryOp::Not,
            operand,
            ..
        } if kind(operand, bindings)? == Bool => Bool,
        Expr::BinaryOp { op, left, right } => {
            let pair = (kind(left, bindings)?, kind(right, bindings)?);
            match op {
                Op::Eq | Op::NotEq => match pair {
                    (String, Null) | (Null, String) => {
                        return Err(unsupported(
                            expr,
                            "the CEL reference checker rejects comparison of a string literal with a null literal or a non-nullable string with null",
                        ));
                    }
                    (String | NullableString, String | NullableString)
                    | (Null, Null | NullableString)
                    | (NullableString, Null)
                    | (Int, Int)
                    | (Fraction, Fraction)
                    | (Bool, Bool) => Bool,
                    _ => {
                        return Err(unsupported(
                            expr,
                            "equality requires compatible scalar types",
                        ));
                    }
                },
                Op::Lt | Op::LtEq | Op::Gt | Op::GtEq
                    if matches!(pair, (Int, Int) | (Fraction, Fraction) | (String, String)) =>
                {
                    Bool
                }
                Op::And | Op::Or if pair == (Bool, Bool) => Bool,
                Op::Concat if pair == (String, String) => String,
                Op::Add | Op::Sub | Op::Mul | Op::Mod if pair == (Int, Int) => {
                    // The emitted expression keeps the operation. This check proves it is in range.
                    let safe = integer(left)
                        .zip(integer(right))
                        .and_then(|(a, b)| match op {
                            Op::Add => a.checked_add(b),
                            Op::Sub => a.checked_sub(b),
                            Op::Mul => a.checked_mul(b),
                            Op::Mod => a.checked_rem(b),
                            _ => None,
                        });
                    safe.ok_or_else(|| unsupported(expr, "CEL integer arithmetic needs a proven in-range result and a valid divisor"))?;
                    Int
                }
                Op::Div if shared_quotient(left, right) => Fraction,
                Op::Div => {
                    return Err(unsupported(
                        expr,
                        "FEL decimal division and CEL integer division differ",
                    ));
                }
                _ => {
                    return Err(unsupported(
                        expr,
                        "operator needs compatible non-nullable operand types",
                    ));
                }
            }
        }
        Expr::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            let yes = kind(then_branch, bindings)?;
            let no = kind(else_branch, bindings)?;
            if kind(condition, bindings)? != Bool {
                return Err(unsupported(expr, "a conditional needs a Boolean condition"));
            }
            if yes == no {
                yes
            } else if matches!((&yes, &no), (List(_), List(_)) | (Object(_), Object(_))) {
                yes.common(no).ok_or_else(|| {
                    unsupported(
                        expr,
                        "a conditional needs compatible collection element types",
                    )
                })?
            } else {
                return Err(unsupported(
                    expr,
                    "a conditional needs matching branch types",
                ));
            }
        }
        Expr::Membership {
            value, container, ..
        } => {
            let ty = kind(value, bindings)?;
            let Expr::Array(items) = container.as_ref() else {
                return Err(unsupported(
                    expr,
                    "membership needs a literal list of comparable scalars",
                ));
            };
            if !matches!(ty, String | Int | Bool | Fraction) {
                return Err(unsupported(expr, "membership needs a non-nullable scalar"));
            }
            for item in items {
                if kind(item, bindings)? != ty {
                    return Err(unsupported(expr, "list element types differ"));
                }
            }
            Bool
        }
        Expr::FunctionCall { name, args } => match (name.as_str(), args.as_slice()) {
            (
                "floor" | "ceil",
                [
                    Expr::BinaryOp {
                        op: Op::Div,
                        left,
                        right,
                    },
                ],
            ) if truncation(left, right) == Some(name.as_str()) => Int,
            ("contains" | "startsWith" | "endsWith", [a, b])
                if kind(a, bindings)? == String && kind(b, bindings)? == String =>
            {
                Bool
            }
            ("length", [a]) if matches!(kind(a, bindings)?, String | List(_)) => Int,
            ("count", [a]) if matches!(kind(a, bindings)?, List(_)) => Int,
            _ => {
                return Err(format!(
                    "unsupported FEL expression {}: this function or its argument types have no native mapping here",
                    print_expr(expr)
                ));
            }
        },
        _ => return Err(format!("unsupported FEL expression {}", print_expr(expr))),
    };
    Ok(result)
}
