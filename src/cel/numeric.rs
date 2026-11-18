//! Checks when literal numeric operations have a reversible native spelling.
// Rust guideline compliant 2026-02-21

use crate::Expr;
use crate::ast::UnaryOp;

/// Extract an in-range signed integer literal.
pub(super) fn integer(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Number(n) => n.normalize().to_string().parse().ok(),
        Expr::UnaryOp {
            op: UnaryOp::Neg,
            operand,
            ..
        } => {
            if let Expr::Number(n) = operand.as_ref() {
                format!("-{}", n.normalize()).parse().ok()
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Binary64 has 53 significand bits. A decimal fraction is exactly representable
/// only when cancelling every factor of five leaves a binary significand that fits.
pub(super) fn shared_fraction(expr: &Expr) -> bool {
    let Expr::Number(n) = expr else { return false };
    let n = n.normalize();
    if n.scale() == 0 {
        return false;
    }
    let magnitude = n.mantissa().unsigned_abs();
    let fives = 5_u128.pow(n.scale());
    if magnitude % fives != 0 {
        return false;
    }
    let binary = magnitude / fives;
    binary.ilog2() + 1 - binary.trailing_zeros() <= 53
}

/// Converts the binary significand exactly when FEL can hold the full decimal.
pub(super) fn fractional_double(value: f64) -> Option<Expr> {
    if !value.is_normal() || value.fract() == 0.0 {
        return None;
    }
    let bits = value.abs().to_bits();
    let significand = (1 << 52) | (bits & ((1 << 52) - 1));
    let zeros = significand.trailing_zeros();
    let exponent = ((bits >> 52) as i32) - 1023 - 52 + zeros as i32;
    let scale = u32::try_from(-exponent).ok()?;
    if !(1..=28).contains(&scale) {
        return None;
    }
    let coefficient = u128::from(significand >> zeros) * 5_u128.pow(scale);
    if coefficient >= 1_u128 << 96 {
        return None;
    }
    let width = scale as usize + 1;
    let mut literal = format!("{coefficient:0width$}");
    literal.insert(literal.len() - scale as usize, '.');
    if value.is_sign_negative() {
        literal.insert(0, '-');
    }
    crate::parse(&literal).ok()
}

/// A checked integer quotient truncates toward zero, choosing floor or ceil
/// from the operand signs. FEL Decimal has enough precision to locate that integer
/// boundary for nonzero signed 64-bit operands.
pub(super) fn truncation(left: &Expr, right: &Expr) -> Option<&'static str> {
    let (n, d) = (integer(left)?, integer(right)?);
    n.checked_div(d)?;
    Some(if (n < 0) != (d < 0) { "ceil" } else { "floor" })
}

/// Checks that both literal divisions represent the exact rational result.
pub(super) fn shared_quotient(left: &Expr, right: &Expr) -> bool {
    let Some((n, d)) = integer(left).zip(integer(right)) else {
        return false;
    };
    let n = n.unsigned_abs();
    let d = d.unsigned_abs();
    if d == 0 || n > (1 << 53) || d > (1 << 53) {
        return false;
    }
    // A double with a negative zero result has no corresponding FEL decimal.
    if n == 0 {
        return integer(right).is_some_and(|value| value > 0);
    }
    let (mut a, mut b) = (n, d);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    let denominator = d / a;
    let scale = denominator.trailing_zeros();
    // rust_decimal uses a 96-bit coefficient and at most 28 fractional digits.
    denominator.is_power_of_two()
        && scale <= 28
        && u128::from(n / a)
            .checked_mul(5_u128.pow(scale))
            .is_some_and(|v| v < (1_u128 << 96))
}

/// Return an exactly representable integer operand of CEL double division.
pub(super) fn whole_double(value: f64) -> Option<Expr> {
    if value.abs() > ((1_u64 << 53) as f64)
        || value.fract() != 0.0
        || (value == 0.0 && value.is_sign_negative())
    {
        return None;
    }
    crate::parse(&(value as i64).to_string()).ok()
}
