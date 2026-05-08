#!/usr/bin/env node
/** Stdin-to-stdout FEL evaluator using the formspec-engine WASM runtime.
 *
 * Reads JSONL from stdin — one object per line: { "expr": "FEL expression", "fields": { ... } }.
 * Writes JSONL to stdout: { "result": <evaluated value>, "error": null } on success,
 * { "result": null, "error": "message" } on failure.
 *
 * Usage:
 *   echo '{"expr":"$a + $b","fields":{"a":3,"b":4}}' | node scripts/fel-wasm-eval.mjs
 */

import { initFormspecEngine, evalFEL } from '../../formspec/packages/formspec-engine/dist/index.js';
import { createInterface } from 'node:readline';

await initFormspecEngine();

const rl = createInterface({ input: process.stdin });

for await (const line of rl) {
    const trimmed = line.trim();
    if (trimmed === '') continue;
    try {
        const { expr, fields = {} } = JSON.parse(trimmed);
        const result = evalFEL(expr, fields);
        process.stdout.write(JSON.stringify({ result, error: null }) + '\n');
    } catch (err) {
        process.stdout.write(
            JSON.stringify({ result: null, error: String(err) }) + '\n',
        );
    }
}
