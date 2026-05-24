//! Phase 3b Tier-2 + Convenience proptest — `lexer::{is_valid_fel_identifier,
//! sanitize_fel_identifier}` and `types::{parse_date_literal,
//! parse_datetime_literal, value_size_estimate}`.
//!
//! Properties:
//!
//! 1. **`sanitize` is `is_valid`-conformant** — for any input, the sanitized
//!    output is always a valid FEL identifier.
//! 2. **`sanitize` is idempotent** — `sanitize(sanitize(s)) == sanitize(s)`.
//! 3. **`is_valid` consistent with sanitize fixed-point** — every valid
//!    identifier is a fixed point of `sanitize`.
//! 4. **`parse_date_literal` round-trip** — canonical `@YYYY-MM-DD` strings
//!    parse and re-format to the same canonical form; out-of-range or
//!    malformed inputs return None.
//! 5. **`parse_datetime_literal` round-trip** — canonical
//!    `@YYYY-MM-DDTHH:MM:SS` strings (with optional `Z` / `+HH:MM` /
//!    `-HH:MM` timezone suffix) parse, and the parsed value re-formats to
//!    the canonical 19-char `YYYY-MM-DDTHH:MM:SS` form (parser truncates
//!    timezone; `format_iso` does not re-emit it).
//! 6. **`value_size_estimate` monotonicity** — composition adds at least
//!    the sum: `size(Array([a,b])) >= size(a) + size(b)`. Scalar invariant:
//!    `size(Number(_)) > 0`.
#![cfg(feature = "proptest-strategies")]
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{
    Date, Value, is_valid_fel_identifier, parse_date_literal, parse_datetime_literal,
    sanitize_fel_identifier, value_size_estimate,
};
use proptest::prelude::*;
use rust_decimal::Decimal;

// ---- identifier strategies ----------------------------------------------

/// Arbitrary ASCII-ish string covering punctuation, digits, letters, and
/// underscores — exercises every branch of `sanitize_fel_identifier`
/// (filter, leading-digit guard, keyword suffix, all-empty fallback).
fn arb_ascii_input() -> impl Strategy<Value = String> {
    r"[a-zA-Z0-9_\-\. !@#\$%\^&\*\(\)\+]{0,16}".prop_map(String::from)
}

/// Arbitrary already-valid identifier — matches the regex
/// `[a-zA-Z_][a-zA-Z0-9_]*` and is rejected if it lands on a reserved
/// keyword (true / false / null / let / in / if / then / else / and / or /
/// not). We post-filter against `is_valid_fel_identifier` to keep the
/// property's antecedent strictly true.
fn arb_valid_identifier() -> impl Strategy<Value = String> {
    r"[a-zA-Z_][a-zA-Z0-9_]{0,15}"
        .prop_map(String::from)
        .prop_filter("must be a valid FEL identifier", |s| {
            is_valid_fel_identifier(s)
        })
}

// ---- date strategies -----------------------------------------------------

/// Tuple of (year, month, day) that always corresponds to a valid civil
/// date. Year range chosen to cover both pre- and post-epoch and remain
/// comfortably inside the parser's i32 range. Day is computed from month
/// + leap-year rule so Feb 29 only appears in leap years.
fn arb_valid_ymd() -> impl Strategy<Value = (i32, u32, u32)> {
    (1900_i32..=2200_i32, 1_u32..=12_u32).prop_flat_map(|(year, month)| {
        let max_day = days_in_month_oracle(year, month);
        (Just(year), Just(month), 1_u32..=max_day)
    })
}

fn arb_valid_hms() -> impl Strategy<Value = (u32, u32, u32)> {
    (0_u32..24, 0_u32..60, 0_u32..60)
}

/// Reference oracle for max-day-in-month — mirrors the production
/// `days_in_month` (which is `pub(crate)`, so the test can't see it).
fn days_in_month_oracle(year: i32, month: u32) -> u32 {
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

/// Canonical `@YYYY-MM-DD` literal from a known-valid YMD tuple.
fn format_canonical_date(year: i32, month: u32, day: u32) -> String {
    format!("@{year:04}-{month:02}-{day:02}")
}

/// Canonical `@YYYY-MM-DDTHH:MM:SS` literal from known-valid YMD+HMS.
fn format_canonical_datetime(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> String {
    format!("@{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")
}

/// Strategy: timezone suffix — empty, `Z`, `+HH:MM`, or `-HH:MM`.
fn arb_tz_suffix() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        Just("Z".to_string()),
        (0_u32..24, 0_u32..60).prop_map(|(h, m)| format!("+{h:02}:{m:02}")),
        (0_u32..24, 0_u32..60).prop_map(|(h, m)| format!("-{h:02}:{m:02}")),
    ]
}

