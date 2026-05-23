//! Concurrency smoke test — verifies `MapEnvironment` is `Send + Sync`
//! and that concurrent evaluation against a shared environment produces
//! consistent results.
#![allow(clippy::missing_docs_in_private_items)]

use fel_core::{MapEnvironment, Value, evaluate, parse};
use std::sync::Arc;
use std::thread;

#[test]
fn concurrent_evaluation_against_arc_env() {
    let env = Arc::new(MapEnvironment::new());
    let exprs: Vec<String> = (0..16).map(|i| format!("{} + 1", i % 10)).collect();

    let mut handles = Vec::new();
    for src in &exprs {
        let env = Arc::clone(&env);
        let src = src.to_string();
        handles.push(thread::spawn(move || {
            let expr = parse(&src).unwrap();
            let result = evaluate(&expr, env.as_ref());
            result.value
        }));
    }

    let results: Vec<Value> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let serial: Vec<Value> = exprs
        .iter()
        .map(|src| {
            let expr = parse(src).unwrap();
            let env = MapEnvironment::new();
            evaluate(&expr, &env).value
        })
        .collect();

    assert_eq!(results, serial);
}

#[test]
fn concurrent_evaluation_with_fields() {
    use std::collections::HashMap;

    let fields: HashMap<String, Value> = [
        ("a".to_string(), Value::Number(10.into())),
        ("b".to_string(), Value::Number(20.into())),
    ]
    .into();
    let env = Arc::new(MapEnvironment::with_fields(fields));
    let exprs = vec!["$a + $b", "$a * $b", "$b - $a", "$b / $a"];

    let mut handles = Vec::new();
    for src in &exprs {
        let env = Arc::clone(&env);
        let src = src.to_string();
        handles.push(thread::spawn(move || {
            let expr = parse(&src).unwrap();
            let result = evaluate(&expr, env.as_ref());
            result.value
        }));
    }

    let results: Vec<Value> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert_eq!(
        results,
        vec![
            Value::Number(30.into()),
            Value::Number(200.into()),
            Value::Number(10.into()),
            Value::Number(2.into()),
        ]
    );
}

#[test]
fn sync_send_check() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MapEnvironment>();
}

/// True overlap-concurrency test: all worker threads enter `evaluate()`
/// on the shared Arc<env> *simultaneously* (synchronized via Barrier),
/// not sequentially as the smoke tests above. Verifies data-race
/// freedom under genuine concurrent access, not just "spawn-then-join."
///
/// Addresses operational-tests reviewer MEDIUM: smoke tests verified
/// sequential interleaving but not concurrent overlap.
#[test]
fn concurrent_overlap_against_arc_env() {
    use std::collections::HashMap;
    use std::sync::Barrier;

    const WORKERS: usize = 8;
    let fields: HashMap<String, Value> = [
        ("x".to_string(), Value::Number(100.into())),
        ("y".to_string(), Value::Number(7.into())),
    ]
    .into();
    let env = Arc::new(MapEnvironment::with_fields(fields));
    let barrier = Arc::new(Barrier::new(WORKERS));

    // All workers compute `$x + $y` (deterministic) but enter eval()
    // simultaneously. Any interior-mutability race or non-Sync field
    // would surface as miscompare or panic under TSAN; passing here
    // pins the no-race invariant on the public API surface.
    let handles: Vec<_> = (0..WORKERS)
        .map(|_| {
            let env = Arc::clone(&env);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let expr = parse("$x + $y").unwrap();
                barrier.wait(); // all workers reach this point, then enter eval together
                evaluate(&expr, env.as_ref()).value
            })
        })
        .collect();

    let results: Vec<Value> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.len(), WORKERS);
    for r in &results {
        assert_eq!(*r, Value::Number(107.into()));
    }
}
