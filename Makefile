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
NIGHTLY_LLVM_PROFDATA = $(HOME)/.rustup/toolchains/nightly-$(RUST_TRIPLE)/lib/rustlib/$(RUST_TRIPLE)/bin/llvm-profdata

.PHONY: all help build test test-full test-differential test-differential-python test-differential-wasm test-all emit-fixtures conformance fuzz-extract fuzz-setup fuzz-coverage fuzz-all seed-fuzz clean

all: build

help:
	@echo "fel-core Makefile"
	@echo ""
	@echo "  make build                       — cargo build"
	@echo "  make test                        — cargo nextest run --all-features"
	@echo "  make test-full                   — cargo test --all-features"
	@echo "  make test-all                    — run all test targets"
	@echo "  make clean                       — cargo clean"
	@echo "  make test-differential           — run cross-runtime oracle (Python + WASM)"
	@echo "  make test-differential-python    — Rust↔Python oracle only"
	@echo "  make test-differential-wasm      — Rust↔WASM oracle only"
	@echo "  make emit-fixtures               — emit conformance fixtures JSONL"
	@echo "  make conformance                 — generate conformance/fel-conformance.jsonl"
	@echo "  make fuzz-extract                — convert fuzz corpus to regression tests"
	@echo "  make fuzz-setup                  — install nightly + llvm-tools + cargo-fuzz"
	@echo "  make fuzz-coverage               — generate HTML coverage from fuzz corpus"
	@echo "  make fuzz-all                    — run all fuzz maintenance targets"
	@echo "  make seed-fuzz                   — seed fuzz corpus from conformance suite"
	@echo ""

build:
	$(CARGO) build

test:
	$(CARGO) nextest run $(CARGO_FLAGS_ALL_FEATURES)

test-full:
	$(CARGO) test $(CARGO_FLAGS_ALL_FEATURES)

test-differential: test-differential-python test-differential-wasm

test-differential-python:
	$(CARGO) test $(CARGO_FLAGS_PROPTEST_FEATURES) --test differential_oracle rust_python_parity -- --ignored --test-threads=1
	$(CARGO) test $(CARGO_FLAGS_PROPTEST_FEATURES) --test differential_oracle r24_power_oracle::power_fractional_negative_rust_python_parity -- --ignored --test-threads=1

test-differential-wasm:
	$(CARGO) test $(CARGO_FLAGS_PROPTEST_FEATURES) --test differential_oracle rust_wasm_parity -- --ignored --test-threads=1

test-all: test-full test-differential

emit-fixtures:
	@mkdir -p tests/fixtures
	$(CARGO) run $(CARGO_FLAGS_PROPTEST_FEATURES) --bin emit-conformance-fixtures -- 256 > tests/fixtures/conformance.jsonl

conformance:
	@mkdir -p conformance
	$(CARGO) run $(CARGO_FLAGS_PROPTEST_FEATURES) --bin emit-conformance-fixtures -- 200 > conformance/fel-conformance.jsonl

fuzz-extract:
	$(PYTHON) scripts/fuzz_to_regression.py

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
