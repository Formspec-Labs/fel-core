# fel-core — Formspec Expression Language (parser, evaluator, dependency analysis).
#
# Primary entry point for building and testing this standalone crate.

CARGO = cargo
PYTHON = python3
RUSTUP = rustup
CARGO_FUZZ = $(CARGO) +nightly fuzz
CARGO_FLAGS_ALL_FEATURES = --all-features
CARGO_FLAGS_PROPTEST_FEATURES = --features proptest-strategies
RUST_TRIPLE = $(shell rustc -Vv | sed -n 's/^host: //p')
RUSTUP_HOME ?= $(HOME)/.rustup
NIGHTLY_LLVM_PROFDATA = $(RUSTUP_HOME)/toolchains/nightly-$(RUST_TRIPLE)/lib/rustlib/$(RUST_TRIPLE)/bin/llvm-profdata

.PHONY: all help build test test-full test-differential test-differential-python test-differential-wasm test-all check-ratification ratify ratify-external conformance lint deny docs package ci fuzz-extract fuzz-regression-refresh fuzz-setup fuzz-coverage fuzz-all seed-fuzz clean \
        mutants-install mutants-p0 mutants-parser mutants-lexer mutants-evaluator mutants-budget mutants-deps mutants-convert mutants-error mutants-prepare-host mutants-extensions mutants-money-dates \
        mutants-shard-1 mutants-shard-2 mutants-shard-3 mutants-shard-4

all: build

help:
	@echo "fel-core Makefile"
	@echo ""
	@echo "  make build                       — cargo build"
	@echo "  make test                        — cargo nextest run --all-features"
	@echo "  make test-full                   — cargo test --all-features"
	@echo "  make test-all                    — run all test targets"
	@echo "  make check-ratification          — validate spec/conformance ratification artifacts"
	@echo "  make lint                        — rustfmt + clippy -D warnings"
	@echo "  make deny                        — cargo-deny advisories/license/source policy"
	@echo "  make docs                        — regenerate rustdoc Markdown mirror"
	@echo "  make package                     — verify crates.io package contents"
	@echo "  make ci                          — local OSS-readiness gate"
	@echo "  make ratify                      — local candidate-ratification gate"
	@echo "  make ratify-external             — cross-runtime differential implementation gate"
	@echo "  make clean                       — cargo clean"
	@echo "  make test-differential           — run cross-runtime oracle (Python + WASM)"
	@echo "  make test-differential-python    — Rust↔Python oracle only"
	@echo "  make test-differential-wasm      — Rust↔WASM oracle only"
	@echo "  make conformance                 — generate conformance/fel-conformance.jsonl"
	@echo "  make fuzz-extract                — append libFuzzer artifacts to fuzz_regression.jsonl"
	@echo "  make fuzz-regression-refresh    — re-emit mustParse/displayOracle for entire JSONL corpus"
	@echo "  make fuzz-setup                  — install nightly + llvm-tools + cargo-fuzz"
	@echo "  make fuzz-coverage               — generate HTML coverage from fuzz corpus"
	@echo "  make fuzz-all                    — run all fuzz maintenance targets"
	@echo "  make seed-fuzz                   — seed fuzz corpus (FORMSPEC_ROOT for monorepo layout)"
	@echo "  make mutants-install             — install cargo-mutants $(CARGO_MUTANTS_VERSION) (pinned)"
	@echo "  make mutants-p0                  — sequential mutation gate on all P0 seams (~6h)"
	@echo "  make mutants-shard-{1..4}        — sharded mutation gate (CI parallelism)"
	@echo "  make mutants-<file>              — per-file mutation (parser/lexer/evaluator/budget/deps/"
	@echo "                                     convert/error/prepare-host/extensions/money-dates)"
	@echo ""

build:
	$(CARGO) build

test:
	$(CARGO) nextest run $(CARGO_FLAGS_ALL_FEATURES)

test-full:
	$(CARGO) test $(CARGO_FLAGS_ALL_FEATURES)

lint:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --all-targets --all-features -- -D warnings

deny:
	$(CARGO) deny check

test-differential: test-differential-python test-differential-wasm

test-differential-python:
	$(CARGO) test $(CARGO_FLAGS_PROPTEST_FEATURES) --test differential_oracle rust_python_parity -- --ignored --test-threads=1
	$(CARGO) test $(CARGO_FLAGS_PROPTEST_FEATURES) --test differential_oracle r24_power_oracle::power_fractional_negative_rust_python_parity -- --ignored --test-threads=1

test-differential-wasm:
	$(CARGO) test $(CARGO_FLAGS_PROPTEST_FEATURES) --test differential_oracle rust_wasm_parity -- --ignored --test-threads=1

test-all: test-full test-differential

check-ratification:
	$(PYTHON) scripts/check-ratification.py --verify-generated

ratify: check-ratification test-full
	RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" $(CARGO) doc --no-deps

ratify-external: test-differential

docs:
	npm run docs:fel-core

package:
	$(CARGO) package --allow-dirty

ci: lint deny ratify package

conformance:
	@mkdir -p conformance
	$(CARGO) run $(CARGO_FLAGS_PROPTEST_FEATURES) --bin emit-conformance-fixtures -- 200 > conformance/fel-conformance.jsonl

