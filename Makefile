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

.PHONY: all help build test test-full test-differential test-differential-python test-differential-wasm test-all check-ratification ratify ratify-external conformance lint deny docs package ci fuzz-extract fuzz-regression-refresh fuzz-setup fuzz-coverage fuzz-all fuzz-run fuzz-run-pipeline fuzz-run-structured fuzz-run-budget seed-fuzz clean \
        mutants-install mutants-p0 mutants-parser mutants-lexer mutants-evaluator mutants-budget mutants-deps mutants-convert mutants-error mutants-prepare-host mutants-extensions mutants-money-dates \
        mutants-interpolation mutants-iso-duration mutants-tier2 \
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
	@echo "  make fuzz-run                    — active discovery loop (default fel_pipeline, 5 min)"
	@echo "  make fuzz-run TARGET=… DURATION=…  — override target / time-box"
	@echo "  make fuzz-run-{pipeline,structured,budget} — per-target convenience"
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
##
## Design: every mutant runs the FULL test suite. Cross-cutting suites
## (differential_oracle, public_conformance_corpus, semantic_invariants,
## fel_proptest) catch parser/evaluator/convert mutations that the
## per-binary scoping originally tried in this Makefile would have missed
## as false-positive survivors. Time is irrelevant; we don't run mutation
## frequently — correctness over speed.
##
## Parallelism: cargo-mutants runs `MUTANTS_JOBS` mutants concurrently,
## each in its own scratch tree. Override with `make MUTANTS_JOBS=12 ...`.
##
## Determinism: PROPTEST_CASES is lowered and a fixed RNG seed is pinned
## so the mutation baseline is reproducible across reruns.

CARGO_MUTANTS_VERSION = 25.3.1
# Auto-detect cores for sensible default. Override locally with
# `make MUTANTS_JOBS=N mutants-p0` on smaller hosts.
MUTANTS_CORES ?= $(shell nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)
MUTANTS_JOBS ?= $(shell echo $$(( $(MUTANTS_CORES) / 2 > 0 ? $(MUTANTS_CORES) / 2 : 1 )))
MUTANTS_ENV = PROPTEST_CASES=64 PROPTEST_RNG_SEED=fel-core-mutants-v1
MUTANTS_COMMON_ARGS = --all-features --no-shuffle --baseline=run --jobs $(MUTANTS_JOBS)

mutants-install:
	@if cargo mutants --version 2>/dev/null | grep -q "$(CARGO_MUTANTS_VERSION)"; then \
	  echo "cargo-mutants $(CARGO_MUTANTS_VERSION) already installed"; \
	else \
	  $(CARGO) install cargo-mutants --version $(CARGO_MUTANTS_VERSION) --locked; \
	fi

# Per-file mutation targets. Each runs the full test suite — cross-cutting
# suites are intentionally included for correctness.
mutants-parser:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/parser.rs'

mutants-lexer:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/lexer.rs'

mutants-evaluator:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/evaluator/core.rs'

mutants-budget:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/evaluator/budget.rs'

mutants-deps:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/dependencies.rs'

mutants-convert:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/convert.rs'

mutants-error:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/error.rs'

mutants-prepare-host:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/prepare_host.rs'

mutants-extensions:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/extensions/registry.rs' --file 'src/extensions/catalog.rs'

mutants-money-dates:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/evaluator/builtins/money.rs' --file 'src/evaluator/builtins/dates.rs'

# Tier-2 mutation gate — non-P0 seams. Run via `make mutants-tier2`.
# Add to mutants-p0 explicitly if these graduate to P0 in a future
# review (currently tracked as FUT-6 in the survivor doc).
mutants-interpolation:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/interpolation.rs'

mutants-iso-duration:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --file 'src/iso_duration.rs'

mutants-tier2: mutants-interpolation mutants-iso-duration

