#![allow(clippy::missing_docs_in_private_items)]
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

use crate::ast::*;
use crate::error::Diagnostic;
use crate::iso_duration::{IsoDurationParse, parse_iso8601_duration};
use crate::types::*;

use super::super::core::Evaluator;
use super::super::util::{dec, parse_time_str};

impl<'a> Evaluator<'a> {
    // ── Date helpers ────────────────────────────────────────────

    pub(in crate::evaluator) fn fn_today(&self) -> Value {
        self.env
            .current_date()
            .map(Value::Date)
            .unwrap_or(Value::Null)
    }

    pub(in crate::evaluator) fn fn_now(&self) -> Value {
        self.env
            .current_datetime()
            .map(Value::Date)
            .unwrap_or(Value::Null)
    }

    pub(in crate::evaluator) fn fn_date_part(
        &mut self,
        args: &[Expr],
        f: fn(&Date) -> Decimal,
    ) -> Value {
        match self.eval_arg(args, 0) {
            Value::Date(d) => Value::Number(f(&d)),
            Value::String(s) => match self.coerce_string_to_date(&s) {
                Some(d) => Value::Number(f(&d)),
                None => Value::Null,
            },
            Value::Null => Value::Null,
            _ => Value::Null,
        }
    }

    pub(in crate::evaluator) fn fn_time_part(&mut self, args: &[Expr], idx: usize) -> Value {
        let s = match self.eval_arg(args, 0) {
            Value::String(s) => s,
            Value::Null => return Value::Null,
            _ => return Value::Null,
        };
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            self.diag("invalid time string");
            return Value::Null;
        }
        match parts.get(idx).and_then(|p| p.parse::<Decimal>().ok()) {
            Some(n) => Value::Number(n),
            None => Value::Null,
        }
    }

    pub(in crate::evaluator) fn fn_time(&mut self, args: &[Expr]) -> Value {
        let h = match self.eval_arg(args, 0) {
            Value::Number(n) => n.to_i64().unwrap_or(0),
            _ => return Value::Null,
        };
        let m = match self.eval_arg(args, 1) {
            Value::Number(n) => n.to_i64().unwrap_or(0),
            _ => return Value::Null,
        };
        let s = match self.eval_arg(args, 2) {
            Value::Number(n) => n.to_i64().unwrap_or(0),
            _ => return Value::Null,
        };
        Value::String(format!("{h:02}:{m:02}:{s:02}"))
    }

    pub(in crate::evaluator) fn fn_time_diff(&mut self, args: &[Expr]) -> Value {
        let t1 = match self.eval_arg(args, 0) {
            Value::String(s) => s,
            _ => return Value::Null,
        };
        let t2 = match self.eval_arg(args, 1) {
            Value::String(s) => s,
            _ => return Value::Null,
        };
        match (parse_time_str(&t1), parse_time_str(&t2)) {
            (Some((h1, m1, s1)), Some((h2, m2, s2))) => {
                Value::Number(dec((h1 * 3600 + m1 * 60 + s1) - (h2 * 3600 + m2 * 60 + s2)))
            }
            _ => {
                self.diag("timeDiff: invalid time strings");
                Value::Null
            }
        }
    }

    pub(in crate::evaluator) fn fn_duration(&mut self, args: &[Expr]) -> Value {
        let v = self.eval_arg(args, 0);
        match v {
            Value::String(s) => match parse_iso8601_duration(&s) {
                IsoDurationParse::Milliseconds(ms) => {
                    // Warn when the date component contains year (Y) or month (M before T).
                    // These use nominal lengths (365d, 30d), not calendar arithmetic.
                    // `W` is intentionally excluded: 1W = 7d exactly is not a nominal-length lie.
                    let has_year = s.contains('Y');
                    let has_date_month = {
                        // 'M' before any 'T' is a month designator; after 'T' it is minutes.
                        let t_pos = s.find('T').unwrap_or(s.len());
                        s[..t_pos].contains('M')
                    };
                    if has_year || has_date_month {
                        self.diagnostics.push(Diagnostic::warning(
                            "duration: year/month components use nominal lengths (365d, 30d); \
                             use dateAdd(\"years\"|\"months\", ...) for calendar arithmetic",
                        ));
                    }
                    Value::Number(dec(ms))
                }
                IsoDurationParse::Invalid => {
                    self.diag("duration: invalid ISO 8601 duration string");
                    Value::Null
                }
                IsoDurationParse::OutOfRange => {
                    self.diag("duration: duration exceeds representable range (milliseconds)");
                    Value::Null
                }
            },
            Value::Null => Value::Null,
            _ => {
                self.diag(format!("duration: expected string, got {}", v.type_name()));
                Value::Null
            }
        }
    }

    pub(in crate::evaluator) fn fn_date_diff(&mut self, args: &[Expr]) -> Value {
        let d1 = match self.eval_arg(args, 0) {
            Value::Date(d) => d,
            Value::String(s) => match self.coerce_string_to_date(&s) {
                Some(d) => d,
                None => return Value::Null,
            },
            _ => return Value::Null,
        };
        let d2 = match self.eval_arg(args, 1) {
            Value::Date(d) => d,
            Value::String(s) => match self.coerce_string_to_date(&s) {
                Some(d) => d,
                None => return Value::Null,
            },
            _ => return Value::Null,
        };
        let unit = match self.eval_arg(args, 2) {
            Value::String(s) => s,
            _ => return Value::Null,
        };
        let result = match unit.as_str() {
            "days" => d1.ordinal_days() - d2.ordinal_days(),
            "months" => {
                (d1.year() as i64 * 12 + d1.month() as i64)
                    - (d2.year() as i64 * 12 + d2.month() as i64)
            }
            "years" => d1.year() as i64 - d2.year() as i64,
            _ => {
                self.diag(format!("dateDiff: unknown unit '{unit}'"));
                return Value::Null;
            }
        };
        Value::Number(dec(result))
    }

    pub(in crate::evaluator) fn fn_date_add(&mut self, args: &[Expr]) -> Value {
        let d = match self.eval_arg(args, 0) {
            Value::Date(d) => d,
            Value::String(s) => match self.coerce_string_to_date(&s) {
                Some(d) => d,
                None => return Value::Null,
            },
            _ => return Value::Null,
        };
        let n = match self.eval_arg(args, 1) {
            Value::Number(n) => n.to_i64().unwrap_or(0),
            _ => return Value::Null,
        };
        let unit = match self.eval_arg(args, 2) {
            Value::String(s) => s,
            _ => return Value::Null,
        };
        match unit.as_str() {
            "days" => Value::Date(date_add_days(&d, n)),
            "months" => {
                let total = d.year() as i64 * 12 + (d.month() as i64 - 1) + n;
                let new_year = (total.div_euclid(12)) as i32;
                let new_month = (total.rem_euclid(12) + 1) as u32;
                let max_day = days_in_month(new_year, new_month);
                let new_day = d.day().min(max_day);
                Value::Date(Date::Date {
                    year: new_year,
                    month: new_month,
                    day: new_day,
                })
            }
            "years" => {
                let new_year = d.year() + n as i32;
                let max_day = days_in_month(new_year, d.month());
                let new_day = d.day().min(max_day);
                Value::Date(Date::Date {
                    year: new_year,
                    month: d.month(),
                    day: new_day,
                })
            }
            _ => {
                self.diag(format!("dateAdd: unknown unit '{unit}'"));
                Value::Null
            }
        }
    }

    /// Try to coerce an ISO date/datetime string to Date. Returns None on failure.
    pub(in crate::evaluator) fn coerce_string_to_date(&self, s: &str) -> Option<Date> {
        parse_date_literal(&format!("@{s}")).or_else(|| parse_datetime_literal(&format!("@{s}")))
    }
}
