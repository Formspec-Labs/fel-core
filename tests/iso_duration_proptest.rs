//! Property tests for ISO 8601 duration parsing.
//!
//! Phase 3b Tier-2 closure for `iso_duration::{IsoDurationParse,
//! parse_iso8601_duration, parse_iso8601_duration_ms}` per the Phase 3 CI gate
//! design doc (`thoughts/2026-05-23-phase-3-ci-gate-design.md`, §Gap item 6):
//! a single proptest on round-trip closes both functions.
//!
//! No public formatter exists on `IsoDurationParse`, so round-trip uses two
//! complementary strategies:
//!   1. **Synthesized canonical inputs.** Build `PT{seconds}S` (and fractional
//!      variants) from bounded `i64` ms, parse, assert milliseconds match.
//!   2. **Two-pass parser consistency.** Build a multi-component canonical
//!      string `P{y}Y{mo}M{w}W{d}DT{h}H{m}M{s}S`, parse to milliseconds,
//!      re-emit a canonical `PT{whole}.{frac}S` from the result, parse again,
//!      assert the second parse equals the first.
//!
//! `parse_iso8601_duration_ms` consistency is checked on the same inputs:
//! `Some(ms)` iff `parse_iso8601_duration` returns `Milliseconds(ms)`.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{IsoDurationParse, parse_iso8601_duration, parse_iso8601_duration_ms};
use proptest::prelude::*;

const MS_PER_SECOND: i64 = 1000;
const MS_PER_MINUTE: i64 = 60 * MS_PER_SECOND;
const MS_PER_HOUR: i64 = 60 * MS_PER_MINUTE;
const MS_PER_DAY: i64 = 24 * MS_PER_HOUR;
const MS_PER_WEEK: i64 = 7 * MS_PER_DAY;
const MS_PER_MONTH: i64 = 30 * MS_PER_DAY;
const MS_PER_YEAR: i64 = 365 * MS_PER_DAY;

/// ~10 000 years in milliseconds — well within `i64::MAX` (~292 M years).
const MS_RANGE: i64 = 10_000 * 365 * 24 * 3_600 * 1_000;

