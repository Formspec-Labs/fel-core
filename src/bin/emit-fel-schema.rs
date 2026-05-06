//! CLI binary: emit the FEL function catalog as JSON to stdout.
//!
//! Usage:
//! ```sh
//! cargo run -p fel-core --bin emit-fel-schema > formspec/schemas/fel-functions.schema.json
//! ```
fn main() {
    let v = fel_core::extensions::emit_schema_json();
    serde_json::to_writer_pretty(std::io::stdout(), &v)
        .expect("failed to write schema JSON to stdout");
    println!(); // trailing newline
}
