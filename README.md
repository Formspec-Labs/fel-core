# fel-core

Rust implementation of **Formspec Expression Language (FEL)** — lex, parse, evaluate, and analyze dependencies with **base-10 decimal** arithmetic (`rust_decimal`), aligned with Formspec core spec semantics.

## Scope

| Layer | Role |
|--------|------|
| **lexer** | Tokenize source with spans (chars + decimal literals). |
| **parser** | Recursive descent → `ast::Expr`. |
| **evaluator** | Tree walk, null propagation, builtins, `Environment` trait for `$` / `@` / MIPs. Split across `evaluator/{core, util, builtins/*}.rs`. |
| **dependencies** | Static AST walk for field/context/MIP refs (no evaluation). |
| **types** | `Value`, `Date`, `Money`, Howard Hinnant civil-day arithmetic. |
| **convert** | Canonical `serde_json::Value` ↔ `Value`. |
| **environment** | `FormspecEnvironment` for engine-style evaluation (host adapter; non-formspec hosts implement the trait directly via `MapEnvironment`). |
| **extensions** | Typed builtin catalog (`BuiltinFunctionCatalogEntry`, `Parameter`, `FelType`, `Package`), schema JSON emission, `ExtensionRegistry` for runtime host functions. |

The FEL 1.0 internal-ratification surface is the specification set under
[`docs/SPEC.md`](docs/SPEC.md), [`specs/fel/fel-grammar.md`](specs/fel/fel-grammar.md),
and [`conformance/`](conformance/). `formspec/schemas/fel-functions.schema.json`
is generated from `BUILTIN_FUNCTIONS` via `cargo run --bin emit-fel-schema`.
TypeScript and Python implementations conform to that schema; a round-trip test
(`tests/schema_round_trip.rs`) keeps emission and the canonical schema in
lock-step.

## Project Status

- [`CHANGELOG.md`](CHANGELOG.md) is the public release history.
- [`TODO.md`](TODO.md) is the current backlog and ratification status.
- [`COMPLETED.md`](COMPLETED.md) is maintainer audit history for resolved
  findings. It preserves detailed evidence that is too verbose for the
  changelog; it is not an active planning surface.

## Architecture