# Run all P0 seams in one invocation. cargo-mutants handles parallelism
# internally via --jobs; this is the canonical "run the baseline" entry.
mutants-p0:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) \
	  --file 'src/parser.rs' \
	  --file 'src/lexer.rs' \
	  --file 'src/evaluator/core.rs' \
	  --file 'src/evaluator/budget.rs' \
	  --file 'src/dependencies.rs' \
	  --file 'src/convert.rs' \
	  --file 'src/error.rs' \
	  --file 'src/prepare_host.rs' \
	  --file 'src/extensions/registry.rs' \
	  --file 'src/extensions/catalog.rs' \
	  --file 'src/evaluator/builtins/money.rs' \
	  --file 'src/evaluator/builtins/dates.rs'

# Sharded entry points for CI matrix. cargo-mutants `--shard k/N` splits
# the mutant list into N disjoint slices; each shard still runs all tests
# per mutant in parallel via --jobs.
mutants-shard-1:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --shard 0/4 \
	  --file 'src/parser.rs' --file 'src/lexer.rs' --file 'src/evaluator/core.rs' \
	  --file 'src/evaluator/budget.rs' --file 'src/dependencies.rs' --file 'src/convert.rs' \
	  --file 'src/error.rs' --file 'src/prepare_host.rs' \
	  --file 'src/extensions/registry.rs' --file 'src/extensions/catalog.rs' \
	  --file 'src/evaluator/builtins/money.rs' --file 'src/evaluator/builtins/dates.rs'

mutants-shard-2:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --shard 1/4 \
	  --file 'src/parser.rs' --file 'src/lexer.rs' --file 'src/evaluator/core.rs' \
	  --file 'src/evaluator/budget.rs' --file 'src/dependencies.rs' --file 'src/convert.rs' \
	  --file 'src/error.rs' --file 'src/prepare_host.rs' \
	  --file 'src/extensions/registry.rs' --file 'src/extensions/catalog.rs' \
	  --file 'src/evaluator/builtins/money.rs' --file 'src/evaluator/builtins/dates.rs'

mutants-shard-3:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --shard 2/4 \
	  --file 'src/parser.rs' --file 'src/lexer.rs' --file 'src/evaluator/core.rs' \
	  --file 'src/evaluator/budget.rs' --file 'src/dependencies.rs' --file 'src/convert.rs' \
	  --file 'src/error.rs' --file 'src/prepare_host.rs' \
	  --file 'src/extensions/registry.rs' --file 'src/extensions/catalog.rs' \
	  --file 'src/evaluator/builtins/money.rs' --file 'src/evaluator/builtins/dates.rs'

mutants-shard-4:
	$(MUTANTS_ENV) cargo mutants $(MUTANTS_COMMON_ARGS) --shard 3/4 \
	  --file 'src/parser.rs' --file 'src/lexer.rs' --file 'src/evaluator/core.rs' \
	  --file 'src/evaluator/budget.rs' --file 'src/dependencies.rs' --file 'src/convert.rs' \
	  --file 'src/error.rs' --file 'src/prepare_host.rs' \
	  --file 'src/extensions/registry.rs' --file 'src/extensions/catalog.rs' \
	  --file 'src/evaluator/builtins/money.rs' --file 'src/evaluator/builtins/dates.rs'

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
	$(CARGO_FUZZ) coverage fel_budget

fuzz-all: seed-fuzz fuzz-coverage fuzz-extract

# Active fuzzing (discovery loop). Bounded by DURATION seconds.
#   make fuzz-run                              # fel_pipeline, 5 min
#   make fuzz-run TARGET=fel_budget            # fel_budget,   5 min
#   make fuzz-run TARGET=fel_structured DURATION=600
FUZZ_TARGET   ?= fel_pipeline
FUZZ_DURATION ?= 300
fuzz-run: fuzz-setup
	$(CARGO_FUZZ) run $(FUZZ_TARGET) -- -max_total_time=$(FUZZ_DURATION)

fuzz-run-pipeline: ; @$(MAKE) fuzz-run FUZZ_TARGET=fel_pipeline
fuzz-run-structured: ; @$(MAKE) fuzz-run FUZZ_TARGET=fel_structured
fuzz-run-budget: ; @$(MAKE) fuzz-run FUZZ_TARGET=fel_budget

seed-fuzz:
	@echo "Copying conformance suite expressions into fuzz corpus..."
	$(PYTHON) scripts/seed_fuzz_corpus.py

clean:
	$(CARGO) clean
