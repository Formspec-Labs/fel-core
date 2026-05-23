//! Adversarial property tests: nesting near limits, whitespace extremes, and unicode tails.
//!
//! Complements `parser_parse_proptest.rs` (uniform random bytes) with structured chaos distributions.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{MapEnvironment, evaluate, parse, tokenize};
use proptest::prelude::*;

fn tokenize_parse_eval_do_not_panic(src: &str) {
    let _ = tokenize(src);
    if let Ok(expr) = parse(src) {
        let env = MapEnvironment::new();
        let _ = evaluate(&expr, &env);
    }
}

/// Depth from shallow through beyond the parser nesting cap (32 frames).
fn arb_parenthesis_depth() -> impl Strategy<Value = usize> {
    0usize..45
}

fn nested_literal(depth: usize) -> String {
    let mut s = String::with_capacity(depth.saturating_mul(2).saturating_add(4));
    s.extend(std::iter::repeat_n('(', depth));
    s.push_str("42");
    s.extend(std::iter::repeat_n(')', depth));
    s
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 64,
        ..Default::default()
    })]

    #[test]
    fn nested_parens_never_panic(depth in arb_parenthesis_depth()) {
        let src = nested_literal(depth);
        tokenize_parse_eval_do_not_panic(&src);
    }

    /// Beyond robustness: the parser's `max_recursion_depth` is 32
    /// internal recursion frames (`src/parser.rs:52`). Each paren pair
    /// in nested literals adds ≥1 frame plus the expression-parser's own
    /// frames (typically +1 per expression layer). Inputs with paren
    /// depth ≥ 33 reliably exceed the cap and MUST be rejected with a
    /// parse error — not just "not panic."
    #[test]
    fn nested_parens_above_cap_are_rejected(extra in 1usize..13) {
        let depth = 33 + extra; // strictly above the per-paren cap
        let src = nested_literal(depth);
        prop_assert!(
            parse(&src).is_err(),
            "depth-{depth} parens (above 32-frame cap) must produce parse error"
        );
    }

    /// Symmetric lower bound: shallow paren nesting MUST parse. Cap is
    /// 16 here (well under the internal frame cap of 32) to leave room
    /// for the expression-parser's own frame contribution per layer.
    /// Discriminates the rejection-cap from a "rejects everything"
    /// regression.
    #[test]
    fn nested_parens_well_below_cap_parse(depth in 0usize..=16) {
        let src = nested_literal(depth);
        prop_assert!(
            parse(&src).is_ok(),
            "depth-{depth} parens (well below 32-frame cap) must parse"
        );
    }

    #[test]
    fn whitespace_inflation_never_panic(
        pad in 0usize..512_usize,
        chars in prop::collection::vec((b'a'..=b'z').prop_map(|b| b as char), 1usize..13usize),
    ) {
        let core: String = chars.into_iter().collect();
        let space = " ".repeat(pad);
        let src = format!("{space}{core}{space}+{space}1{space}");
        tokenize_parse_eval_do_not_panic(&src);
    }

    #[test]
    fn lossy_utf8_with_unicode_tail_never_panic(
        bytes in prop::collection::vec(any::<u8>(), 0..384),
        extra in prop::collection::vec(0x80u32..0x110000, 0..24),
    ) {
        let mut base = String::from_utf8_lossy(&bytes).into_owned();
        for cp in extra {
            if let Some(ch) = char::from_u32(cp) {
                if !ch.is_control() {
                    base.push(ch);
                }
            }
        }
        tokenize_parse_eval_do_not_panic(&base);
    }

    #[test]
    fn long_flat_add_chain_never_panic(terms in 8usize..50usize) {
        let mut src = String::with_capacity(terms.saturating_mul(4));
        src.push('0');
        for _ in 0..terms {
            src.push_str(" + 1");
        }
        tokenize_parse_eval_do_not_panic(&src);
    }
}
