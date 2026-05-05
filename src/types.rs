//! FEL runtime value types with base-10 decimal arithmetic.
use rust_decimal::Decimal;
use std::fmt;

/// Runtime value for FEL evaluation (mirrors JSON + dates + money).
#[derive(Debug, Clone)]
pub enum Value {
    /// Null / absent value.
    Null,
    /// Boolean (`true` or `false`).
    Boolean(bool),
    /// Numeric value (high-precision decimal, rust_decimal 96-bit mantissa).
    Number(Decimal),
    /// UTF-8 string value.
    String(String),
    /// Calendar date or date-time value.
    Date(Date),
    /// Ordered list of values.
    Array(Vec<Value>),
    /// Ordered key-value map.
    Object(Vec<(String, Value)>),
    /// Monetary amount with ISO currency code.
    Money(Money),
}

/// Calendar date or date-time (no timezone model; used by date functions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Date {
    /// Calendar date (year, month, day).
    Date {
        /// Gregorian year.
        year: i32,
        /// Month 1–12.
        month: u32,
        /// Day of month 1–31.
        day: u32,
    },
    /// Date with time of day (no timezone).
    DateTime {
        /// Gregorian year.
        year: i32,
        /// Month 1–12.
        month: u32,
        /// Day of month 1–31.
        day: u32,
        /// Hour 0–23.
        hour: u32,
        /// Minute 0–59.
        minute: u32,
        /// Second 0–59.
        second: u32,
    },
}

/// Monetary value with ISO currency code.
#[derive(Debug, Clone)]
pub struct Money {
    /// Decimal amount (base-10).
    pub amount: Decimal,
    /// ISO 4217 currency code (e.g. `USD`).
    pub currency: String,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Date(a), Value::Date(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Money(a), Value::Money(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialEq for Money {
    fn eq(&self, other: &Self) -> bool {
        self.currency == other.currency && self.amount == other.amount
    }
}

impl Value {
    /// Lowercase FEL type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Boolean(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Date(_) => "date",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Money(_) => "money",
        }
    }

    /// True only for [`Value::Null`].
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Loose truth test (not FEL `and`/`or` typing — used by some builtins).
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Boolean(b) => *b,
            Value::Number(n) => !n.is_zero(),
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            _ => true,
        }
    }

    /// Extract number or `None`.
    pub fn as_number(&self) -> Option<Decimal> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Borrow string or `None`.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Extract boolean or `None`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Borrow date/datetime or `None`.
    pub fn as_date(&self) -> Option<&Date> {
        match self {
            Value::Date(d) => Some(d),
            _ => None,
        }
    }

    /// Borrow array or `None`.
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Borrow money or `None`.
    pub fn as_money(&self) -> Option<&Money> {
        match self {
            Value::Money(m) => Some(m),
            _ => None,
        }
    }
}

impl Date {
    /// Calendar year component.
    pub fn year(&self) -> i32 {
        match self {
            Date::Date { year, .. } | Date::DateTime { year, .. } => *year,
        }
    }

    /// Month 1–12.
    pub fn month(&self) -> u32 {
        match self {
            Date::Date { month, .. } | Date::DateTime { month, .. } => *month,
        }
    }

    /// Day of month.
    pub fn day(&self) -> u32 {
        match self {
            Date::Date { day, .. } | Date::DateTime { day, .. } => *day,
        }
    }

    /// `(year, month, day)` tuple.
    pub fn to_naive_date(&self) -> (i32, u32, u32) {
        (self.year(), self.month(), self.day())
    }

    /// Days since epoch (1970-01-01) for ordering.
    pub fn ordinal_days(&self) -> i64 {
        days_from_civil(self.year(), self.month(), self.day())
    }

    /// Full ordinal including time (seconds from epoch) for DateTime ordering.
    pub fn ordinal(&self) -> i64 {
        match self {
            Date::Date { .. } => self.ordinal_days() * 86400,
            Date::DateTime {
                hour,
                minute,
                second,
                ..
            } => {
                self.ordinal_days() * 86400
                    + *hour as i64 * 3600
                    + *minute as i64 * 60
                    + *second as i64
            }
        }
    }

    /// `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS` (no timezone suffix).
    pub fn format_iso(&self) -> String {
        match self {
            Date::Date { year, month, day } => {
                format!("{year:04}-{month:02}-{day:02}")
            }
            Date::DateTime {
                year,
                month,
                day,
                hour,
                minute,
                second,
            } => {
                format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")
            }
        }
    }
}

/// Days from civil date to days since the FEL epoch (1970-01-01) (algorithm from Howard Hinnant).
pub fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let y = if month <= 2 {
        year as i64 - 1
    } else {
        year as i64
    };
    let m = if month <= 2 {
        month as i64 + 9
    } else {
        month as i64 - 3
    };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * m as u64 + 2) / 5 + day as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