/// Reformat a millisecond count as a canonical `PT{whole}.{frac}S` (or
/// `-PT{whole}.{frac}S` for negatives). Whole seconds + three fractional
/// digits — the parser ingests up to four fractional digits with rounding,
/// so three digits is lossless for whole-millisecond inputs.
fn ms_to_canonical_pt_seconds(ms: i64) -> String {
    let neg = ms < 0;
    // `i64::MIN.unsigned_abs()` is the safe abs; ms range bound keeps us
    // well clear of the `.abs()` overflow corner.
    let mag = ms.unsigned_abs();
    let whole = mag / 1000;
    let frac = mag % 1000;
    let body = if frac == 0 {
        format!("PT{whole}S")
    } else {
        format!("PT{whole}.{frac:03}S")
    };
    if neg { format!("-{body}") } else { body }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 512,
        ..Default::default()
    })]

    /// Synthesized-canonical round-trip: a `PT{seconds}S` built from arbitrary
    /// bounded `i64` ms parses back to the same ms count, and the `_ms`
    /// wrapper agrees.
    #[test]
    fn synthesized_pt_seconds_round_trip(ms in -MS_RANGE..=MS_RANGE) {
        let canonical = ms_to_canonical_pt_seconds(ms);
        let parsed = parse_iso8601_duration(&canonical);
        prop_assert_eq!(parsed, IsoDurationParse::Milliseconds(ms),
            "round-trip failed for canonical {:?}", canonical);
        prop_assert_eq!(parse_iso8601_duration_ms(&canonical), Some(ms));
    }

    /// Two-pass parser consistency on a multi-component canonical string.
    /// Build `P{y}Y{mo}M{w}W{d}DT{h}H{m}M{s}S` with bounded nonneg components,
    /// parse, re-emit the resulting ms as `PT{whole}.{frac}S`, parse again,
    /// assert equality. This pins the parser as deterministic across the
    /// canonical-form normalization step.
    #[test]
    fn multi_component_two_pass_consistency(
        y in 0u32..=20,
        mo in 0u32..=120,
        w in 0u32..=520,
        d in 0u32..=3_650,
        h in 0u32..=24_000,
        m in 0u32..=60_000,
        s in 0u32..=86_400,
    ) {
        let input = format!("P{y}Y{mo}M{w}W{d}DT{h}H{m}M{s}S");
        let first = parse_iso8601_duration(&input);

        // All components nonneg and bounded — must produce `Milliseconds`.
        let ms = match first {
            IsoDurationParse::Milliseconds(v) => v,
            other => {
                prop_assert!(false,
                    "multi-component input {:?} produced {:?}; expected Milliseconds",
                    input, other);
                unreachable!()
            }
        };

        // Hand-computed total — must match the parser bit-for-bit.
        let expected = i64::from(y) * MS_PER_YEAR
            + i64::from(mo) * MS_PER_MONTH
            + i64::from(w) * MS_PER_WEEK
            + i64::from(d) * MS_PER_DAY
            + i64::from(h) * MS_PER_HOUR
            + i64::from(m) * MS_PER_MINUTE
            + i64::from(s) * MS_PER_SECOND;
        prop_assert_eq!(ms, expected,
            "parser total disagrees with hand-computed sum for {:?}", input);

        // Re-emit canonical PT-seconds form and re-parse — must match.
        let reemitted = ms_to_canonical_pt_seconds(ms);
        let second = parse_iso8601_duration(&reemitted);
        prop_assert_eq!(second, first,
            "two-pass parse differs: {:?} -> {:?} -> {:?}",
            input, reemitted, second);

        // `_ms` wrapper consistency on both passes.
        prop_assert_eq!(parse_iso8601_duration_ms(&input), Some(ms));
        prop_assert_eq!(parse_iso8601_duration_ms(&reemitted), Some(ms));
    }

    /// Fractional-second round-trip: `PT{whole}.{frac:03}S` with up to three
    /// fractional digits is lossless under the parser's rounding rule (the
    /// fourth digit governs half-up; absent, no rounding occurs).
    #[test]
    fn fractional_seconds_three_digit_round_trip(
        whole in 0u32..=86_400,
        frac in 0u32..=999,
    ) {
        let input = format!("PT{whole}.{frac:03}S");
        let parsed = parse_iso8601_duration(&input);
        let expected_ms = i64::from(whole) * MS_PER_SECOND + i64::from(frac);
        prop_assert_eq!(parsed, IsoDurationParse::Milliseconds(expected_ms),
            "fractional round-trip failed for {:?}", input);
        prop_assert_eq!(parse_iso8601_duration_ms(&input), Some(expected_ms));
    }

    /// Negative-prefix round-trip: a leading `-` produces negated milliseconds
    /// and round-trips via the canonical `-PT{whole}.{frac}S` reformat.
    /// (The parser accepts a single leading `-` on the whole duration, not
    /// per-component `-1D` — confirmed by reading `parse_iso8601_duration`.)
    #[test]
    fn negative_prefix_round_trip(ms_mag in 1i64..=MS_RANGE) {
        let ms = -ms_mag;
        let canonical = ms_to_canonical_pt_seconds(ms);
        prop_assert!(canonical.starts_with('-'),
            "canonical form of negative ms must start with '-': {:?}", canonical);
        let parsed = parse_iso8601_duration(&canonical);
        prop_assert_eq!(parsed, IsoDurationParse::Milliseconds(ms));
        prop_assert_eq!(parse_iso8601_duration_ms(&canonical), Some(ms));
    }

    /// `_ms` wrapper invariant on arbitrary inputs: returns `Some(x)` iff
    /// `parse_iso8601_duration` returns `Milliseconds(x)`; `None` for
    /// `Invalid` or `OutOfRange`.
    #[test]
    fn ms_wrapper_consistency_on_arbitrary_inputs(
        bytes in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        let s = String::from_utf8_lossy(&bytes);
        let full = parse_iso8601_duration(&s);
        let wrapper = parse_iso8601_duration_ms(&s);
        match full {
            IsoDurationParse::Milliseconds(x) => prop_assert_eq!(wrapper, Some(x)),
            IsoDurationParse::Invalid | IsoDurationParse::OutOfRange => {
                prop_assert_eq!(wrapper, None);
            }
        }
    }
}
