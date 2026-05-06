#![allow(clippy::missing_docs_in_private_items)]
use intl_pluralrules::{PluralCategory, PluralRuleType, PluralRules};
use rust_decimal::Decimal;
use unic_langid::LanguageIdentifier;

use crate::ast::*;

// Decimal constants
pub(super) fn dec(n: i64) -> Decimal {
    Decimal::from(n)
}

/// Functions whose arguments are all eagerly evaluated and trace as
/// `FunctionCalled { args, result }` steps.
///
/// Lazy / short-circuiting functions (`if`,
/// `coalesce`, `countWhere`, `every`, `some`, `sumWhere`, `avgWhere`, etc.)
/// are deliberately absent — they explain themselves via [`TraceStep::IfBranch`]
/// or future dedicated step kinds.
pub(super) fn is_eager_traceable_function(name: &str) -> bool {
    matches!(
        name,
        "sum"
            | "count"
            | "avg"
            | "min"
            | "max"
            | "length"
            | "contains"
            | "startsWith"
            | "endsWith"
            | "substring"
            | "replace"
            | "upper"
            | "lower"
            | "trim"
            | "matches"
            | "format"
            | "round"
            | "floor"
            | "ceil"
            | "abs"
            | "power"
            | "empty"
            | "present"
            | "selected"
            | "isNumber"
            | "isString"
    )
}

/// Source-style symbol for a binary operator, used in trace output.
pub(super) fn binary_op_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::Concat => "&",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        BinaryOp::And => "and",
        BinaryOp::Or => "or",
    }
}

/// Render a `$foo.bar[2].baz` style path (sans leading `$`) for trace output.
pub(super) fn render_field_path(name: &Option<String>, path: &[PathSegment]) -> String {
    let mut out = String::new();
    if let Some(n) = name {
        out.push_str(n);
    }
    for seg in path {
        match seg {
            PathSegment::Dot(part) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(part);
            }
            PathSegment::Index(idx) => {
                out.push_str(&format!("[{idx}]"));
            }
            PathSegment::Wildcard => {
                out.push_str("[*]");
            }
        }
    }
    out
}

pub(super) fn extract_field_path(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::FieldRef { name, path } => {
            let mut segs = Vec::new();
            if let Some(n) = name {
                segs.push(n.clone());
            }
            for seg in path {
                match seg {
                    PathSegment::Dot(n) => segs.push(n.clone()),
                    PathSegment::Index(idx) => {
                        if let Some(last) = segs.last_mut() {
                            last.push_str(&format!("[{idx}]"));
                        }
                    }
                    PathSegment::Wildcard => {
                        if let Some(last) = segs.last_mut() {
                            last.push_str("[*]");
                        }
                    }
                }
            }
            segs
        }
        _ => Vec::new(),
    }
}

pub(super) fn parse_time_str(s: &str) -> Option<(i64, i64, i64)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

/// BCP 47 tag for plural rules: empty host locale behaves like `en` (prior hand-rolled default).
fn language_id_for_plural_rules(locale_str: &str) -> LanguageIdentifier {
    let s = locale_str.trim();
    if s.is_empty() {
        return "en".parse().expect("en is valid BCP 47");
    }
    s.parse()
        .unwrap_or_else(|_| "en".parse().expect("en is valid BCP 47"))
}

/// Cardinal plural category string for FEL, using CLDR data from `intl_pluralrules`.
///
/// Unknown or unsupported locales fall back to English cardinal rules.
pub(super) fn fel_cardinal_plural_category(locale_str: &str, n: i64) -> Option<&'static str> {
    let langid = language_id_for_plural_rules(locale_str);
    let rules = PluralRules::create(langid, PluralRuleType::CARDINAL).or_else(|_| {
        PluralRules::create(
            "en".parse::<LanguageIdentifier>()
                .expect("en is valid BCP 47"),
            PluralRuleType::CARDINAL,
        )
    });
    let pr = rules.ok()?;
    let cat = pr.select(n).ok()?;
    Some(plural_category_fel_name(cat))
}

fn plural_category_fel_name(cat: PluralCategory) -> &'static str {
    match cat {
        PluralCategory::ZERO => "zero",
        PluralCategory::ONE => "one",
        PluralCategory::TWO => "two",
        PluralCategory::FEW => "few",
        PluralCategory::MANY => "many",
        PluralCategory::OTHER => "other",
    }
}
