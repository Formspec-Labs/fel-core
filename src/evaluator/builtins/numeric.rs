#![allow(clippy::missing_docs_in_private_items)]
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

use crate::ast::*;
use crate::types::*;

use super::super::core::Evaluator;

impl<'a> Evaluator<'a> {
    // ── Numeric helpers ─────────────────────────────────────────

    pub(in crate::evaluator) fn fn_num1(
        &mut self,
        args: &[Expr],
        f: fn(Decimal) -> Decimal,
    ) -> Value {
        match self.eval_arg(args, 0) {
            Value::Number(n) => Value::Number(f(n)),
            Value::Null => Value::Null,
            _ => Value::Null,
        }
    }

    pub(in crate::evaluator) fn fn_round(&mut self, args: &[Expr]) -> Value {
        let n = match self.eval_arg(args, 0) {
            Value::Number(n) => n,
            Value::Null => return Value::Null,
            _ => return Value::Null,
        };
        let precision = if args.len() > 1 {
            match self.eval_arg(args, 1) {
                Value::Number(p) => p.to_i32().unwrap_or(0),
                _ => 0,
            }
        } else {
            0
        };
        // Banker's rounding (round half to even) — native in rust_decimal
        let rounded = n.round_dp_with_strategy(
            precision.max(0) as u32,
            rust_decimal::RoundingStrategy::MidpointNearestEven,
        );
        Value::Number(rounded)
    }

    pub(in crate::evaluator) fn fn_power(&mut self, args: &[Expr]) -> Value {
        let base = match self.eval_arg(args, 0) {
            Value::Number(n) => n,
            Value::Null => return Value::Null,
            _ => return Value::Null,
        };
        let exp = match self.eval_arg(args, 1) {
            Value::Number(n) => n,
            Value::Null => return Value::Null,
            _ => return Value::Null,
        };
        // For non-negative integer exponents, use repeated multiplication
        if let Some(exp_u64) = exp.to_u64() {
            let mut result = Decimal::ONE;
            for _ in 0..exp_u64 {
                result = match result.checked_mul(base) {
                    Some(r) => r,
                    None => {
                        self.diag("power: overflow");
                        return Value::Null;
                    }
                };
            }
            return Value::Number(result);
        }
        // Negative or fractional exponent: fall back to f64 and convert back
        let base_f = base.to_f64().unwrap_or(0.0);
        let exp_f = exp.to_f64().unwrap_or(0.0);
        let result = base_f.powf(exp_f);
        if result.is_finite() {
            match Decimal::from_f64(result) {
                Some(d) => Value::Number(d),
                None => {
                    self.diag("power: overflow");
                    Value::Null
                }
            }
        } else {
            self.diag("power: overflow");
            Value::Null
        }
    }
}
