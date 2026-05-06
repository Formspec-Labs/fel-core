//! Grammar-biased fuzzing: combine fixed seed snippets with arbitrary tail bytes so mutations stay closer to valid syntax.
//!
//! LibFuzzer mutates files under `fuzz/corpus/fel_structured/` when running this target.
#![no_main]

use fel_core::{evaluate, parse, tokenize, MapEnvironment};

/// Short expressions that exercise parsing, calls, arrays, and field paths.
const SEEDS: &[&str] = &[
    "1",
    "1 + 2 * 3",
    "(42)",
    "$ab",
    "sum([1,2,3])",
    "true = false",
    "null",
    "if(true, 1, 2)",
    "\"hello\"",
];

fn mix_seed_and_tail(seed: &str, tail: &[u8]) -> String {
    let mut out = String::with_capacity(seed.len().saturating_add(tail.len()));
    out.push_str(seed);
    out.push_str(&String::from_utf8_lossy(tail));
    out
}

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let (idx, tail) = match data.split_first() {
        Some((&head, rest)) => (head as usize % SEEDS.len(), rest),
        None => return,
    };
    let src = mix_seed_and_tail(SEEDS[idx], tail);

    let _ = tokenize(src.as_ref());

    let Ok(expr) = parse(src.as_ref()) else {
        return;
    };

    let env = MapEnvironment::new();
    let _ = evaluate(&expr, &env);
});
