# fel-core — Formspec Expression Language (parser, evaluator, dependency analysis).
#
# Primary entry point for building and testing this standalone crate.

CARGO = cargo

.PHONY: all help build test clean

all: build

help:
	@echo "fel-core Makefile"
	@echo ""
	@echo "  make build   — cargo build"
	@echo "  make test    — cargo nextest run"
	@echo "  make clean   — cargo clean"
	@echo ""

build:
	$(CARGO) build

test:
	$(CARGO) nextest run

clean:
	$(CARGO) clean
