#![allow(clippy::missing_docs_in_private_items)]

use rust_decimal::Decimal;

use crate::ast::Expr;
use crate::types::{Date, Value, parse_date_literal, parse_datetime_literal};

use super::super::core::Evaluator;

const MONTHS_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

const MONTHS_EN_FULL: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

const MONTHS_FR: [&str; 12] = [
    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
    "déc.",
];

const MONTHS_FR_FULL: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

const WEEKDAYS_EN: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

const WEEKDAYS_EN_FULL: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

const WEEKDAYS_FR: [&str; 7] = ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."];

const WEEKDAYS_FR_FULL: [&str; 7] = [
    "dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DateFormatPattern {
    Short,
    Medium,
    Long,
    Full,
}

impl DateFormatPattern {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "short" => Some(Self::Short),
            "medium" => Some(Self::Medium),
            "long" => Some(Self::Long),
            "full" => Some(Self::Full),
            _ => None,
        }
    }

    /// The style name a Locale document keys its `formats.date` pattern by.
    fn name(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Medium => "medium",
            Self::Long => "long",
            Self::Full => "full",
        }
    }
}

impl<'a> Evaluator<'a> {
    /// `formatNumber(value, locale?)` - locale-aware decimal formatting; null in means null out.
    pub(in crate::evaluator) fn fn_format_number(&mut self, args: &[Expr]) -> Value {
        let value = self.eval_arg(args, 0);
        if matches!(value, Value::Null) {
            return Value::Null;
        }
        let n = match value {
            Value::Number(n) => n,
            _ => {
                self.diag("formatNumber: value must be a number".to_string());
                return Value::Null;
            }
        };
        let locale = self.resolve_optional_locale(args, 1);
        self.make_string(format_decimal_locale(n, locale.as_deref()))
    }

    /// `formatDate(value, pattern?, locale?)` - formats date values per locale.
    pub(in crate::evaluator) fn fn_format_date(&mut self, args: &[Expr]) -> Value {
        let value = self.eval_arg(args, 0);
        if matches!(value, Value::Null) {
            return Value::Null;
        }
        let date = match value {
            Value::Date(d) => d,
            Value::String(s) => match parse_format_date_input(&s) {
                Some(d) => d,
                None => {
                    self.diag(format!("formatDate: invalid date literal '{s}'"));
                    return Value::Null;
                }
            },
            _ => {
                self.diag("formatDate: value must be a date or ISO 8601 string".to_string());
                return Value::Null;
            }
        };

        let (pattern, locale) = self.parse_format_date_pattern_locale(args);
        // The Locale document may say how this locale writes a style; the style name stays the
        // authored contract, so a Definition never carries a pattern.
        let text = match self.env.date_format(pattern.name()) {
            Some(authored) => format_date_pattern(&date, authored, locale.as_deref()),
            None => format_date_locale(&date, pattern, locale.as_deref()),
        };
        self.make_string(text)
    }

    fn parse_format_date_pattern_locale(
        &mut self,
        args: &[Expr],
    ) -> (DateFormatPattern, Option<String>) {
        let mut pattern = DateFormatPattern::Medium;
        let mut locale = self.env.locale().map(|s| s.to_string());

        if args.len() >= 2 {
            match self.eval_arg(args, 1) {
                Value::String(s) => {
                    if let Some(p) = DateFormatPattern::parse(&s) {
                        pattern = p;
                        if args.len() >= 3 {
                            locale = self.resolve_optional_locale(args, 2);
                        }
                    } else {
                        locale = Some(s);
                    }
                }
                Value::Null => {
                    if args.len() >= 3 {
                        locale = self.resolve_optional_locale(args, 2);
                    }
                }
                _ => {
                    self.diag(
                        "formatDate: pattern must be short, medium, long, or full".to_string(),
                    );
                }
            }
        }

        (pattern, locale)
    }

    fn resolve_optional_locale(&mut self, args: &[Expr], idx: usize) -> Option<String> {
        if args.len() <= idx {
            return self.env.locale().map(|s| s.to_string());
        }
        match self.eval_arg(args, idx) {
            Value::String(s) => Some(s),
            Value::Null => self.env.locale().map(|s| s.to_string()),
            _ => {
                self.diag("locale argument must be a string".to_string());
                self.env.locale().map(|s| s.to_string())
            }
        }
    }
}

fn language_tag(locale: Option<&str>) -> &str {
    locale
        .and_then(|l| l.split('-').next())
        .filter(|s| !s.is_empty())
        .unwrap_or("en")
}

fn parse_format_date_input(s: &str) -> Option<Date> {
    parse_date_literal(s)
        .or_else(|| parse_datetime_literal(s))
        .or_else(|| parse_date_literal(&format!("@{s}")))
        .or_else(|| parse_datetime_literal(&format!("@{s}")))
}

