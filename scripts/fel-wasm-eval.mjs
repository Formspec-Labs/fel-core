#!/usr/bin/env node
/** Stdin-to-stdout FEL evaluator using the formspec-engine WASM runtime.
 *
 * Reads JSONL from stdin — one object per line: { "expr": "FEL expression", "fields": { ... } }.
 * Writes JSONL to stdout: { "resultJson": "<raw JSON>", "error": null } on success,
 * { "result": null, "error": "message" } on failure.
 *
 * The raw JSON string is intentional: parsing inside Node would round large
 * JSON numbers through JavaScript's IEEE-754 number type before the Rust oracle
 * can compare them.
 *
 * Usage:
 *   echo '{"expr":"$a + $b","fields":{"a":3,"b":4}}' | node scripts/fel-wasm-eval.mjs
 *
 * Set `FORMSPEC_ENGINE_PATH` to the formspec repo root when the stack layout
 * differs (see `conformance/README.md`).
 */

import { createRequire } from 'node:module';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const require = createRequire(import.meta.url);
const defaultEngineRoot = path.resolve(
    fileURLToPath(new URL('.', import.meta.url)),
    '../../formspec',
);
const engineRoot = path.resolve(
    process.env.FORMSPEC_ENGINE_PATH ?? defaultEngineRoot,
);
const engineDist = path.join(engineRoot, 'packages/formspec-engine/dist');

const { initFormspecEngine } = require(path.join(engineDist, 'index.js'));
const { getWasmModule } = require(path.join(engineDist, 'wasm-bridge-runtime.js'));

await initFormspecEngine();

const rl = createInterface({ input: process.stdin });

for await (const line of rl) {
    const trimmed = line.trim();
    if (trimmed === '') continue;
    try {
        const { expr, fields = {} } = JSON.parse(trimmed);
        const resultJson = getWasmModule().evalFEL(expr, JSON.stringify(fields));
        process.stdout.write(JSON.stringify({ resultJson, error: null }) + '\n');
    } catch (err) {
        process.stdout.write(
            JSON.stringify({ result: null, error: String(err) }) + '\n',
        );
    }
}