**Authoritative API detail** is `cargo doc --no-deps`. A **single-file
Markdown mirror** of the same rustdoc (for editors and LLM context) is
[`docs/rustdoc-md/API.md`](docs/rustdoc-md/API.md), produced by
[cargo-doc-md](https://github.com/Crazytieguy/cargo-doc-md) plus
[`scripts/bundle-rustdoc-md.mjs`](scripts/bundle-rustdoc-md.mjs).

### Pipeline

```text
Source string
    → Lexer::tokenize → Vec<SpannedToken>
    → Parser::parse_expression → Expr (AST)
    → Evaluator::eval (or extract_dependencies for static analysis)
    → Value + Vec<Diagnostic>
```

- **Parse errors** surface as `Error::Parse` wrapping `ParseError` (human-readable `message` plus optional byte `span`; see bundled rustdoc).
- **Eval errors** produce `Diagnostic` entries and a null or partial value (non-fatal evaluation path).

### Evaluation diagnostics (`kind`)

Some diagnostics include structured `kind` when serialized with [`fel_diagnostics_to_json_value`](src/error.rs): **`undefinedFunction`** (`name`), **`typeMismatch`** (`fnName` / `fn_name`, `expected`, `got`). Plain message-only diagnostics omit `kind`.

## Diagnostics

The [`DiagnosticKind`](src/error.rs) enum defines the closed set of machine-readable diagnostic categories produced during evaluation:

| Variant | Fields | Meaning |
|---------|--------|---------|
| `UndefinedFunction` | `name: String` | Function name could not be resolved in builtins or extension registry. |
| `TypeMismatch` | `fn_name: String`, `expected: String`, `got: String` | Builtin or expression context expected a different runtime type. |

**Stability commitment:** Diagnostic kinds are append-only. Existing kinds and their field shapes are stable through 1.0. Hosts may match exhaustively on the closed set.

### Parser limits

Maximum nested expression depth is **32** recursive frames (deep parentheses); exceeding it yields a parse error before stack exhaustion.

### ISO 8601 duration fractional seconds

The duration parser maps fractional seconds to milliseconds using the **first four** fractional digits (three millisecond digits plus one rounding digit); further digits do not increase precision beyond that quantization.

### Crate-root API

Stable entry points (`evaluate`, `Trace`, `IndexMap`, JSON helpers, …) are re-exported at the crate root. Implementation modules such as `trace` and `iso_duration` are `pub(crate)` so downstream code prefers stable paths.

### Versioning posture

**Language ratified, Rust API pre-1.0.** FEL language semantics are tracked as a
Formspec-internal v1.0 ratified specification in `docs/SPEC.md`, the normative
grammar, and the conformance corpus. The Rust crate is not yet published to
crates.io and is consumed via `path = "../fel-core"` in the monorepo. Public
Rust entry points may still be renamed before crate publication; syntax,
evaluation semantics, builtin behavior, diagnostic-kind wire shapes, and
conformance expectations should change only through a spec+fixture update.

### Ratification gates

```sh
make ratify
```

Runs the local internal-ratification gate: static spec/conformance checks,
byte-for-byte public conformance regeneration, full all-features tests, and
rustdoc broken-link denial.

```sh
make ratify-external
```

Runs the optional cross-runtime implementation gate against sibling Python and
WASM runtimes when those runtimes are present. The GitHub Actions workflow also
offers a scheduled/dispatch-only external conformance job for maintainers who
configure access to the sibling Formspec runtime repository.

```sh
make ci
```

Runs the local OSS-readiness gate: rustfmt, clippy with `-D warnings`,
`cargo-deny`, ratification checks, full all-features tests, rustdoc broken-link
denial, and `cargo package`.

### JSON conversion, numbers, and dates

[`convert`](src/convert.rs) serializes FEL dates as ISO strings and does **not**
coerce arbitrary JSON strings into `Date` on ingest (see
`string_no_date_coercion` tests). That avoids silent type surprises unless a
host opts into an explicit tagged shape.

FEL numbers are base-10 decimals, not JavaScript numbers or `BigInt`s.
`fel_to_json` / `fel_to_ui_json` emit JSON numbers only when the printed JSON
number preserves the decimal text and whole integers are within JavaScript's
safe integer range; otherwise they emit normalized decimal strings. Exact
machine interchange should use `fel_to_wire_json`, which tags numbers as
`{"$type":"number","value":...}` and uses strings for fractional decimals or
unsafe integers.

### Source modules (`src/`)

| File | Responsibility |
|------|----------------|
| `lib.rs` | Crate root, `#![deny(missing_docs)]` + `#![warn(clippy::missing_docs_in_private_items)]`, re-exports, `tokenize` / JSON helpers for FFI. |
| `lexer.rs` | `Token`, `Lexer`, spans. |
| `parser.rs` | `parse()`, precedence per module doc comment. |
| `ast.rs` | `Expr`, `PathSegment`, operators. |
| `evaluator/` | `evaluator/core.rs` — `Environment`, `MapEnvironment`, `Evaluator`, `evaluate()`, `eval_function` dispatch, broadcasting, null propagation. `evaluator/util.rs` — free helpers (decimal, plural rules, locale). `evaluator/builtins/{aggregates,strings,numeric,dates,locale,money,logic_types}.rs` — per-domain builtin impls on `Evaluator`. |
| `types.rs` | `Value`, `Date`, `Money`, literals, Hinnant `days_from_civil` / `civil_from_days` (epoch 1970-01-01). |
| `error.rs` | `Error`, `ParseError`, `Diagnostic`, `Severity`, JSON diagnostic helpers. |
| `dependencies.rs` | `Dependencies`, `extract_dependencies`, JSON wire helpers. |
| `environment.rs` | `FormspecEnvironment`, repeat + MIP + instances. |
| `context_json.rs` | `formspec_environment_from_json_map` for WASM-style payloads. |
| `convert.rs` | `json_to_fel`, `fel_to_json`, field maps. |
| `extensions/` | `types.rs` / `catalog.rs` (`BUILTIN_FUNCTIONS`), `schema.rs` (`emit_schema_json`, catalog JSON), `registry.rs` (`ExtensionRegistry`). Crate-root `fel_core::extensions::*` unchanged. |
| `iso_duration.rs` | ISO 8601 duration parser. `fn_duration` emits a warning diagnostic when input contains `Y` or date-component `M` (nominal-length lie). |
| `printer.rs` | `print_expr` (AST → source). |
| `prepare_host.rs` | FEL source rewriter for WASM/TS-binding parity. |
| `wire_style.rs` | `JsonWireStyle` for dependency JSON key casing. |
| `bin/emit-fel-schema.rs` | CLI: regenerates `formspec/schemas/fel-functions.schema.json` from the catalog. |

### Cross-stack consumers

- **formspec-core** — FEL analysis (catalog-driven type inference, parameter-position type checking), rewrites, shared types.
- **formspec-eval** — Batch definition evaluation.
- **formspec-py / formspec-wasm** — Thin FFI over `tokenize`, `parse`, `evaluate`, `list_builtin_functions`, JSON helpers.
- **work-spec / wos-core, wos-runtime** — FEL evaluation against `MapEnvironment` for workflow guards and decision tables.
- **work-spec / wos-lint** — Static lint rules consume `builtin_function_catalog_for(Package::Universal)` for unknown-function and boolean-shape checks.

## For LLM assistants

Before answering questions about this crate’s API, behavior, or module layout:

1. Read this README (**Architecture** above for layout and pipeline).
2. Read [`docs/rustdoc-md/API.md`](docs/rustdoc-md/API.md) in full (bundled public rustdoc).

Skipping that file will miss public-item rustdoc that is not duplicated here.

## Quick start

```rust
use fel_core::{parse, evaluate, MapEnvironment, Value};
use std::collections::HashMap;
use rust_decimal::Decimal;

let expr = parse("$a + 1").unwrap();
let mut fields = HashMap::new();
fields.insert("a".into(), Value::Number(Decimal::from(2)));
let env = MapEnvironment::with_fields(fields);
let out = evaluate(&expr, &env);
assert_eq!(out.value, Value::Number(Decimal::from(3)));
```

## API documentation (rustdoc)

From the repo root:

```bash
cargo doc --no-deps --open
```

Public-doc enforcement is on by default (`#![deny(missing_docs)]` at the crate
root). The `.github/workflows/doc.yml` CI gate additionally runs:

```bash
RUSTDOCFLAGS='-D rustdoc::broken-intra-doc-links' cargo doc --no-deps
```

### Markdown export (from doc comments)

One-time install:

```sh
cargo install cargo-doc-md
```

Regenerate HTML + bundled Markdown:

```sh
npm run docs:fel-core
```

This runs `cargo doc-md` into `target/doc-md-fel-core`,
`scripts/bundle-rustdoc-md.mjs` -> `docs/rustdoc-md/API.md`, then
`cargo doc --no-deps`.

## Internal (private) documentation

Public docs are denied (`#![deny(missing_docs)]`); private items are warned but selectively allowed inside heavyweight internal pipelines.

- **Narrative + allow** — `lexer`, `parser`, `dependencies`, `environment`, `extensions`, `printer`, `context_json`, and each `evaluator/` submodule extend the module `//!` with a short internal overview and use `#![allow(clippy::missing_docs_in_private_items)]` so internals are not each annotated with `///`.
- **Tests** — Each `#[cfg(test)] mod tests` uses `#![allow(clippy::missing_docs_in_private_items)]` for helper fns.

Strict check (lint only this crate):

```sh
cargo clippy --all-targets --all-features -- -D warnings
```

## Tests

```sh
cargo nextest run --all-features
```

Integration-style suites live under `tests/`. Notable:

- `schema_round_trip.rs` — `emit_schema_json()` byte-equals the canonical `formspec/schemas/fel-functions.schema.json`.
- `civil_calendar_proptest` — proptest harness validates Hinnant `days_from_civil` / `civil_from_days` / `days_in_month` against `chrono` over years `1900..=2200` (inline in [`src/types.rs`](src/types.rs)).
- `parser_parse_proptest.rs` — random bytes (lossy UTF-8) fed to `parse()` (panic hunt).
- `builtin_catalog_consistency.rs` — every entry in `BUILTIN_FUNCTIONS` is recognized by `eval_function` dispatch.

## Regenerating the FEL function schema

`formspec/schemas/fel-functions.schema.json` is auto-generated from the Rust catalog:

```sh
cargo run --bin emit-fel-schema > ../formspec/schemas/fel-functions.schema.json
```

The round-trip test catches any drift; if it fails, edit `BUILTIN_FUNCTIONS` in `src/extensions/catalog.rs` (the source of truth), not the JSON.

## License

Apache-2.0 — see [LICENSE](LICENSE) and [LICENSING.md](LICENSING.md).

## Project Governance

- Security reports: [SECURITY.md](SECURITY.md)
- Contributions: [CONTRIBUTING.md](CONTRIBUTING.md)
- Conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- Dependency policy: [deny.toml](deny.toml)
