# fel-core

Rust implementation of **FEL** — the Formspec Expression Language. Lex, parse, evaluate, and statically analyze expressions with **base-10 decimal** arithmetic (no float drift), explicit null propagation, and JSON-native interop.

FEL is small enough to embed safely — no loops, no I/O, no mutable state — but expressive enough to drive form validation, computed fields, conditional logic, and policy decisions. Closest neighbor in the Rust ecosystem: [CEL](https://github.com/cel-rust/cel-rust).

## Usage

Not yet published to crates.io. Consume via path while the Rust API stabilizes:

```toml
[dependencies]
fel-core = { path = "../fel-core" }
rust_decimal = "1"
```

Evaluate a simple expression:

```rust
use fel_core::{parse, evaluate, MapEnvironment, Value};
use rust_decimal::Decimal;
use std::collections::HashMap;

let expr = parse("$a + 1").unwrap();

let mut fields = HashMap::new();
fields.insert("a".into(), Value::Number(Decimal::from(2)));
let env = MapEnvironment::with_fields(fields);

let out = evaluate(&expr, &env);
assert_eq!(out.value, Value::Number(Decimal::from(3)));
```

Full public API: [`docs/rustdoc-md/API.md`](docs/rustdoc-md/API.md) (rustdoc bundled to one file). Regenerate with `npm run docs:fel-core`.

## Convert CEL and FEL source

Use Rust 1.88 or later. Enable the optional `cel-bridge` feature to use
`fel_core::cel` or install the
`fel-convert` command from this checkout. Each conversion takes one expression
from an argument or standard input and writes the converted source to standard
output. Errors go to standard error with a failing exit status.

```sh
cargo install --path . --features cel-bridge --bin fel-convert
fel-convert to-cel "money(\$fee, 'USD')" | fel-convert to-fel
# money($fee, 'USD')
```

The default preserves the parsed expression for round-trip editing. Keep the
opening version comment. Shared syntax stays readable and named helper calls
represent features unique to either language. When the target grammar cannot
express the full tree within its limits, a helper can hold the original source.
Comments, spacing and equivalent literal spellings can change. Use `--literal`
to treat an author's opening marker comment as ordinary text.

Use `--native` for ordinary target expressions. Supply the same field mappings
in both directions. Undeclared fields default to nullable strings. Explicit
bindings replace those defaults. `--snapshot` selects fields from the FEL
`celInputs` instance.

```sh
fel-convert to-cel --native --bind 'names:list=$medications[*].medDrugName' 'count($medications[*].medDrugName)'
# names.filter(item, dyn(item) != null).size()
fel-convert to-fel --native --bind 'names:list=$medications[*].medDrugName' 'size(names)'
# length($medications[*].medDrugName)
```

FEL `count` counts non-null values. CEL `size` and fel-core `length` count all
list positions. A native conversion requires compatible input types and a
verified return trip. Both modes parse source without evaluating a rule.
The converter accepts up to 256 KiB per expression, 64 nested delimiters and
256 structural operations. FEL's own parser also limits nesting. Run
`make test-cel-bridge` for the Rust API tests and the independent Go CEL checks.
[Rust API and examples](src/cel/mod.rs) · [Function coverage](conformance/cel/README.md)

## Features

- **Decimal arithmetic** via `rust_decimal` — no `NaN`, no precision drift on money-like values.
- **Null propagation** — `null + 1 == null`; explicit `??` coalesce when you want a default.
- **Static dependency analysis** — `extract_dependencies()` walks the AST without evaluating, surfacing field references, context bindings, MIPs, and wildcard usage.
- **JSON-native** — bidirectional `Value ↔ serde_json::Value` with three wire styles (UI-friendly, exact-wire, JS-safe-range).
- **Host-pluggable** — `Environment` trait for field/context resolution; `ExtensionRegistry` for custom functions.
- **Closed diagnostic set** — `DiagnosticKind` is append-only; consumers can match exhaustively.
- **Cross-runtime conformance** — same fixture corpus runs against the TypeScript and Python implementations.

## Conformance & spec

FEL is a specification first, an implementation second. Normative sources:

- [`specs/fel/fel-grammar.md`](specs/fel/fel-grammar.md) — grammar
- [`docs/SPEC.md`](docs/SPEC.md) — semantics
- [`conformance/`](conformance/) — fixture corpus, byte-for-byte cross-runtime parity

`formspec/schemas/fel-functions.schema.json` is generated from the Rust builtin catalog (`cargo run --bin emit-fel-schema`).

## Quality gates

| Command | Runs |
|---|---|
| `cargo test` | Unit + integration + [`lib_reexport_coverage_gate`](tests/lib_reexport_coverage_gate.rs) (every `pub use` requires a proptest cite). |
| `make ratify` | Local internal ratification — spec/conformance, schema regen, all-features tests, rustdoc broken-link denial. |
| `make ratify-external` | Cross-runtime gate against sibling Python + WASM runtimes. |
| `make ci` | OSS-readiness — adds rustfmt, clippy `-D warnings`, `cargo-deny`, `cargo package`. |

Weekly `cargo-mutants` job runs across the P0 file set; trend at [`conformance/mutation-baseline.jsonl`](conformance/mutation-baseline.jsonl), classification in [`thoughts/2026-05-23-mutation-survivor-followups.md`](thoughts/2026-05-23-mutation-survivor-followups.md).

## Stability

**Language ratified, Rust API pre-1.0.** FEL syntax, evaluation semantics, builtin behavior, diagnostic-kind wire shapes, and conformance expectations change only through a spec+fixture update. Rust entry points may still be renamed before crate publication.

## Project history

- [`CHANGELOG.md`](CHANGELOG.md) — release history
- [`TODO.md`](TODO.md) — backlog
- [`COMPLETED.md`](COMPLETED.md) — resolved-finding audit history (verbose evidence preserved out of the changelog)

## License & governance

Apache-2.0 — [LICENSE](LICENSE), [LICENSING.md](LICENSING.md).

- Security: [SECURITY.md](SECURITY.md)
- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)
- Conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- Dependency policy: [deny.toml](deny.toml)
