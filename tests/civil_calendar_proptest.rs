//! Property tests for the Hinnant civil-calendar primitives exposed by fel-core.
//!
//! Validates `days_from_civil_pub` / `civil_from_days_pub` / `days_in_month`
//! against `chrono::NaiveDate` over the Gregorian range 1900–2200.
#![allow(clippy::missing_docs_in_private_items)]

use chrono::Datelike;
use fel_core::{civil_from_days_pub, days_from_civil_pub, days_in_month};
use proptest::prelude::*;

// ── Epoch offset ──────────────────────────────────────────────────────────────
//
// The Hinnant algorithm uses the Unix epoch (1970-01-01) as its reference point:
//   days_from_civil_pub(1970, 1, 1) == 0
//
// chrono's `num_days_from_ce` counts from 0001-01-01 (CE epoch):
//   NaiveDate::from_ymd_opt(1970, 1, 1).num_days_from_ce() == 719163
//
// So: fel_days = chrono_days_from_ce - EPOCH_OFFSET
//     EPOCH_OFFSET = 719163
const EPOCH_OFFSET: i32 = 719_163;

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        ..Default::default()
    })]

    /// `days_from_civil_pub` matches chrono's `num_days_from_ce` shifted by the epoch offset.
    #[test]
    fn days_from_civil_matches_chrono(
        year in 1900_i32..=2200_i32,
        month in 1_u32..=12_u32,
        // 1-28 is valid for every (year, month) combination; avoids conditional range logic.
        day in 1_u32..=28_u32,
    ) {
        let max_day = days_in_month(year, month);
        prop_assume!(day <= max_day);

        let fel_days = days_from_civil_pub(year, month, day);
        let chrono_date = chrono::NaiveDate::from_ymd_opt(year, month, day)
            .expect("chrono rejected a date that days_in_month accepted");
        let chrono_days = chrono_date.num_days_from_ce();
        let expected = (chrono_days - EPOCH_OFFSET) as i64;

        prop_assert_eq!(fel_days, expected,
            "days_from_civil_pub({}, {}, {}) = {}, expected {}",
            year, month, day, fel_days, expected
        );
    }

    /// Round-trip: `civil_from_days_pub(days_from_civil_pub(y, m, d))` == original date.
    #[test]
    fn round_trip_civil(
        year in 1900_i32..=2200_i32,
        month in 1_u32..=12_u32,
        day in 1_u32..=28_u32,
    ) {
        let max_day = days_in_month(year, month);
        prop_assume!(day <= max_day);

        let days = days_from_civil_pub(year, month, day);
        let reconstructed = civil_from_days_pub(days);
        let expected = fel_core::Date::Date { year, month, day };

        prop_assert_eq!(reconstructed, expected,
            "round-trip failed for {}-{:02}-{:02}",
            year, month, day
        );
    }

    /// `days_in_month` matches chrono's last day of the month for the same (year, month).
    #[test]
    fn days_in_month_matches_chrono(
        year in 1900_i32..=2200_i32,
        month in 1_u32..=12_u32,
    ) {
        let fel_dim = days_in_month(year, month);

        // chrono: last day = first day of next month minus one day.
        let next_month = if month == 12 { 1 } else { month + 1 };
        let next_year  = if month == 12 { year + 1 } else { year };
        let first_of_next = chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1)
            .expect("valid next-month date");
        let last_of_this = first_of_next.pred_opt().expect("valid predecessor");
        let chrono_dim = last_of_this.day();

        prop_assert_eq!(fel_dim, chrono_dim,
            "days_in_month({}, {}) = {}, chrono says {}",
            year, month, fel_dim, chrono_dim
        );
    }
}