fn format_decimal_locale(n: Decimal, locale: Option<&str>) -> String {
    let lang = language_tag(locale);
    let negative = n.is_sign_negative();
    let abs = n.abs().normalize();

    let plain = abs.to_string();
    let (int_digits, frac_part) = if let Some(dot) = plain.find('.') {
        (&plain[..dot], Some(&plain[dot + 1..]))
    } else {
        (plain.as_str(), None)
    };

    let grouped = format_integer_grouped(int_digits, lang);
    let sign = if negative { "-" } else { "" };

    match (lang, frac_part) {
        ("fr" | "de" | "es" | "it", Some(frac)) => format!("{sign}{grouped},{frac}"),
        (_, Some(frac)) => format!("{sign}{grouped}.{frac}"),
        _ => format!("{sign}{grouped}"),
    }
}

fn format_integer_grouped(int_digits: &str, lang: &str) -> String {
    let digits: Vec<char> = int_digits.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return "0".to_string();
    }
    let sep = match lang {
        "fr" | "de" | "es" | "it" => ' ',
        _ => ',',
    };
    let mut out = String::new();
    for (i, ch) in digits.iter().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(sep);
        }
        out.push(*ch);
    }
    out.chars().rev().collect()
}

fn month_name_abbrev(lang: &str, month: u32) -> &'static str {
    let idx = (month as usize).saturating_sub(1);
    match lang {
        "fr" => MONTHS_FR.get(idx).copied().unwrap_or("?"),
        _ => MONTHS_EN.get(idx).copied().unwrap_or("?"),
    }
}

fn month_name_full(lang: &str, month: u32) -> &'static str {
    let idx = (month as usize).saturating_sub(1);
    match lang {
        "fr" => MONTHS_FR_FULL.get(idx).copied().unwrap_or("?"),
        _ => MONTHS_EN_FULL.get(idx).copied().unwrap_or("?"),
    }
}

/// Day of week, Sunday = 0. 1970-01-01 was a Thursday.
fn weekday_index(date: &Date) -> usize {
    (date.ordinal_days() + 4).rem_euclid(7) as usize
}

fn weekday_name_abbrev(lang: &str, weekday: usize) -> &'static str {
    match lang {
        "fr" => WEEKDAYS_FR[weekday],
        _ => WEEKDAYS_EN[weekday],
    }
}

fn weekday_name_full(lang: &str, weekday: usize) -> &'static str {
    match lang {
        "fr" => WEEKDAYS_FR_FULL[weekday],
        _ => WEEKDAYS_EN_FULL[weekday],
    }
}

fn format_date_locale(date: &Date, pattern: DateFormatPattern, locale: Option<&str>) -> String {
    let lang = language_tag(locale);
    let (year, month, day) = date.to_naive_date();
    let yy = year.rem_euclid(100);

    match pattern {
        DateFormatPattern::Short => match lang {
            "fr" => format!("{day:02}/{month:02}/{yy:02}"),
            _ => format!("{month}/{day}/{yy:02}"),
        },
        DateFormatPattern::Medium => {
            let month_name = month_name_abbrev(lang, month);
            match lang {
                "fr" => format!("{day} {month_name} {year}"),
                _ => format!("{month_name} {day}, {year}"),
            }
        }
        DateFormatPattern::Long => {
            let month_name = month_name_full(lang, month);
            match lang {
                "fr" => format!("{day} {month_name} {year}"),
                _ => format!("{month_name} {day}, {year}"),
            }
        }
        // CLDR `full` is `long` plus the weekday.
        DateFormatPattern::Full => {
            let month_name = month_name_full(lang, month);
            let weekday = weekday_name_full(lang, weekday_index(date));
            match lang {
                "fr" => format!("{weekday} {day} {month_name} {year}"),
                _ => format!("{weekday}, {month_name} {day}, {year}"),
            }
        }
    }
}

/// Renders `date` with an authored pattern (Locale spec §2.4): the ICU letters `yyyy`, `yy`,
/// `MMMM`, `MMM`, `MM`, `M`, `dd`, `d`, `EEEE`, `EEE` are fields; every other character is literal.
fn format_date_pattern(date: &Date, pattern: &str, locale: Option<&str>) -> String {
    let lang = language_tag(locale);
    let (year, month, day) = date.to_naive_date();
    let weekday = weekday_index(date);
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let run = chars[i..].iter().take_while(|&&x| x == c).count();
        let (text, used) = match (c, run) {
            ('y', n) if n >= 4 => (format!("{year}"), 4),
            ('y', 2) => (format!("{:02}", year.rem_euclid(100)), 2),
            ('M', n) if n >= 4 => (month_name_full(lang, month).to_string(), 4),
            ('M', 3) => (month_name_abbrev(lang, month).to_string(), 3),
            ('M', 2) => (format!("{month:02}"), 2),
            ('M', 1) => (format!("{month}"), 1),
            ('d', n) if n >= 2 => (format!("{day:02}"), 2),
            ('d', 1) => (format!("{day}"), 1),
            ('E', n) if n >= 4 => (weekday_name_full(lang, weekday).to_string(), 4),
            ('E', n) if n >= 1 => (weekday_name_abbrev(lang, weekday).to_string(), n.min(3)),
            _ => (c.to_string(), 1),
        };
        out.push_str(&text);
        i += used;
    }
    out
}
