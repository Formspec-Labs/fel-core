# fel-core fuzz targets

Compile-only check (no extra tooling): from `fel-core/fuzz`, run `cargo build --release`.

LibFuzzer runs require [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) and the **nightly** toolchain (stable fails on macOS with sanitizer flags such as `-Zsanitizer=address`):

```bash
cargo install cargo-fuzz
rustup toolchain install nightly   # if needed
```

From this directory (`fel-core/fuzz`):

```bash
cargo +nightly fuzz run fel_pipeline -- -runs=10000
cargo +nightly fuzz run fel_structured -- -runs=10000
```

- **`fel_pipeline`**: arbitrary bytes interpreted as lossy UTF-8 → `tokenize` → `parse` → `evaluate` → `print_expr` + re-parse (no panics).
- **`fel_structured`**: first byte picks a small catalog of seed snippets; remaining bytes append as lossy UTF-8 tail (grammar-biased mutations).

`fuzz/corpus/<target>/` is gitignored; populate it locally by running the targets (LibFuzzer writes evolved inputs there).

Ignore `fuzz/artifacts/` (generated crashes; listed in `fel-core/.gitignore`).
