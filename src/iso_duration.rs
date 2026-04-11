//! ISO 8601 duration parsing for FEL `duration()`.
//!
//! Parses the `PnYnMnDTnHnMnS` form. Date-designator `M` is months; after `T`, `M` is minutes.
//! Years and months in the date component use fixed lengths (365 days, 30 days) — not calendar
//! arithmetic. See Core spec §3.5.4 `duration`.

use regex::Regex;
use std::sync::OnceLock;

fn date_part_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:(\d+)Y)?(?:(\d+)M)?(?:(\d+)W)?(?:(\d+)D)?$").expect("valid regex")
    })
}

fn time_part_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)(?:\.(\d+))?S)?$").expect("valid regex")
    })
}

const MS_PER_SECOND: i128 = 1000;
const MS_PER_MINUTE: i128 = 60 * MS_PER_SECOND;
const MS_PER_HOUR: i128 = 60 * MS_PER_MINUTE;
const MS_PER_DAY: i128 = 24 * MS_PER_HOUR;
const MS_PER_WEEK: i128 = 7 * MS_PER_DAY;
const MS_PER_MONTH: i128 = 30 * MS_PER_DAY;
const MS_PER_YEAR: i128 = 365 * MS_PER_DAY;

/// Outcome of parsing an ISO 8601 duration for FEL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsoDurationParse {
    /// Whole milliseconds (FEL `number`).
    Milliseconds(i64),
    /// Empty input, missing `P`, unsupported shape, or a numeric component that does not fit `i128`.
    Invalid,
    /// Total milliseconds do not fit in `i64`.
    OutOfRange,
}

fn component_i128(caps: &regex::Captures<'_>, idx: usize) -> Option<i128> {
    match caps.get(idx) {
        None => Some(0),
        Some(m) => m.as_str().parse().ok(),
    }
}

fn accumulate(total: &mut i128, delta: i128) -> Result<(), ()> {
    *total = total.checked_add(delta).ok_or(())?;
    Ok(())
}

/// Parse an ISO 8601 duration; distinguishes invalid input from out-of-range totals.
pub fn parse_iso8601_duration(input: &str) -> IsoDurationParse {
    let s = input.trim();
    if s.is_empty() {
        return IsoDurationParse::Invalid;
    }
    let neg = s.starts_with('-');
    let s = s.strip_prefix('-').unwrap_or(s).trim_start();
    let Some(s) = s.strip_prefix('P') else {
        return IsoDurationParse::Invalid;
    };
    if s.is_empty() {
        return IsoDurationParse::Invalid;
    }

    let (date_str, time_str) = if let Some(i) = s.find('T') {
        (&s[..i], Some(&s[i + 1..]))
    } else {
        (s, None)
    };

    if date_str.is_empty() && time_str == Some("") {
        return IsoDurationParse::Invalid;
    }

    let mut total: i128 = 0;

    if !date_str.is_empty() {
        let Some(caps) = date_part_re().captures(date_str) else {
            return IsoDurationParse::Invalid;
        };
        if &caps[0] != date_str {
            return IsoDurationParse::Invalid;
        }
        let y = match component_i128(&caps, 1) {
            Some(v) => v,
            None => return IsoDurationParse::Invalid,
        };
        let mo = match component_i128(&caps, 2) {
            Some(v) => v,
            None => return IsoDurationParse::Invalid,
        };
        let w = match component_i128(&caps, 3) {
            Some(v) => v,
            None => return IsoDurationParse::Invalid,
        };
        let d = match component_i128(&caps, 4) {
            Some(v) => v,
            None => return IsoDurationParse::Invalid,
        };
        let Some(dy) = y.checked_mul(MS_PER_YEAR) else {
            return IsoDurationParse::OutOfRange;
        };
        if accumulate(&mut total, dy).is_err() {
            return IsoDurationParse::OutOfRange;
        }
        let Some(dmo) = mo.checked_mul(MS_PER_MONTH) else {
            return IsoDurationParse::OutOfRange;
        };
        if accumulate(&mut total, dmo).is_err() {
            return IsoDurationParse::OutOfRange;
        }
        let Some(dw) = w.checked_mul(MS_PER_WEEK) else {
            return IsoDurationParse::OutOfRange;
        };
        if accumulate(&mut total, dw).is_err() {
            return IsoDurationParse::OutOfRange;
        }
        let Some(dd) = d.checked_mul(MS_PER_DAY) else {
            return IsoDurationParse::OutOfRange;
        };
        if accumulate(&mut total, dd).is_err() {
            return IsoDurationParse::OutOfRange;
        }
    }

    if let Some(t) = time_str {
        if !t.is_empty() {
            let Some(caps) = time_part_re().captures(t) else {
                return IsoDurationParse::Invalid;
            };
            if &caps[0] != t {
                return IsoDurationParse::Invalid;
            }
            let h = match component_i128(&caps, 1) {
                Some(v) => v,
                None => return IsoDurationParse::Invalid,
            };
            let m = match component_i128(&caps, 2) {
                Some(v) => v,
                None => return IsoDurationParse::Invalid,
            };
            let s_whole = match component_i128(&caps, 3) {
                Some(v) => v,
                None => return IsoDurationParse::Invalid,
            };
            let frac_ms = caps
                .get(4)
                .map(|f| fractional_seconds_to_ms(f.as_str()))
                .unwrap_or(0);
            let frac_ms = i128::from(frac_ms);
            let Some(dh) = h.checked_mul(MS_PER_HOUR) else {
                return IsoDurationParse::OutOfRange;
            };
            if accumulate(&mut total, dh).is_err() {
                return IsoDurationParse::OutOfRange;
            }
            let Some(dm) = m.checked_mul(MS_PER_MINUTE) else {
                return IsoDurationParse::OutOfRange;
            };
            if accumulate(&mut total, dm).is_err() {
                return IsoDurationParse::OutOfRange;
            }
            let Some(ds) = s_whole.checked_mul(MS_PER_SECOND) else {
                return IsoDurationParse::OutOfRange;
            };
            if accumulate(&mut total, ds).is_err() {
                return IsoDurationParse::OutOfRange;
            }
            if accumulate(&mut total, frac_ms).is_err() {
                return IsoDurationParse::OutOfRange;
            }
        }
    }

    if total == 0 && date_str.is_empty() && time_str.is_none() {
        return IsoDurationParse::Invalid;
    }

    let out = if neg { -total } else { total };
    match i64::try_from(out) {
        Ok(ms) => IsoDurationParse::Milliseconds(ms),
        Err(_) => IsoDurationParse::OutOfRange,
    }
}