## ── Mutation testing (Phase 2 of the test triage plan) ──────────
##
## See thoughts/2026-05-23-test-suite-triage.md §"Phase 2 — Mutation
## gate on critical seams" and `.cargo/mutants.toml`.
##
## Tool pinned: cargo-mutants 25.3.1. Install with `make mutants-install`.
## Per-file targets scope test execution to the relevant test binaries —
## running cargo-mutants with no scoping incurs the full ~30-binary suite
## per mutant (intractable).
##
## Determinism: PROPTEST_CASES is lowered and a fixed RNG seed is pinned
## so the mutation baseline is reproducible across reruns.

CARGO_MUTANTS_VERSION = 25.3.1
MUTANTS_ENV = PROPTEST_CASES=64 PROPTEST_RNG_SEED=fel-core-mutants-v1
MUTANTS_COMMON_ARGS = --all-features --in-place --no-shuffle --baseline=run

mutants-install:
	@if cargo mutants --version 2>/dev/null | grep -q "$(CARGO_MUTANTS_VERSION)"; then \
	  echo "cargo-mutants $(CARGO_MUTANTS_VERSION) already installed"; \
	else \
	  $(CARGO) install cargo-mutants --version $(CARGO_MUTANTS_VERSION) --locked; \
	fi

# Per-file mutation targets. Each scopes test execution to the binaries that
# actually exercise the mutated file. Add files here as new P0 seams emerge.
mutants-parser:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/parser.rs' \
	  -- --lib --test parser_rejection_tests --test parser_parse_proptest --test ast_proptest --test lexer_tests

mutants-lexer:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/lexer.rs' \
	  -- --lib --test lexer_tests --test parser_parse_proptest --test fel_chaos_proptest

mutants-evaluator:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/evaluator/core.rs' \
	  -- --lib --test evaluator_tests --test semantic_invariants --test decimal_properties \
	     --test evaluator_regression_guards --test fuzz_regression_corpus

mutants-budget:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/evaluator/budget.rs' \
	  -- --lib --test budget_tests --test stress_tests

mutants-deps:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/dependencies.rs' \
	  -- --lib --test environment_integration_tests --test evaluator_tests

mutants-convert:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/convert.rs' \
	  -- --lib --test evaluator_tests --test fel_proptest --test decimal_properties --test schema_round_trip

mutants-error:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/error.rs' \
	  -- --lib --test evaluator_tests --test snapshot_tests --test builtin_catalog_consistency

mutants-prepare-host:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/prepare_host.rs' \
	  -- --lib --test host_bindings --test evaluator_tests

mutants-extensions:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/extensions/registry.rs' --file 'src/extensions/catalog.rs' \
	  -- --lib --test builtin_catalog_consistency --test evaluator_tests --test host_bindings --test function_semantics_conformance

mutants-money-dates:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/evaluator/builtins/money.rs' --file 'src/evaluator/builtins/dates.rs' \
	  -- --lib --test evaluator_tests --test locale_fel_functions

# Run all P0 seams sequentially. Slow (~6 hours total); use mutants-shard-* in CI.
mutants-p0: mutants-parser mutants-lexer mutants-evaluator mutants-budget mutants-deps \
            mutants-convert mutants-error mutants-prepare-host mutants-extensions mutants-money-dates

# Sharded entry points for CI (4-way parallelism). Shard membership is
# tuned by mutant count: each shard ≈ 250-280 mutants.
mutants-shard-1: mutants-evaluator
mutants-shard-2: mutants-prepare-host mutants-lexer
mutants-shard-3: mutants-parser mutants-money-dates
mutants-shard-4: mutants-budget mutants-deps mutants-convert mutants-error mutants-extensions

fuzz-extract:
	$(PYTHON) scripts/fuzz_to_regression.py

fuzz-regression-refresh:
	@tmp=$$(mktemp) && \
	$(CARGO) run --bin emit-fuzz-regression-corpus < tests/corpus/fuzz_regression.jsonl > "$$tmp" && \
	mv "$$tmp" tests/corpus/fuzz_regression.jsonl

fuzz-setup:
	@command -v $(RUSTUP) >/dev/null 2>&1 || (echo "rustup is required for fuzz tooling setup." && exit 1)
	$(RUSTUP) toolchain install nightly
	$(RUSTUP) component add llvm-tools-preview --toolchain nightly
	@command -v cargo-fuzz >/dev/null 2>&1 || $(CARGO) install cargo-fuzz

fuzz-coverage: fuzz-setup
	@test -x "$(NIGHTLY_LLVM_PROFDATA)" || (echo "missing llvm-profdata at $(NIGHTLY_LLVM_PROFDATA)" && exit 1)
	$(CARGO_FUZZ) coverage fel_pipeline
	$(CARGO_FUZZ) coverage fel_structured

fuzz-all: seed-fuzz fuzz-coverage fuzz-extract

seed-fuzz:
	@echo "Copying conformance suite expressions into fuzz corpus..."
	$(PYTHON) scripts/seed_fuzz_corpus.py

clean:
	$(CARGO) clean
