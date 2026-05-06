#![allow(clippy::missing_docs_in_private_items)]
use rust_decimal::Decimal;

use crate::ast::*;
use crate::types::*;

use super::helpers::{fold_min_max_choice, fold_money_sum};
use super::super::core::Evaluator;
use super::super::util::dec;

impl<'a> Evaluator<'a> {
    // ── Aggregate helpers ───────────────────────────────────────

    pub(in crate::evaluator) fn fn_aggregate(
        &mut self,
        args: &[Expr],
        name: &str,
        f: fn(&[Decimal]) -> Decimal,
    ) -> Value {
        let val = self.eval_arg(args, 0);
        let arr = match self.get_array(&val, name) {
            Some(a) => a,
            None => return Value::Null,
        };
        if name == "sum" {
            let non_null: Vec<&Value> = arr.iter().filter(|v| !v.is_null()).collect();
            if non_null.iter().any(|v| matches!(v, Value::Money(_))) {
                self.diag("sum: money values are not supported; use moneySum() for currency-safe aggregation");
                return Value::Null;
            }
        }
        let nums: Vec<Decimal> = arr.iter().filter_map(|v| v.as_number()).collect();
        if nums.is_empty() && name == "avg" {
            self.diag(format!("{name}: no numeric elements"));
            return Value::Null;
        }
        Value::Number(f(&nums))
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
        if !self.require_min_args(args, 2, "countWhere") {
            return Value::Null;
        }
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
        if !self.require_min_args(args, 2, "every") {
            return Value::Null;
        }
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
        if !self.require_min_args(args, 2, "some") {
            return Value::Null;
        }
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
        let nums: Vec<Decimal> = matched.iter().filter_map(|v| v.as_number()).collect();
        Value::Number(nums.iter().copied().sum())
    }

    pub(in crate::evaluator) fn fn_avg_where(&mut self, args: &[Expr]) -> Value {
        let Some(matched) = self.filter_where(args, "avgWhere") else {
            return Value::Null;
        };
        let nums: Vec<Decimal> = matched.iter().filter_map(|v| v.as_number()).collect();
        if nums.is_empty() {
            return Value::Null;
        }
        Value::Number(nums.iter().copied().sum::<Decimal>() / Decimal::from(nums.len() as i64))
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