/// Parse an ISO 8601 duration to whole milliseconds.
///
/// Returns `None` on invalid input or if the result does not fit `i64`.
#[inline]
pub fn parse_iso8601_duration_ms(input: &str) -> Option<i64> {
    match parse_iso8601_duration(input) {
        IsoDurationParse::Milliseconds(ms) => Some(ms),
        IsoDurationParse::Invalid | IsoDurationParse::OutOfRange => None,
    }
}

fn fractional_seconds_to_ms(frac: &str) -> i64 {
    if frac.is_empty() {
        return 0;
    }
    let mut v = 0.0f64;
    for (i, c) in frac.chars().enumerate().take(9) {
        if let Some(d) = c.to_digit(10) {
            v += (d as f64) * 10f64.powi(-((i as i32) + 1));
        }
    }
    (v * 1000.0).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pt1s() {
        assert_eq!(
            parse_iso8601_duration("PT1S"),
            IsoDurationParse::Milliseconds(1000)
        );
    }

    #[test]
    fn pt1h() {
        assert_eq!(
            parse_iso8601_duration("PT1H"),
            IsoDurationParse::Milliseconds(3_600_000)
        );
    }

    #[test]
    fn p1d() {
        assert_eq!(
            parse_iso8601_duration("P1D"),
            IsoDurationParse::Milliseconds(86_400_000)
        );
    }

    #[test]
    fn combined() {
        assert_eq!(
            parse_iso8601_duration("P1DT12H"),
            IsoDurationParse::Milliseconds(86_400_000 + 12 * 3_600_000)
        );
    }

    #[test]
    fn negative() {
        assert_eq!(
            parse_iso8601_duration("-PT1M"),
            IsoDurationParse::Milliseconds(-60_000)
        );
    }

    #[test]
    fn fractional_seconds() {
        assert_eq!(
            parse_iso8601_duration("PT0.5S"),
            IsoDurationParse::Milliseconds(500)
        );
    }

    #[test]
    fn bare_p_rejected() {
        assert_eq!(parse_iso8601_duration("P"), IsoDurationParse::Invalid);
    }

    #[test]
    fn pt_without_components_rejected() {
        assert_eq!(parse_iso8601_duration("PT"), IsoDurationParse::Invalid);
    }

    #[test]
    fn p0d_is_zero() {
        assert_eq!(
            parse_iso8601_duration("P0D"),
            IsoDurationParse::Milliseconds(0)
        );
    }

    #[test]
    fn empty_rejected() {
        assert_eq!(parse_iso8601_duration(""), IsoDurationParse::Invalid);
    }

    #[test]
    fn overflow_total_ms_is_out_of_range() {
        assert_eq!(
            parse_iso8601_duration("P106751991167301D"),
            IsoDurationParse::OutOfRange
        );
    }

    #[test]
    fn component_too_large_for_i128_is_invalid() {
        let digits = "9".repeat(50);
        let s = format!("P{digits}D");
        assert_eq!(parse_iso8601_duration(&s), IsoDurationParse::Invalid);
    }

    #[test]
    fn parse_ms_wrapper_matches() {
        assert_eq!(parse_iso8601_duration_ms("PT1H"), Some(3_600_000));
    }
}
