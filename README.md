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

This crate **is** the FEL spec source of truth: `formspec/schemas/fel-functions.schema.json` is generated from `BUILTIN_FUNCTIONS` via `cargo run --bin emit-fel-schema`. TypeScript and Python implementations conform to that schema; a round-trip test (`tests/schema_round_trip.rs`) keeps emission and the canonical schema in lock-step.

## Architecture

**Authoritative API detail** is `cargo doc -p fel-core --no-deps`. A **single-file Markdown mirror** of the same rustdoc (for editors and LLM context) is [`docs/rustdoc-md/API.md`](docs/rustdoc-md/API.md), produced by [cargo-doc-md](https://github.com/Crazytieguy/cargo-doc-md) plus `scripts/bundle-rustdoc-md.mjs`.

### Pipeline

```text
Source string
    → Lexer::tokenize → Vec<SpannedToken>
    → Parser::parse_expression → Expr (AST)
    → Evaluator::eval (or extract_dependencies for static analysis)
    → Value + Vec<Diagnostic>
```

- **Parse errors** surface as `Error::Parse`.
- **Eval errors** produce `Diagnostic` entries and a null or partial value (non-fatal evaluation path).

### Source modules (`src/`)

| File | Responsibility |
|------|----------------|
| `lib.rs` | Crate root, `#![deny(missing_docs)]` + `#![warn(clippy::missing_docs_in_private_items)]`, re-exports, `tokenize` / JSON helpers for FFI. |
| `lexer.rs` | `Token`, `Lexer`, spans. |
| `parser.rs` | `parse()`, precedence per module doc comment. |
| `ast.rs` | `Expr`, `PathSegment`, operators. |
| `evaluator/` | `evaluator/core.rs` — `Environment`, `MapEnvironment`, `Evaluator`, `evaluate()`, `eval_function` dispatch, broadcasting, null propagation. `evaluator/util.rs` — free helpers (decimal, plural rules, locale). `evaluator/builtins/{aggregates,strings,numeric,dates,money,logic_types}.rs` — per-domain builtin impls on `Evaluator`. |
| `types.rs` | `Value`, `Date`, `Money`, literals, Hinnant `days_from_civil` / `civil_from_days` (epoch 1970-01-01). |
| `error.rs` | `Error`, `Diagnostic`, `Severity`. |
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
cargo doc -p fel-core --no-deps --open
```

Public-doc enforcement is on by default (`#![deny(missing_docs)]` at the crate root). The `.github/workflows/doc.yml` CI gate additionally runs:

```bash
RUSTDOCFLAGS='-D rustdoc::broken-intra-doc-links' cargo doc -p fel-core --no-deps
```

### Markdown export (from doc comments)

One-time install:

```bash
cargo install cargo-doc-md
```

Regenerate HTML + bundled Markdown:

```bash
npm run docs:fel-core
```

This runs `cargo doc-md` into `target/doc-md-fel-core`, `scripts/bundle-rustdoc-md.mjs` → `docs/rustdoc-md/API.md`, then `cargo doc -p fel-core --no-deps`.

## Internal (private) documentation

Public docs are denied (`#![deny(missing_docs)]`); private items are warned but selectively allowed inside heavyweight internal pipelines.

- **Narrative + allow** — `lexer`, `parser`, `dependencies`, `environment`, `extensions`, `printer`, `context_json`, and each `evaluator/` submodule extend the module `//!` with a short internal overview and use `#![allow(clippy::missing_docs_in_private_items)]` so internals are not each annotated with `///`.
- **Tests** — Each `#[cfg(test)] mod tests` uses `#![allow(clippy::missing_docs_in_private_items)]` for helper fns.

Strict check (lint only this crate):

```bash
cargo clippy -p fel-core --no-deps -- -D clippy::missing_docs_in_private_items
```

## Tests

```bash
cargo nextest run -p fel-core
```

Integration-style suites live under `tests/`. Notable:

- `schema_round_trip.rs` — `emit_schema_json()` byte-equals the canonical `formspec/schemas/fel-functions.schema.json`.
- `civil_calendar_proptest.rs` — proptest harness validates Hinnant `days_from_civil` / `civil_from_days` / `days_in_month` against `chrono` over years `1900..=2200`.
- `builtin_catalog_consistency.rs` — every entry in `BUILTIN_FUNCTIONS` is recognized by `eval_function` dispatch.

## Regenerating the FEL function schema

`formspec/schemas/fel-functions.schema.json` is auto-generated from the Rust catalog:

```bash
cargo run -p fel-core --bin emit-fel-schema > ../formspec/schemas/fel-functions.schema.json
```

The round-trip test catches any drift; if it fails, edit `BUILTIN_FUNCTIONS` in `src/extensions/catalog.rs` (the source of truth), not the JSON.

## License

Apache-2.0 — see [LICENSE](../../LICENSE) and [LICENSING.md](../../LICENSING.md).
