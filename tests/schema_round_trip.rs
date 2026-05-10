//! Round-trip test: fel-core's emitted FEL schema matches the canonical
//! `formspec/schemas/fel-functions.schema.json` byte-for-byte (up to JSON
//! semantic equivalence — key order in objects doesn't matter).

const CANONICAL: &str = include_str!("../../formspec/schemas/fel-functions.schema.json");

#[test]
fn emitted_schema_matches_canonical() {
    let canonical: serde_json::Value = serde_json::from_str(CANONICAL).unwrap();
    let emitted = fel_core::extensions::emit_schema_json();
    assert_eq!(
        emitted, canonical,
        "fel-core emission diverges from canonical schema"
    );
}
