//! LibFuzzer harness: arbitrary bytes → UTF-8 lossy → tokenize → parse → evaluate → optional print round-trip.
//!
//! Run from `fel-core/fuzz`: `cargo fuzz run fel_pipeline -- -runs=10000`
#![no_main]

use fel_core::{evaluate, parse, print_expr, tokenize, MapEnvironment};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let src = String::from_utf8_lossy(data);
    let _ = tokenize(src.as_ref());

    let Ok(expr) = parse(src.as_ref()) else {
        return;
    };

    let env = MapEnvironment::new();
    let _ = evaluate(&expr, &env);

    let printed = print_expr(&expr);
    let _ = parse(printed.as_ref());
});
