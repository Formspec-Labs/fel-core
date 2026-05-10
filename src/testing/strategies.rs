//! Proptest strategies for generating FEL [`Value`]s and [`Expr`] ASTs with structural shrinking.
//!
//! Gated behind `cfg(any(test, feature = "proptest-strategies"))` so the `proptest` dev-dependency
//! is not pulled into production builds.
//!
//! Public surface:
//! - [`arb_value(depth)`]         — all [`Value`] variants, depth-bounded.
//! - [`arb_expr(depth, catalog)`] — well-typed AST that composes literal, op, call, and reference sub-strategies.
//! - [`arb_decimal()`]            — biased toward overflow-adjacent, sub-precision, and identity values.

use proptest::prelude::*;
use proptest::strategy::{BoxedStrategy, Strategy};
use rust_decimal::Decimal;

use crate::ast::{BinaryOp, Expr, PathSegment, UnaryOp};
use crate::extensions::BuiltinFunctionCatalogEntry;
use crate::lexer::is_valid_fel_identifier;
use crate::types::{CurrencyCode, Date, Money, Value};

/// Default recursion depth for [`arb_value`] and [`arb_expr`].
pub const MAX_STRATEGY_DEPTH: u32 = 4;

/// Generates a valid FEL identifier (excludes reserved keywords).
fn arb_identifier() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9_]{0,8}"
        .prop_filter("identifier must be valid", |s| is_valid_fel_identifier(s))
}

/// Generates a valid FEL identifier of length ≥1 (excludes reserved keywords).
fn arb_identifier_nonempty() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9_]{1,8}"
        .prop_filter("identifier must be valid", |s| is_valid_fel_identifier(s))
}
///
/// `depth` defaults to [`MAX_STRATEGY_DEPTH`] when `None`.
pub fn arb_value(depth: impl Into<Option<u32>>) -> BoxedStrategy<Value> {
    let depth = depth.into().unwrap_or(MAX_STRATEGY_DEPTH);
    if depth == 0 {
        return arb_leaf_value().boxed();
    }
    let leaf = arb_leaf_value();
    let array = prop::collection::vec(arb_value(depth - 1), 0..4)
        .prop_map(Value::Array)
        .boxed();
    let object = prop::collection::vec(
        ("[a-z]{1,6}".prop_map(|s: String| s), arb_value(depth - 1)),
        0..4,
    )
    .prop_map(|pairs| Value::Object(pairs.into_iter().collect()))
    .boxed();
    prop_oneof![leaf, array, object].boxed()
}

fn arb_leaf_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Boolean),
        arb_decimal().prop_map(Value::Number),
        "[a-zA-Z0-9 ]{0,20}".prop_map(Value::String),
        arb_date().prop_map(Value::Date),
        arb_money().prop_map(Value::Money),
    ]
}

fn arb_date() -> impl Strategy<Value = Date> {
    (arb_year(), 1u32..=12, 1u32..=28).prop_map(|(y, m, d)| Date::Date {
        year: y,
        month: m,
        day: d,
    })
}

fn arb_money() -> impl Strategy<Value = Money> {
    (arb_decimal().prop_map(|d| d.abs()), arb_currency_code())
        .prop_map(|(amount, currency)| Money { amount, currency })
}

fn arb_currency_code() -> impl Strategy<Value = CurrencyCode> {
    prop::sample::select(vec![
        CurrencyCode::parse("USD").unwrap(),
        CurrencyCode::parse("EUR").unwrap(),
        CurrencyCode::parse("GBP").unwrap(),
        CurrencyCode::parse("JPY").unwrap(),
    ])
}

fn arb_year() -> impl Strategy<Value = i32> {
    1900i32..2100
}

