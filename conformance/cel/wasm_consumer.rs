//! Loads the public source converter through a WebAssembly export.
// Rust guideline compliant 2026-02-21

use fel_core::cel::{Language, convert};

/// Return one when a deeply nested CEL expression survives a source round trip.
#[unsafe(no_mangle)]
pub extern "C" fn check_source_round_trip() -> u32 {
    let source = format!("{}item{}", "f(".repeat(50), ")".repeat(50));
    let result = convert(&source, Language::Cel).and_then(|fel| convert(&fel, Language::Fel));
    u32::from(result.as_ref() == Ok(&source))
}
