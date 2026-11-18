#!/bin/sh
# Link a small consumer against the crate and use its public API in WebAssembly.
set -eu
cd "$(dirname "$0")/../.."
target=wasm32-unknown-unknown
build_dir=${CARGO_TARGET_DIR:-target}
export AR_wasm32_unknown_unknown=${AR_wasm32_unknown_unknown:-llvm-ar}
if ! command -v "$AR_wasm32_unknown_unknown" >/dev/null; then
    echo 'Set AR_wasm32_unknown_unknown to the path to llvm-ar.' >&2
    exit 1
fi
cargo fetch --locked --target "$target"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
ln -s "$PWD" "$scratch/fel-core"
cp Cargo.lock conformance/cel/wasm_consumer.rs "$scratch/"
cat > "$scratch/Cargo.toml" <<'TOML'
[package]
name = "fel-source-wasm-check"
version = "0.0.0"
edition = "2024"
[lib]
path = "wasm_consumer.rs"
crate-type = ["cdylib"]
[dependencies]
fel-core = { path = "fel-core", features = ["cel-bridge"] }
TOML
cargo build --offline --manifest-path "$scratch/Cargo.toml" --target "$target" --target-dir "$build_dir"
node --input-type=module - "$build_dir/$target/debug/fel_source_wasm_check.wasm" <<'JS'
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[2]));
assert.equal(instance.exports.check_source_round_trip(), 1);
JS
