#![allow(clippy::missing_docs_in_private_items)]
use rust_decimal::Decimal;

use crate::ast::*;
use crate::types::*;

use super::super::core::Evaluator;
use super::super::util::dec;
use super::helpers::{fold_min_max_choice, fold_money_sum};

impl<'a> Evaluator<'a> {
    // ── Aggregate helpers ───────────────────────────────────────

    pub(in crate::evaluator) fn fn_aggregate(&mut self, args: &[Expr], name: &str) -> Value {
        let val = self.eval_arg(args, 0);
        let arr = match self.get_array(&val, name) {
            Some(a) => a,
            None => return Value::Null,
        };
        if name == "sum" && arr.iter().any(|v| matches!(v, Value::Money(_))) {
            self.diag_coded(
                "FEL_SUM_REJECTS_MONEY",
                "sum: money values are not supported; use moneySum() for currency-safe aggregation",
            );
            return Value::Null;
        }
        self.fold_numeric_aggregate(name, arr)
    }

    fn fold_numeric_aggregate(&mut self, name: &str, elements: &[Value]) -> Value {
        let mut total = Decimal::ZERO;
        let mut count = 0usize;
        for number in elements.iter().filter_map(Value::as_number) {
            let Some(next) = total.checked_add(number) else {
                self.diag_coded(
                    "FEL_ARITHMETIC_OVERFLOW",
                    format!("{name}: arithmetic overflow"),
                );
                return Value::Null;
            };
            total = next;
            count += 1;
        }
        if matches!(name, "avg" | "avgWhere") {
            if count == 0 {
                if name == "avg" {
                    self.diag("avg: no numeric elements");
                }
                return Value::Null;
            }
            total /= Decimal::from(count);
        }
        Value::Number(total)
    }

    pub(in crate::evaluator) fn fn_count(&mut self, val: &Value) -> Value {
        match val {
            Value::Array(a) => Value::Number(dec(a.iter().filter(|v| !v.is_null()).count() as i64)),
            Value::Null => Value::Null,
            _ => Value::Null,
        }
    }

    pub(in crate::evaluator) fn fn_min_max(&mut self, args: &[Expr], is_min: bool) -> Value {
        let name = if is_min { "min" } else { "max" };

        // Variadic scalar form: min(a, b, ...) — evaluate all args as an implicit array
        if args.len() >= 2 {
            let vals: Vec<Value> = args.iter().map(|a| self.eval(a)).collect();
            let non_null: Vec<&Value> = vals.iter().filter(|v| !v.is_null()).collect();
            if non_null.is_empty() {
                return Value::Null;
            }
            return fold_min_max_choice(self, name, &non_null, is_min).unwrap_or(Value::Null);
        }

        // Aggregate form: min([1, 2, 3])
        let val = self.eval_arg(args, 0);
        let arr = match self.get_array(&val, name) {
            Some(a) => a,
            None => return Value::Null,
        };
        let non_null: Vec<&Value> = arr.iter().filter(|v| !v.is_null()).collect();
        if non_null.is_empty() {
            return Value::Null;
        }
        fold_min_max_choice(self, name, &non_null, is_min).unwrap_or(Value::Null)
    }

    pub(in crate::evaluator) fn fn_count_where(&mut self, args: &[Expr]) -> Value {
        // Arity enforced by the uniform pre-dispatch gate in `eval_function`.
        let arr_val = self.eval(&args[0]);
        let arr = match self.get_array(&arr_val, "countWhere") {
            Some(a) => a,
            None => return Value::Null,
        };
        let mut count = 0i64;
        for elem in arr {
            let pred = self.eval_under_dollar(elem, &args[1]);
            if pred.is_truthy() {
                count += 1;
            }
        }
        Value::Number(dec(count))
    }

    pub(in crate::evaluator) fn fn_every(&mut self, args: &[Expr]) -> Value {
        // Arity enforced by the uniform pre-dispatch gate in `eval_function`.
        let arr_val = self.eval(&args[0]);
        let arr = match self.get_array(&arr_val, "every") {
            Some(a) => a,
            None => return Value::Null,
        };
        for elem in arr {
            let pred = self.eval_under_dollar(elem, &args[1]);
            if !pred.is_truthy() {
                return Value::Boolean(false);
            }
        }
        Value::Boolean(true)
    }

    pub(in crate::evaluator) fn fn_some(&mut self, args: &[Expr]) -> Value {
        // Arity enforced by the uniform pre-dispatch gate in `eval_function`.
        let arr_val = self.eval(&args[0]);
        let arr = match self.get_array(&arr_val, "some") {
            Some(a) => a,
            None => return Value::Null,
        };
        for elem in arr {
            let pred = self.eval_under_dollar(elem, &args[1]);
            if pred.is_truthy() {
                return Value::Boolean(true);
            }
        }
        Value::Boolean(false)
    }

    pub(in crate::evaluator) fn fn_sum_where(&mut self, args: &[Expr]) -> Value {
        let Some(matched) = self.filter_where(args, "sumWhere") else {
            return Value::Null;
        };
        self.fold_numeric_aggregate("sumWhere", &matched)
    }

    pub(in crate::evaluator) fn fn_avg_where(&mut self, args: &[Expr]) -> Value {
        let Some(matched) = self.filter_where(args, "avgWhere") else {
            return Value::Null;
        };
        self.fold_numeric_aggregate("avgWhere", &matched)
    }

    pub(in crate::evaluator) fn fn_min_where(&mut self, args: &[Expr]) -> Value {
        let Some(matched) = self.filter_where(args, "minWhere") else {
            return Value::Null;
        };
        let non_null: Vec<&Value> = matched.iter().filter(|v| !v.is_null()).collect();
        if non_null.is_empty() {
            return Value::Null;
        }
        fold_min_max_choice(self, "minWhere", &non_null, true).unwrap_or(Value::Null)
    }

    pub(in crate::evaluator) fn fn_max_where(&mut self, args: &[Expr]) -> Value {
        let Some(matched) = self.filter_where(args, "maxWhere") else {
            return Value::Null;
        };
        let non_null: Vec<&Value> = matched.iter().filter(|v| !v.is_null()).collect();
        if non_null.is_empty() {
            return Value::Null;
        }
        fold_min_max_choice(self, "maxWhere", &non_null, false).unwrap_or(Value::Null)
    }

    pub(in crate::evaluator) fn fn_money_sum_where(&mut self, args: &[Expr]) -> Value {
        let Some(matched) = self.filter_where(args, "moneySumWhere") else {
            return Value::Null;
        };
        fold_money_sum(self, "moneySumWhere", matched.iter())
    }
}