// ---- Value strategies ----------------------------------------------------

/// Small leaf strategy for `Value` — covers every variant we care about
/// for the size-estimate monotonicity property. Decimal payloads stay
/// small to keep proptest shrinking responsive.
fn arb_leaf_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Boolean),
        (-1_000_000_i64..=1_000_000_i64).prop_map(|n| Value::Number(Decimal::from(n))),
        "[a-zA-Z0-9 ]{0,16}".prop_map(|s: String| Value::String(s)),
    ]
}

// ---- properties ----------------------------------------------------------

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        ..Default::default()
    })]

    /// Property #1 — `sanitize` is `is_valid`-conformant: the sanitized
    /// output is always a valid FEL identifier, no matter what mess we
    /// throw at it.
    #[test]
    fn sanitize_output_is_always_valid(input in arb_ascii_input()) {
        let sanitized = sanitize_fel_identifier(&input);
        prop_assert!(
            is_valid_fel_identifier(&sanitized),
            "sanitize({:?}) = {:?} which is NOT a valid FEL identifier",
            input,
            sanitized
        );
    }

    /// Property #2 — `sanitize` is idempotent: a second pass is a no-op.
    /// Equivalent to "sanitize output is a fixed point of sanitize".
    #[test]
    fn sanitize_is_idempotent(input in arb_ascii_input()) {
        let once = sanitize_fel_identifier(&input);
        let twice = sanitize_fel_identifier(&once);
        prop_assert_eq!(&once, &twice);
    }

    /// Property #3 — `is_valid` consistent with sanitize fixed-point:
    /// if a string is already valid, sanitize must leave it untouched.
    #[test]
    fn valid_identifiers_are_sanitize_fixed_points(s in arb_valid_identifier()) {
        let sanitized = sanitize_fel_identifier(&s);
        prop_assert_eq!(
            &s,
            &sanitized,
            "valid identifier {:?} mutated by sanitize → {:?}",
            s,
            sanitized
        );
    }

    /// Property #4a — `parse_date_literal` round-trip: every canonical
    /// `@YYYY-MM-DD` string with valid Y/M/D parses, and the parsed value
    /// re-formats (via `format_iso`, prefixed with `@`) to the same input.
    #[test]
    fn parse_date_literal_round_trip(ymd in arb_valid_ymd()) {
        let (year, month, day) = ymd;
        let input = format_canonical_date(year, month, day);
        let parsed = parse_date_literal(&input)
            .unwrap_or_else(|| panic!("expected Some for canonical date {input:?}"));
        // Structural equality
        prop_assert_eq!(&parsed, &Date::Date { year, month, day });
        // Round-trip via format_iso (no @ prefix in format_iso output).
        let reformatted = format!("@{}", parsed.format_iso());
        prop_assert_eq!(&input, &reformatted);
    }

    /// Property #4b — `parse_date_literal` rejects out-of-range months
    /// (anything outside 1..=12).
    #[test]
    fn parse_date_literal_rejects_bad_month(
        year in 1900_i32..=2200_i32,
        month in 13_u32..=99_u32,
        day in 1_u32..=28_u32,
    ) {
        let input = format!("@{year:04}-{month:02}-{day:02}");
        prop_assert!(
            parse_date_literal(&input).is_none(),
            "month {} should be rejected for {:?}",
            month,
            input
        );
    }

    /// Property #4c — `parse_date_literal` rejects out-of-range days
    /// (always rejects days strictly greater than `days_in_month`).
    #[test]
    fn parse_date_literal_rejects_bad_day(ymd in arb_valid_ymd(), extra in 1_u32..=40_u32) {
        let (year, month, _valid_day) = ymd;
        let max = days_in_month_oracle(year, month);
        let bad_day = max + extra;
        let input = format!("@{year:04}-{month:02}-{bad_day:02}");
        prop_assert!(
            parse_date_literal(&input).is_none(),
            "day {} > max {} for {:04}-{:02} should be rejected",
            bad_day,
            max,
            year,
            month
        );
    }

    /// Property #4d — `parse_date_literal` rejects malformed shapes
    /// (missing `@`, wrong separator count, non-numeric components).
    #[test]
    fn parse_date_literal_rejects_malformed(
        ymd in arb_valid_ymd(),
    ) {
        let (year, month, day) = ymd;
        // Missing @
        let no_at = format!("{year:04}-{month:02}-{day:02}");
        prop_assert!(parse_date_literal(&no_at).is_none());
        // Slashes instead of dashes
        let slashes = format!("@{year:04}/{month:02}/{day:02}");
        prop_assert!(parse_date_literal(&slashes).is_none());
        // Wrong segment count
        let two_segs = format!("@{year:04}-{month:02}");
        prop_assert!(parse_date_literal(&two_segs).is_none());
    }

    /// Property #5a — `parse_datetime_literal` round-trip without
    /// timezone: canonical `@YYYY-MM-DDTHH:MM:SS` parses and re-formats
    /// (via `@` + `format_iso`) to the same input.
    #[test]
    fn parse_datetime_literal_round_trip_no_tz(
        ymd in arb_valid_ymd(),
        hms in arb_valid_hms(),
    ) {
        let (year, month, day) = ymd;
        let (hour, minute, second) = hms;
        let input = format_canonical_datetime(year, month, day, hour, minute, second);
        let parsed = parse_datetime_literal(&input)
            .unwrap_or_else(|| panic!("expected Some for canonical datetime {input:?}"));
        prop_assert_eq!(
            &parsed,
            &Date::DateTime { year, month, day, hour, minute, second }
        );
        let reformatted = format!("@{}", parsed.format_iso());
        prop_assert_eq!(&input, &reformatted);
    }

    /// Property #5b — `parse_datetime_literal` with timezone suffix
    /// (`Z`, `+HH:MM`, `-HH:MM`) parses to the same Date value as the
    /// untagged form. The parser intentionally truncates timezone info
    /// (it strips a trailing `Z` and otherwise keeps only the first
    /// 19 chars after `@`), so the round-trip target is the canonical
    /// 19-char form, not the original input.
    #[test]
    fn parse_datetime_literal_strips_timezone(
        ymd in arb_valid_ymd(),
        hms in arb_valid_hms(),
        tz in arb_tz_suffix(),
    ) {
        let (year, month, day) = ymd;
        let (hour, minute, second) = hms;
        let canonical = format_canonical_datetime(year, month, day, hour, minute, second);
        let with_tz = format!("{canonical}{tz}");
        let parsed = parse_datetime_literal(&with_tz)
            .unwrap_or_else(|| panic!("expected Some for datetime+tz {with_tz:?}"));
        prop_assert_eq!(
            &parsed,
            &Date::DateTime { year, month, day, hour, minute, second }
        );
        // format_iso never re-emits a timezone — the round-trip target
        // is the timezone-free canonical form.
        let reformatted = format!("@{}", parsed.format_iso());
        prop_assert_eq!(&canonical, &reformatted);
    }

    /// Property #5c — `parse_datetime_literal` rejects out-of-range
    /// hour / minute / second components.
    #[test]
    fn parse_datetime_literal_rejects_bad_time(
        ymd in arb_valid_ymd(),
        bad_hour in 24_u32..=99_u32,
    ) {
        let (year, month, day) = ymd;
        let input = format!("@{year:04}-{month:02}-{day:02}T{bad_hour:02}:00:00");
        prop_assert!(
            parse_datetime_literal(&input).is_none(),
            "hour {} should be rejected for {:?}",
            bad_hour,
            input
        );
    }

    /// Property #6a — `value_size_estimate` is at-least-additive over
    /// `Value::Array` composition: the size of an array containing
    /// (a, b) is ≥ size(a) + size(b). (Production formula adds
    /// `arr.len() * 16` per-element overhead on top, so this holds
    /// strictly with a positive gap.)
    #[test]
    fn value_size_array_is_at_least_additive(
        a in arb_leaf_value(),
        b in arb_leaf_value(),
    ) {
        let sa = value_size_estimate(&a);
        let sb = value_size_estimate(&b);
        let sab = value_size_estimate(&Value::Array(vec![a, b]));
        prop_assert!(
            sab >= sa + sb,
            "size(Array([a,b])) = {} should be ≥ size(a) + size(b) = {} + {} = {}",
            sab,
            sa,
            sb,
            sa + sb
        );
    }

    /// Property #6b — scalar invariant: every `Value::Number` has a
    /// strictly positive size estimate (matches the production constant
    /// of 16 bytes for the Decimal payload).
    #[test]
    fn value_size_number_is_positive(n in -1_000_000_000_i64..=1_000_000_000_i64) {
        let v = Value::Number(Decimal::from(n));
        let size = value_size_estimate(&v);
        prop_assert!(size > 0, "size(Number({})) = {}, expected > 0", n, size);
    }
}