/// Gregorian days in `month` for `year` (validates `month` in debug only).
pub fn days_in_month(year: i32, month: u32) -> u32 {
    debug_assert!(
        (1..=12).contains(&month),
        "days_in_month called with invalid month: {month}"
    );
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Parse "@YYYY-MM-DD" into Date.
pub fn parse_date_literal(s: &str) -> Option<Date> {
    let s = s.strip_prefix('@')?;
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    Some(Date::Date { year, month, day })
}

/// Parse "@YYYY-MM-DDTHH:MM:SS..." into Date.
pub fn parse_datetime_literal(s: &str) -> Option<Date> {
    let s = s.strip_prefix('@')?;
    // Strip timezone suffix
    let s = s.trim_end_matches('Z');
    let s = if s.len() > 19 { &s[..19] } else { s };
    let (date_part, time_part) = s.split_once('T')?;
    let dp: Vec<&str> = date_part.split('-').collect();
    let tp: Vec<&str> = time_part.split(':').collect();
    if dp.len() != 3 || tp.len() != 3 {
        return None;
    }
    let year: i32 = dp[0].parse().ok()?;
    let month: u32 = dp[1].parse().ok()?;
    let day: u32 = dp[2].parse().ok()?;
    let hour: u32 = tp[0].parse().ok()?;
    let minute: u32 = tp[1].parse().ok()?;
    let second: u32 = tp[2].parse().ok()?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    if hour >= 24 || minute >= 60 || second >= 60 {
        return None;
    }
    Some(Date::DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}

/// Add days to a date.
pub fn date_add_days(d: &Date, n: i64) -> Date {
    let total_days = d.ordinal_days() + n;
    civil_from_days(total_days)
}

/// Convert days since the FEL epoch (1970-01-01) back to a civil [`Date`] (date-only).
pub fn civil_from_days(z: i64) -> Date {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    Date::Date {
        year: y as i32,
        month: m as u32,
        day: d as u32,
    }
}


/// Format a Decimal: strip trailing zeros, show as integer when possible.
pub fn format_number(n: Decimal) -> String {
    n.normalize().to_string()
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Number(n) => write!(f, "{}", format_number(*n)),
            Value::String(s) => write!(f, "{s}"),
            Value::Date(d) => write!(f, "{}", d.format_iso()),
            Value::Array(a) => {
                write!(f, "[")?;
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::Object(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
            Value::Money(m) => write!(f, "{} {}", format_number(m.amount), m.currency),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_docs_in_private_items)]
    use super::*;

    /// Spec: core/spec.md §3.5.4, fel-grammar.md §3.6 —
    /// days_in_month must return correct values for all 12 months,
    /// including Feb in leap and non-leap years, and century rules.
    #[test]
    fn days_in_month_all_months() {
        // 31-day months
        assert_eq!(days_in_month(2024, 1), 31);
        assert_eq!(days_in_month(2024, 3), 31);
        assert_eq!(days_in_month(2024, 5), 31);
        assert_eq!(days_in_month(2024, 7), 31);
        assert_eq!(days_in_month(2024, 8), 31);
        assert_eq!(days_in_month(2024, 10), 31);
        assert_eq!(days_in_month(2024, 12), 31);
        // 30-day months
        assert_eq!(days_in_month(2024, 4), 30);
        assert_eq!(days_in_month(2024, 6), 30);
        assert_eq!(days_in_month(2024, 9), 30);
        assert_eq!(days_in_month(2024, 11), 30);
    }

    /// Spec: core/spec.md §3.5.4 — Feb in leap year.
    #[test]
    fn days_in_month_feb_leap_year() {
        assert_eq!(days_in_month(2024, 2), 29, "2024 is a leap year");
    }

    /// Spec: core/spec.md §3.5.4 — Feb in non-leap year.
    #[test]
    fn days_in_month_feb_non_leap_year() {
        assert_eq!(days_in_month(2023, 2), 28, "2023 is not a leap year");
    }

    /// Spec: core/spec.md §3.5.4 — Century leap year rules:
    /// 2000 is a leap year (divisible by 400), 1900 is not (divisible by 100 but not 400).
    #[test]
    fn days_in_month_century_leap_rules() {
        assert_eq!(
            days_in_month(2000, 2),
            29,
            "2000 is divisible by 400 → leap"
        );
        assert_eq!(
            days_in_month(1900, 2),
            28,
            "1900 is divisible by 100 but not 400 → not leap"
        );
    }

    /// Spec: fel-grammar.md §3.6, core/spec.md §3.4.3 —
    /// parse_date_literal must reject invalid month (13).
    #[test]
    fn parse_date_literal_invalid_month() {
        assert!(
            parse_date_literal("@2024-13-01").is_none(),
            "month 13 is invalid"
        );
    }

    /// Spec: fel-grammar.md §3.6, core/spec.md §3.4.3 —
    /// parse_date_literal must reject invalid day (32).
    #[test]
    fn parse_date_literal_invalid_day() {
        assert!(
            parse_date_literal("@2024-01-32").is_none(),
            "day 32 is invalid"
        );
    }

    /// Spec: fel-grammar.md §3.6, core/spec.md §3.4.3 —
    /// parse_date_literal must reject Feb 29 on a non-leap year.
    #[test]
    fn parse_date_literal_feb29_non_leap() {
        assert!(
            parse_date_literal("@2023-02-29").is_none(),
            "2023 is not a leap year"
        );
        assert!(
            parse_date_literal("@2024-02-29").is_some(),
            "2024 is a leap year"
        );
    }

    /// Spec: core/spec.md §3.4.3 — valid dates must parse successfully.
    #[test]
    fn parse_date_literal_valid() {
        let d = parse_date_literal("@2024-06-15").unwrap();
        assert_eq!(
            d,
            Date::Date {
                year: 2024,
                month: 6,
                day: 15
            }
        );
    }

    /// Spec: core/spec.md §3.5.4 — round-trip: date → ordinal days → date
    /// must produce the original date (identity property).
    #[test]
    fn civil_from_days_round_trip() {
        let test_dates = [
            (2024, 1, 1),
            (2024, 2, 29), // leap day
            (2024, 12, 31),
            (2000, 1, 1),  // century leap
            (1900, 3, 1),  // century non-leap
            (1970, 1, 1),  // unix epoch
            (2026, 3, 19), // today
        ];
        for (year, month, day) in test_dates {
            let date = Date::Date { year, month, day };
            let days = date.ordinal_days();
            let reconstructed = civil_from_days(days);
            assert_eq!(
                reconstructed, date,
                "round-trip failed for {year}-{month:02}-{day:02}"
            );
        }
    }
}
