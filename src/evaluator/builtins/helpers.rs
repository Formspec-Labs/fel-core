//! Shared helpers for builtin implementations (ordering, money folds).
#![allow(clippy::missing_docs_in_private_items)]

use std::cmp::Ordering;

use crate::types::{Money, Value};

use super::super::core::Evaluator;

/// Compare two values for scalar min/max (`Number`, `String`, `Date`). Mixed shapes emit a diagnostic.
pub(in crate::evaluator::builtins) fn cmp_ordered_min_max(
    eval: &mut Evaluator,
    fn_label: &str,
    a: &Value,
    b: &Value,
) -> Option<Ordering> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => Some(x.cmp(y)),
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        (Value::Date(x), Value::Date(y)) => Some(x.ordinal().cmp(&y.ordinal())),
        _ => {
            eval.diag(format!("{fn_label}: mixed types"));
            None
        }
    }
}

/// Reduce non-empty slice to min (`pick_smaller`) or max (`!pick_smaller`). Returns `None` on mixed types (after diagnostic).
pub(in crate::evaluator::builtins) fn fold_min_max_choice(
    eval: &mut Evaluator,
    fn_label: &str,
    elems: &[&Value],
    pick_smaller: bool,
) -> Option<Value> {
    debug_assert!(!elems.is_empty());
    let mut best = elems[0].clone();
    for elem in &elems[1..] {
        let ord = cmp_ordered_min_max(eval, fn_label, &best, *elem)?;
        let replace = if pick_smaller {
            ord.is_gt()
        } else {
            ord.is_lt()
        };
        if replace {
            best = (*elem).clone();
        }
    }
    Some(best)
}

/// Sum money elements; skips nulls; mismatched currency or non-money emits diagnostic and returns null.
pub(in crate::evaluator::builtins) fn fold_money_sum<'a, I>(
    eval: &mut Evaluator,
    fn_label: &str,
    elements: I,
) -> Value
where
    I: IntoIterator<Item = &'a Value>,
{
    let mut total: Option<Money> = None;
    for elem in elements {
        match elem {
            Value::Money(m) => match &total {
                None => total = Some(m.clone()),
                Some(t) => {
                    if t.currency != m.currency {
                        eval.diag_coded(
                            "FEL_MONEY_SUM_MIXED_CURRENCIES",
                            format!("{fn_label}: mixed currencies"),
                        );
                        return Value::Null;
                    }
                    total = Some(Money {
                        amount: t.amount + m.amount,
                        currency: t.currency.clone(),
                    });
                }
            },
            Value::Null => {}
            _ => {
                eval.diag_coded(
                    "FEL_MONEY_SUM_NON_MONEY",
                    format!("{fn_label}: non-money element"),
                );
                return Value::Null;
            }
        }
    }
    match total {
        Some(t) => Value::Money(t),
        None => Value::Null,
    }
}
