//! Emits deterministic conformance fixtures for cross-runtime differential testing.
//!
//! Usage: `cargo run --features proptest-strategies --bin emit-conformance-fixtures -- 256 > fixtures.jsonl`
//!
//! Each line is `{"source": "...", "value": ...}` consumed by sibling runtime harnesses.
use fel_core::{builtin_function_catalog, evaluate, fel_to_json, MapEnvironment};
use fel_core::testing::strategies::arb_expr;
use proptest::prelude::*;
use proptest::test_runner::TestRunner;

fn main() {
    let catalog = builtin_function_catalog();
    let strategy = arb_expr(3, catalog);
    let mut runner = TestRunner::deterministic();
    let mut count = 0usize;

    let target: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(256);

    let mut buf = String::new();
    while count < target {
        if let Ok(tree) = strategy.new_tree(&mut runner) {
            let expr = tree.current();
            let env = MapEnvironment::new();
            let result = evaluate(&expr, &env);
            let source = fel_core::print_expr(&expr);
            let value = fel_to_json(&result.value);
            let json = serde_json::json!({
                "source": source,
                "value": value,
            });
            writeln_jsonl(&mut buf, &json);
            count += 1;
            if count % 64 == 0 {
                eprintln!("emitted {count}/{target}");
            }
        }
    }
    print!("{buf}");
}

fn writeln_jsonl(buf: &mut String, value: &serde_json::Value) {
    buf.push_str(&serde_json::to_string(value).unwrap());
    buf.push('\n');
}
