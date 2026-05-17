#![allow(clippy::missing_docs_in_private_items)]
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

use crate::ast::*;
use crate::types::*;

use super::super::core::Evaluator;

/// Maximum non-negative integer exponent for `power()` (defense-in-depth; integer path is O(log n)).
const MAX_POWER_EXPONENT: u64 = 10_000;

/// Raises `base` to `exp` using binary exponentiation (`exp` must fit the cap).
fn decimal_pow_int(mut base: Decimal, mut exp: u64) -> Option<Decimal> {
    let mut acc = Decimal::ONE;
    while exp > 0 {
        if exp & 1 != 0 {
            acc = acc.checked_mul(base)?;
        }
        exp >>= 1;
        if exp > 0 {
            base = base.checked_mul(base)?;
        }
    }
    Some(acc)
}

impl<'a> Evaluator<'a> {
    // ── Numeric helpers ─────────────────────────────────────────

    pub(in crate::evaluator) fn fn_num1(
        &mut self,
        args: &[Expr],
        fn_name: &str,
        f: fn(Decimal) -> Decimal,
    ) -> Value {
        match self.eval_arg(args, 0) {
            Value::Number(n) => Value::Number(f(n)),
            Value::Null => Value::Null,
            other => self.reject_expected_type(fn_name, "number", &other),
        }
    }

    pub(in crate::evaluator) fn fn_round(&mut self, args: &[Expr]) -> Value {
        let n = match self.eval_arg(args, 0) {
            Value::Number(n) => n,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type("round", "number", &other),
        };
        let precision = if args.len() > 1 {
            match self.eval_arg(args, 1) {
                Value::Number(p) => p.to_i32().unwrap_or(0),
                Value::Null => return Value::Null,
                other => return self.reject_expected_type("round", "number", &other),
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
        if !self.require_min_args(args, 2, "power") {
            return Value::Null;
        }
        let base = match self.eval_arg(args, 0) {
            Value::Number(n) => n,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type("power", "number", &other),
        };
        let exp = match self.eval_arg(args, 1) {
            Value::Number(n) => n,
            Value::Null => return Value::Null,
            other => return self.reject_expected_type("power", "number", &other),
        };
        // Non-negative integer exponents: O(log n) multiply + hard cap (MAX_EVAL_DEPTH does not apply).
        if let Some(exp_u64) = exp.to_u64() {
            if exp_u64 > MAX_POWER_EXPONENT {
                self.diag("power: exponent too large");
                return Value::Null;
            }
            return match decimal_pow_int(base, exp_u64) {
                Some(result) => Value::Number(result),
                None => {
                    self.diag("power: overflow");
                    Value::Null
                }
            };
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
