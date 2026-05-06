//! Parser smoke tests — random inputs must not panic (may return `Err`).
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::parse;
use proptest::prelude::*;

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        ..Default::default()
    })]

    #[test]
    fn parse_random_lossy_utf8_does_not_panic(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let s = String::from_utf8_lossy(&bytes);
        let _ = parse(&s);
    }
}
