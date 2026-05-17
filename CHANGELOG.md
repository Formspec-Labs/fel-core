# Changelog

All notable changes to `fel-core` will be documented in this file.

This project follows semantic commit messages. The Rust crate is pre-1.0, while
the FEL language semantics are tracked through the ratified specification and
conformance corpus.

## [Unreleased]

### Added
- **OSS Readiness**: Finalized public project governance files (LICENSE, SECURITY.md, CONTRIBUTING.md, CODE_OF_CONDUCT.md).
- **Documentation Tooling**: Integrated `cargo-doc-md` to produce the single-file `docs/rustdoc-md/API.md` mirror.

## [0.1.0] - 2026-05-17

### Added
- **Locale Ratification**: Ratified `formatNumber()` and `formatDate()` as full FEL 1.0 features with deterministic cross-runtime behavior.
- **Internal Ratification Pass**: Established `scripts/check-ratification.py` and `make ratify` gates to ensure specification, conformance, and implementation stay in lock-step.
- **Implementation Report**: Created `conformance/IMPLEMENTATION-REPORT.md` to track conformance class coverage across Rust, Python, and WASM runtimes.
- **JSON Specification**: Formally specified public/result JSON versus typed wire JSON behavior in `docs/SPEC.md`.

### Changed
- **Precision Tightening**: `fel_to_json` and `fel_to_wire_json` now emit decimal strings for unsafe integers and imprecise fractional values to ensure exact machine interchange.
- **WASM Parity**: Updated WASM differential helper to compare raw JSON, avoiding Node-side precision loss during testing.

## [0.0.3] - 2026-05-08

### Added
- **Multi-lens Review Follow-ups**: Addressed R21–R41 findings from deep-dive architectural review.
- **Evaluator Options**: Introduced `EvaluatorOptions` parameter-object to collapse multiple `evaluate*` entry points into a clean, extensible API.
- **TS/WASM Oracle**: Added a Node-based differential oracle to complement the Python oracle, ensuring parity with the TypeScript engine.
- **Normative SPEC**: Created `docs/SPEC.md` as the authoritative source for FEL evaluation rules and budget contracts.

### Changed
- **Allocation Safety**: Introduced `Evaluator::make_{string,array,object}()` helpers; migrated all heap-producing evaluation sites to prevent budget bypasses.
- **Budget API**: Renamed `EvalBudget::tiny()` to `min_viable()` and added fluent constructors for batch and interactive use cases.

### Fixed
- **Budget Redundancy**: Eliminated duplicate "budget exceeded" diagnostics on recursive evaluation frames (R21).
- **Concat Tracking**: Fixed bypass where string concatenation (`&`) and extension results skipped the allocation budget (R22, R32).
- **Visibility**: Moved internal calendar primitives (`civil_from_days`, etc.) to `pub(crate)` to hide implementation details (R36).

## [0.0.2] - 2026-05-07

### Added
- **Chaos & Pressure Initiative**: Implementation of C1–C10 test hardening strategies.
- **Resource Budgets**: Implemented `EvalBudget` (step count, allocation ceiling, wall-clock deadline).
- **Property Testing**: Introduced generative AST strategies (`arb_expr`, `arb_value`) and validated fixpoint properties.
- **Fuzzing**: Added `cargo-fuzz` targets for pipeline and structured mutation testing; integrated crash replays into CI.
- **Snapshot Tests**: Integrated `insta` for pinning exact parser and evaluator error prose (C10).

### Fixed
- **Stack Safety**: Enforced evaluator recursion depth limit (128 frames) alongside the existing parser limit (C4).
- **Arithmetic Integrity**: Validated all `Decimal` operations against panic-inducing overflow and division by zero.

## [0.0.1] - 2026-05-06

### Added
- **Structured Diagnostics**: Added `DiagnosticKind` enum for machine-readable errors (e.g., `UndefinedFunction`, `TypeMismatch`).
- **Shared Test Harness**: Consolidated duplicated integration-test logic into `tests/common/mod.rs`.
- **Stress Coverage**: Added `tests/stress_tests.rs` for large-array and deep-lookup regressions.

### Changed
- **Modernized Objects**: Switched `Value::Object` from linear `Vec` to `IndexMap` for O(1) lookup with insertion-order preservation.
- **Currency Type**: Refactored `Money.currency` to use a stack-allocated `CurrencyCode` ([u8; 3]) instead of heap `String`.
- **Visibility Sweep**: Tightened 17 modules to `pub(crate)` and narrowed crate-root re-exports to the stable public surface.

### Fixed
- **Negative Lexing**: Fixed bug where unspaced subtraction like `1-2` was incorrectly lexed as a single negative number (Item 1).
- **Extension Fallback**: Wired the `ExtensionRegistry` into the evaluator; corrected dispatcher to fall through for unknown names (Item 2).
- **Currency Safety**: Prevented `sum()` from silently stripping currency from `Money` values (Item 3).
- **Truthy Alignment**: Aligned `boolean(number)` conversion with `is_truthy()` semantics (Item 4).
- **Null Propagation**: Fixed `length(null)`, `empty(null)`, and `present(null)` to correctly propagate null instead of returning default values (Item 5).
- **JSON Precision**: Fixed precision loss in JSON serialization for non-integer Decimals by using strings (Item 9).
- **Regex Boundaries**: Fixed `prepare_host` regex to skip quoted spans when rewriting qualified group refs (Item 12).
- **Parser Normalization**: Refactored `PostfixAccess` to use a flat `FieldRef` representation in the parser (Item 28).

## [0.0.0] - 2026-05-04

- **Repository Extraction**: Extracted `fel-core` from the Formspec monorepo into a standalone repository.
- **Initial Core**: Reference implementation of the lexer, parser, evaluator, and dependency analysis.
- **License**: Adopted Apache-2.0.