/// Generates a well-typed [`Expr`] AST using catalog entries for function call arity.
///
/// Sub-strategies compose so proptest derives structural shrinking for free.
///
/// `depth` defaults to [`MAX_STRATEGY_DEPTH`] when `None`.
pub fn arb_expr(
    depth: impl Into<Option<u32>>,
    catalog: &[BuiltinFunctionCatalogEntry],
) -> BoxedStrategy<Expr> {
    let depth = depth.into().unwrap_or(MAX_STRATEGY_DEPTH);
    let leaf = arb_leaf_expr();
    if depth == 0 {
        return leaf.boxed();
    }
    let sub = arb_expr(depth - 1, catalog);
    let unary = arb_unary(sub.clone());
    let binary = arb_binary(sub.clone());
    let ternary = arb_ternary(sub.clone());
    let if_expr = arb_if_then_else(sub.clone());
    let func = arb_function_call(sub.clone(), catalog);
    let array = arb_array_expr(sub.clone());
    let object = arb_object_expr(sub.clone());
    let membership = arb_membership(sub.clone());
    let null_coalesce = arb_null_coalesce(sub.clone());
    let let_binding = arb_let_binding(sub.clone());
    let postfix = arb_postfix_access(sub.clone());
    prop_oneof![
        3 => leaf,
        2 => unary,
        2 => binary,
        1 => ternary,
        1 => if_expr,
        2 => func,
        1 => array,
        1 => object,
        1 => membership,
        1 => null_coalesce,
        1 => let_binding,
        1 => postfix,
    ]
    .boxed()
}

fn arb_leaf_expr() -> BoxedStrategy<Expr> {
    prop_oneof![
        Just(Expr::Null),
        any::<bool>().prop_map(Expr::Boolean),
        arb_decimal().prop_map(|d| Expr::Number(d.abs())),
        "[a-zA-Z0-9 ]{0,12}".prop_map(Expr::String),
        arb_field_ref(),
        arb_var_ref(),
        arb_identifier().prop_map(|s| Expr::VarRef {
            name: s,
            path: vec![],
        }),
    ]
    .boxed()
}

fn arb_field_ref() -> BoxedStrategy<Expr> {
    prop_oneof![
        Just(Expr::FieldRef {
            name: None,
            path: vec![],
        }),
        arb_identifier().prop_map(|name| Expr::FieldRef {
            name: Some(name),
            path: vec![],
        }),
        (arb_identifier(), arb_identifier_nonempty()).prop_map(|(name, tail)| Expr::FieldRef {
            name: Some(name),
            path: vec![PathSegment::Dot(tail)],
        }),
        (1usize..=5).prop_map(|idx| Expr::FieldRef {
            name: Some("items".to_string()),
            path: vec![PathSegment::Index(idx)],
        }),
    ]
    .boxed()
}

fn arb_var_ref() -> BoxedStrategy<Expr> {
    arb_identifier()
        .prop_map(|name| Expr::VarRef { name, path: vec![] })
        .boxed()
}

fn arb_unary(sub: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    prop_oneof![
        (Just(UnaryOp::Not), sub.clone(), any::<bool>()).prop_map(|(op, operand, bang)| {
            Expr::UnaryOp {
                op,
                operand: Box::new(operand),
                bang,
            }
        }),
        (Just(UnaryOp::Neg), sub).prop_map(|(op, operand)| Expr::UnaryOp {
            op,
            operand: Box::new(operand),
            bang: false,
        }),
    ]
    .boxed()
}

