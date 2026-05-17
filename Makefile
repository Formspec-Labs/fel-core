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

.PHONY: all help build test test-full test-differential test-differential-python test-differential-wasm test-all check-ratification ratify ratify-external conformance lint deny docs package ci fuzz-extract fuzz-regression-refresh fuzz-setup fuzz-coverage fuzz-all seed-fuzz clean

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
	@echo "  make seed-fuzz                   — seed fuzz corpus from conformance suite"
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
