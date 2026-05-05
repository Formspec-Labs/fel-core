#![allow(clippy::missing_docs_in_private_items)]
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::ast::*;
use crate::types::*;

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
            if !non_null.is_empty() && non_null.iter().all(|v| matches!(v, Value::Money(_))) {
                let total = non_null.iter().fold(Decimal::ZERO, |acc, value| {
                    acc + match value {
                        Value::Money(m) => m.amount,
                        _ => Decimal::ZERO,
                    }
                });
                return Value::Number(total);
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
            let mut best = non_null[0].clone();
            for elem in &non_null[1..] {
                let cmp = match (&best, *elem) {
                    (Value::Number(a), Value::Number(b)) => Some(a.cmp(b)),
                    (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
                    (Value::Date(a), Value::Date(b)) => Some(a.ordinal().cmp(&b.ordinal())),
                    _ => {
                        self.diag(format!("{name}: mixed types"));
                        return Value::Null;
                    }
                };
                if let Some(ord) = cmp
                    && ((is_min && ord.is_gt()) || (!is_min && ord.is_lt()))
                {
                    best = (*elem).clone();
                }
            }
            return best;
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
        let mut best = non_null[0].clone();
        for elem in &non_null[1..] {
            let cmp = match (&best, *elem) {
                (Value::Number(a), Value::Number(b)) => Some(a.cmp(b)),
                (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
                (Value::Date(a), Value::Date(b)) => Some(a.ordinal().cmp(&b.ordinal())),
                _ => {
                    self.diag(format!("{name}: mixed types"));
                    return Value::Null;
                }
            };
            if let Some(ord) = cmp
                && ((is_min && ord.is_gt()) || (!is_min && ord.is_lt()))
            {
                best = (*elem).clone();
            }
        }
        best
    }

    pub(in crate::evaluator) fn fn_count_where(&mut self, args: &[Expr]) -> Value {
        if args.len() < 2 {
            self.diag("countWhere: requires 2 arguments");
            return Value::Null;
        }
        let arr_val = self.eval(&args[0]);
        let arr = match self.get_array(&arr_val, "countWhere") {
            Some(a) => a,
            None => return Value::Null,
        };
        let mut count = 0i64;
        for elem in &arr {
            self.let_scopes
                .push(HashMap::from([("$".to_string(), elem.clone())]));
            let pred = self.eval(&args[1]);
            self.let_scopes.pop();
            if pred.is_truthy() {
                count += 1;
            }
        }
        Value::Number(dec(count))
    }

    pub(in crate::evaluator) fn fn_every(&mut self, args: &[Expr]) -> Value {
        if args.len() < 2 {
            self.diag("every: requires 2 arguments");
            return Value::Null;
        }
        let arr_val = self.eval(&args[0]);
        let arr = match self.get_array(&arr_val, "every") {
            Some(a) => a,
            None => return Value::Null,
        };
        for elem in &arr {
            self.let_scopes
                .push(HashMap::from([("$".to_string(), elem.clone())]));
            let pred = self.eval(&args[1]);
            self.let_scopes.pop();
            if !pred.is_truthy() {
                return Value::Boolean(false);
            }
        }
        Value::Boolean(true)
    }

    pub(in crate::evaluator) fn fn_some(&mut self, args: &[Expr]) -> Value {
        if args.len() < 2 {
            self.diag("some: requires 2 arguments");
            return Value::Null;
        }
        let arr_val = self.eval(&args[0]);
        let arr = match self.get_array(&arr_val, "some") {
            Some(a) => a,
            None => return Value::Null,
        };
        for elem in &arr {
            self.let_scopes
                .push(HashMap::from([("$".to_string(), elem.clone())]));
            let pred = self.eval(&args[1]);
            self.let_scopes.pop();
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
        let mut best = non_null[0].clone();
        for elem in &non_null[1..] {
            let cmp = match (&best, *elem) {
                (Value::Number(a), Value::Number(b)) => Some(a.cmp(b)),
                (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
                (Value::Date(a), Value::Date(b)) => Some(a.ordinal().cmp(&b.ordinal())),
                _ => {
                    self.diag("minWhere: mixed types".to_string());
                    return Value::Null;
                }
            };
            if let Some(ord) = cmp
                && ord.is_gt()
            {
                best = (*elem).clone();
            }
        }
        best
    }

    pub(in crate::evaluator) fn fn_max_where(&mut self, args: &[Expr]) -> Value {
        let Some(matched) = self.filter_where(args, "maxWhere") else {
            return Value::Null;
        };
        let non_null: Vec<&Value> = matched.iter().filter(|v| !v.is_null()).collect();
        if non_null.is_empty() {
            return Value::Null;
        }
        let mut best = non_null[0].clone();
        for elem in &non_null[1..] {
            let cmp = match (&best, *elem) {
                (Value::Number(a), Value::Number(b)) => Some(a.cmp(b)),
                (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
                (Value::Date(a), Value::Date(b)) => Some(a.ordinal().cmp(&b.ordinal())),
                _ => {
                    self.diag("maxWhere: mixed types".to_string());
                    return Value::Null;
                }
            };
            if let Some(ord) = cmp
                && ord.is_lt()
            {
                best = (*elem).clone();
            }
        }
        best
    }

    pub(in crate::evaluator) fn fn_money_sum_where(&mut self, args: &[Expr]) -> Value {
        let Some(matched) = self.filter_where(args, "moneySumWhere") else {
            return Value::Null;
        };
        let mut total: Option<Money> = None;
        for elem in &matched {
            match elem {
                Value::Money(m) => match &total {
                    None => total = Some(m.clone()),
                    Some(t) => {
                        if t.currency != m.currency {
                            self.diag("moneySumWhere: mixed currencies");
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
                    self.diag("moneySumWhere: non-money element");
                    return Value::Null;
                }
            }
        }
        match total {
            Some(t) => Value::Money(t),
            None => Value::Null,
        }
    }
}