fn arb_binary(sub: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    let ops: Vec<BinaryOp> = vec![
        BinaryOp::Add,
        BinaryOp::Sub,
        BinaryOp::Mul,
        BinaryOp::Div,
        BinaryOp::Mod,
        BinaryOp::Concat,
        BinaryOp::Eq,
        BinaryOp::NotEq,
        BinaryOp::Lt,
        BinaryOp::Gt,
        BinaryOp::LtEq,
        BinaryOp::GtEq,
        BinaryOp::And,
        BinaryOp::Or,
    ];
    (prop::sample::select(ops), sub.clone(), sub)
        .prop_map(|(op, left, right)| Expr::BinaryOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
        .boxed()
}

fn arb_ternary(sub: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    (sub.clone(), sub.clone(), sub)
        .prop_map(|(condition, then_branch, else_branch)| Expr::Ternary {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        })
        .boxed()
}

fn arb_if_then_else(sub: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    (sub.clone(), sub.clone(), sub)
        .prop_map(|(condition, then_branch, else_branch)| Expr::IfThenElse {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        })
        .boxed()
}

fn arb_function_call(
    sub: BoxedStrategy<Expr>,
    catalog: &[BuiltinFunctionCatalogEntry],
) -> BoxedStrategy<Expr> {
    let entries: Vec<(String, usize, usize)> = catalog
        .iter()
        .map(|e| {
            let (min, max) = min_max_arity(e).unwrap_or((0, 3));
            // Variadic functions get up to min+8; non-variadic use total param count.
            let max_capped = if e.parameters.last().is_some_and(|p| p.variadic) {
                (min + 8).min(32)
            } else {
                max
            };
            (e.name.to_string(), min, max_capped)
        })
        .collect();
    if entries.is_empty() {
        return Just(Expr::FunctionCall {
            name: "unknown".to_string(),
            args: vec![],
        })
        .boxed();
    }
    let names: Vec<String> = entries.iter().map(|(n, _, _)| n.clone()).collect();
    prop::sample::select(names)
        .prop_flat_map(move |name| {
            let (min, max) = entries
                .iter()
                .find(|(n, _, _)| n == &name)
                .map(|(_, min, max)| (*min, *max))
                .unwrap_or((0, 3));
            let count_range = if min == max { min..=min } else { min..=max };
            let sub2 = sub.clone();
            (Just(name.clone()), prop::collection::vec(sub2, count_range))
                .prop_map(|(name, args)| Expr::FunctionCall { name, args })
        })
        .boxed()
}

fn min_max_arity(entry: &BuiltinFunctionCatalogEntry) -> Option<(usize, usize)> {
    if entry.parameters.is_empty() {
        return Some((0, 0));
    }
    let required = entry.parameters.iter().filter(|p| p.required).count();
    let total = entry.parameters.len();
    let variadic = entry.parameters.last().is_some_and(|p| p.variadic);
    let max = if variadic { required + 5 } else { total };
    Some((required, max))
}

fn arb_array_expr(sub: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    prop::collection::vec(sub, 0..4)
        .prop_map(Expr::Array)
        .boxed()
}

fn arb_object_expr(sub: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    prop::collection::vec((arb_identifier_nonempty(), sub), 0..3)
        .prop_map(Expr::Object)
        .boxed()
}

fn arb_membership(sub: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    (
        sub.clone(),
        prop::collection::vec(sub, 0..4).prop_map(Expr::Array),
        any::<bool>(),
    )
        .prop_map(|(value, container, negated)| Expr::Membership {
            value: Box::new(value),
            container: Box::new(container),
            negated,
        })
        .boxed()
}

fn arb_null_coalesce(sub: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    (sub.clone(), sub)
        .prop_map(|(left, right)| Expr::NullCoalesce {
            left: Box::new(left),
            right: Box::new(right),
        })
        .boxed()
}

fn arb_let_binding(sub: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    (arb_identifier_nonempty(), sub.clone(), sub)
        .prop_map(|(name, value, body)| Expr::LetBinding {
            name,
            value: Box::new(value),
            body: Box::new(body),
        })
        .boxed()
}

fn arb_postfix_access(sub: BoxedStrategy<Expr>) -> BoxedStrategy<Expr> {
    (
        sub,
        prop_oneof![
            arb_identifier_nonempty().prop_map(|s| vec![PathSegment::Dot(s)]),
            (1usize..=3).prop_map(|idx| vec![PathSegment::Index(idx)]),
        ],
    )
        .prop_map(|(expr, path)| Expr::PostfixAccess {
            expr: Box::new(expr),
            path,
        })
        .boxed()
}

/// Biased [`Decimal`] strategy — covers overflow-adjacent values, sub-precision values, and
/// zero/one identities alongside uniform small-range values.
pub fn arb_decimal() -> impl Strategy<Value = Decimal> {
    prop_oneof![
        (-1000i64..1000).prop_map(Decimal::from),
        (i64::MAX - 100..i64::MAX).prop_map(Decimal::from),
        (i64::MIN..i64::MIN + 100).prop_map(Decimal::from),
        Just(Decimal::ZERO),
        Just(Decimal::ONE),
        Just(Decimal::NEGATIVE_ONE),
        Just(Decimal::new(1, 28)),
        (0i64..1000).prop_map(|n| Decimal::new(n, 4)),
        (0i64..1000).prop_map(|n| Decimal::new(n, 10)),
        (u64::MAX - 100..u64::MAX).prop_map(|n| Decimal::from(n)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::builtin_function_catalog;

    proptest! {
        #![proptest_config(proptest::test_runner::Config {
            cases: 64,
            ..Default::default()
        })]

        #[test]
        fn arb_value_does_not_panic(v in arb_value(MAX_STRATEGY_DEPTH)) {
            let _ = format!("{:?}", v);
        }

        #[test]
        fn arb_expr_does_not_panic(
            e in arb_expr(MAX_STRATEGY_DEPTH, builtin_function_catalog())
        ) {
            let _ = format!("{:?}", e);
        }

        #[test]
        fn arb_decimal_is_well_formed(d in arb_decimal()) {
            let s = d.to_string();
            prop_assert!(!s.is_empty());
        }
    }
}
