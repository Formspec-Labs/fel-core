#![allow(clippy::missing_docs_in_private_items)]
use crate::ast::*;
use crate::types::*;

use super::super::core::Evaluator;
use super::helpers::fold_money_sum;

impl<'a> Evaluator<'a> {
    // ── Money helpers ───────────────────────────────────────────

    pub(in crate::evaluator) fn fn_money(&mut self, args: &[Expr]) -> Value {
        let amount = match self.eval_arg(args, 0) {
            Value::Number(n) => n,
            _ => return Value::Null,
        };
        let currency_str = match self.eval_arg(args, 1) {
            Value::String(s) => s,
            _ => return Value::Null,
        };
        let Some(currency) = CurrencyCode::parse(&currency_str) else {
            self.diag("money: currency must be a three-letter ISO code");
            return Value::Null;
        };
        Value::Money(Money { amount, currency })
    }

    pub(in crate::evaluator) fn fn_money_add(&mut self, args: &[Expr]) -> Value {
        let a = match self.eval_arg(args, 0) {
            Value::Money(m) => m,
            _ => return Value::Null,
        };
        let b = match self.eval_arg(args, 1) {
            Value::Money(m) => m,
            _ => return Value::Null,
        };
        if a.currency != b.currency {
            self.diag("moneyAdd: currency mismatch");
            return Value::Null;
        }
        Value::Money(Money {
            amount: a.amount + b.amount,
            currency: a.currency,
        })
    }

    pub(in crate::evaluator) fn fn_money_sum(&mut self, args: &[Expr]) -> Value {
        let val = self.eval_arg(args, 0);
        let arr = match self.get_array(&val, "moneySum") {
            Some(a) => a,
            None => return Value::Null,
        };
        fold_money_sum(self, "moneySum", arr.iter())
    }
}
