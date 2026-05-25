# fs-9quq scoping — Developer debugging console

Ticket: [`fs-9quq`](../../tk) FORMSPEC-JOURNEY-DEBUG-001 · P3 · open · parent `fs-lgw3` (open journey gaps).
Source row: [`formspec/TODO.md:997`](../../formspec/TODO.md) — FEATURE-MATRIX §16.13.

## 1. What "developer debugging console" probably means

The ticket frames it as "Chrome DevTools for form logic" and names four surfaces in the AC:

1. Trace tree view (FEL eval trace per expression)
2. Expression breakpoints
3. Validation report browser
4. Event replay stepper

The implied home in the AC is "Studio package `formspec-devtools`" with Playwright E2E — i.e. a **browser UI in formspec-studio**. That framing leans (b) browser REPL/explorer.

The **conceptual nugget** (one sentence): authors and integrators can't see *why* a FEL expression produced its value; raw JSON traces ship but no viewer renders them. The ticket is a UX gap on existing data structures, not a missing capability. Everything the ticket needs already exists at the data layer:

- FEL eval trace — `fel_core::Trace`/`TraceStep` ([`src/trace.rs`](../src/trace.rs)) wired through WASM as `evalFELWithTrace` ([`formspec/crates/formspec-wasm/src/fel.rs:85`](../../formspec/crates/formspec-wasm/src/fel.rs)), already round-tripped by `formspec-engine` ([`tests/fel-trace.test.mjs`](../../formspec/packages/formspec-engine/tests/fel-trace.test.mjs)).
- Validation report — `ValidationReport` shape with paths, severities, codes (formspec/core spec §4.3).
- Event replay — RawProject / engine command/event stream.
- Diagnostics — `Diagnostic { severity, message, code, kind, span }` ([`src/error.rs:61`](../src/error.rs)).

So the work is **presentation over existing data**, not new primitives.

## 2. What already exists

In `fel-core`:

- `Trace` / 5 `TraceStep` variants (FieldResolved, FunctionCalled, BinaryOp, IfBranch, ShortCircuit) — opt-in via `EvaluatorOptions::trace`.
- `Diagnostic` with stable `code` + `kind` + source `span` for highlight rendering.
- `print_expr` for AST round-trip.
- `extract_dependencies` for "what does this expression depend on?"
- Three existing `bin/` targets, all emit-*: conformance fixtures, schema, fuzz corpus. No interactive bin. No `clap` dep.

In `formspec/`:

- WASM bridge: `evalFEL`, `evalFELWithTrace`, `evalFELWithContext`, `analyzeFEL`, `getFELDependencies`, `tokenizeFEL`, `printFEL`, `parseFEL`.
- `formspec-engine` re-exports the bridge functions; consumed by studio and tests.
- 251 conformance fixtures in [`fel-core/conformance/fel-conformance.jsonl`](../conformance/fel-conformance.jsonl) — ready-made input corpus for any debug surface.

In `formspec-studio/`:

- `trace-index.ts` / `trace-digest.ts` / `formspec_trace` MCP tool — **different "trace"**: source-artifact provenance (Trace spec §6), not FEL eval traces. Reusing the name in `formspec-devtools` would collide.
- No FEL eval trace consumer anywhere in studio packages today.
- `formspec-studio` is a Vite-built lit-html visual designer; no devtools sub-package.

Net: **the data layer is complete; the presentation layer is empty.** Zero TS code consumes `TraceStep`, even though the bridge ships it.

## 3. Three implementation options

### Option A — `fel` CLI in `fel-core/src/bin/fel.rs` (S, ~1 day)

Standalone Rust binary. `fel eval '<expr>' --fields '{"a":3}' --trace` prints value, diagnostics, and an indented trace tree to stderr. Reads expressions from argv, stdin, or a fixture file (reuse the conformance JSONL). Optional `--json` flag emits raw envelope for piping.

- **Cost:** S. New file. Add `clap` (already common across stack). Tests = nextest assertions on stdout/stderr. No new spec surface, no WASM, no JS.
- **Reach:** consultant integrators debugging FEL expressions in `cargo`-aware terminal sessions. Conformance-suite authors. Anyone bisecting a fixture failure. **Does not reach** form authors in Studio.
- **User value:** unblocks the #1 friction point ("why isn't this expression working?") for the audience that owns the spec — internal contributors, integrator engineers. Zero ceremony, fast feedback. Equivalent of `jq` / `cue eval` for FEL.

### Option B — CLI + minimal JSON-source web viewer (M, ~1 week)

Option A, plus a tiny static `formspec-devtools` viewer in `formspec-studio/packages/formspec-devtools/` that takes a JSON envelope (paste or file-load) and renders trace tree + diagnostics with source-span highlighting. **No Studio integration, no engine handle.** Pure source-in / view-out.

- **Cost:** M. New package; lit-html or Preact (match studio); one Playwright E2E.
- **Reach:** Option A's audience plus authors who can copy/paste a trace from a chat tool or DevTools console. Still requires the user to obtain the JSON envelope somehow.
- **User value:** opens the door for authors but the data-acquisition step is awkward. Probably useful as a teaching/docs surface (embed in formspec-site fixture examples).

### Option C — full DevtoolsContract + integrated Studio panel (L, ~1-2 months)

Everything in the ticket AC: trace tree, expression breakpoints, validation browser, event replay stepper, all wired into Studio's live engine handle. Spec the `DevtoolsContract` (Studio↔engine inspection protocol — breakpoint registration, step granularity, event-stream replay control). Build the inspector panel that consumes the contract. Three Playwright E2Es.

- **Cost:** L. Breakpoints require new evaluator instrumentation (pause/resume seam) — net new feature on `fel-core`, not just a viewer. Event replay needs a stable engine event stream contract. Studio panel is real UI design work, not a debug helper.
- **Reach:** form authors, integrator engineers, support — everyone who touches a live form.
- **User value:** highest absolute. Also highest debt — breakpoints in particular are a contract decision that should not be made under "P3 sustaining" pressure; once shipped, every future evaluator change has to honor the pause/resume seam.

## 4. Recommendation

**Option A** (the `fel` CLI), but **explicitly framed as the seam for a future B/C**, not as the end of the work. The CLI emits the canonical debug envelope shape — `{ expr, fields, value, diagnostics, trace }` with stable JSON keys. Any later browser viewer (B) consumes the same envelope as paste-target. Any later integrated devtools (C) reuses the trace projection logic by importing the same emit code into `formspec-engine`.

One-sentence reason: the conceptual nugget is "make the existing data legible" and Option A delivers that to the audience that ships the spec today — at S cost, with no new evaluator features, with zero risk of locking in a Studio contract the stack doesn't yet need.

**Should this gate 1.0?** No. The MVP path in [`GOAL.md`](../../GOAL.md) is wos-server + case-portal + governed-case-flow. A FEL debug console is not on that path. Option A is small enough to ship opportunistically alongside other `fel-core` work; B and C should not be released-gated and **C should be deferred to post-1.0 explicitly** — the right home for an integrated devtools panel is `formspec-studio` after Studio itself stabilizes, not before.

## 5. Vertical slice if greenlit (Option A only)

Minimal proof-of-seam, single PR:

1. Add `clap = { version = "...", features = ["derive"] }` to `fel-core/Cargo.toml` under `[features]` `cli`, default off.
2. `src/bin/fel.rs` exposes one subcommand: `eval`. Accepts `<EXPR>`, `--fields <JSON>`, `--trace`, `--json`. No other subcommands in the first slice — `parse`, `tokenize`, `deps`, `analyze` are obvious follow-ups but excluded from v0.
3. Stable JSON envelope shape (stamped into a fixture in `conformance/` so the shape is testable cross-runtime):

```json
{
  "expression": "<source>",
  "fields": { },
  "value": <ui-projection>,
  "hasErrorDiagnostics": <bool>,
  "diagnostics": [ ],
  "trace": [ { "kind": "FieldResolved", ... } ]
}
```

4. Two `cargo nextest` integration tests: (a) `eval '1 + 2'` returns `3`, (b) `eval '$a + $b' --fields '{"a":3,"b":4}' --trace` emits a trace with 3 steps.
5. `make build` builds the binary; `make test` runs the integration tests. README adds a "Debugging FEL expressions" section pointing at the binary.

That's the seam. Everything else (more subcommands, fixture-replay mode, source-span pretty-printing, color, REPL loop) is an additive follow-up that doesn't change the contract.

## 6. What this is NOT

- **NOT a stack-wide formspec-studio replacement** — Studio remains the visual designer; the CLI is a debugging adjunct.
- **NOT a new FEL feature** — no new evaluator semantics, no breakpoint seam, no event-replay protocol. Read-only over existing structured outputs.
- **NOT the formspec-devtools npm package the ticket AC names** — that's Option B/C territory; the CLI ships first because it's where the data already exists and the audience already lives.
- **NOT a 1.0 release gate** — GOAL.md does not require it; do not block the MVP closeout on this work.
- **NOT a multi-runtime debug protocol** — Python `formspec` already has its own validators and Rust IS the byte authority; cross-runtime debug parity is out of scope until someone asks for it with a concrete user.
- **NOT Studio-trace (provenance) related** — `formspec-studio` already ships `trace-index`/`trace-digest` for source-artifact provenance per the Trace spec. Different concept; different audience. If implemented, the `fel` CLI must use a distinct vocabulary (`fel-trace` / `eval-trace`) to avoid collision.
- **NOT a breakpoint debugger** — breakpoints in the AC require evaluator pause/resume instrumentation. That is a real feature with real debt; it stays deferred.

## Closing

The ticket as written conflates "make existing trace data legible" (S, ready) with "Chrome DevTools for forms" (L, not ready). The honest answer is to ship the small thing under the existing ticket, leave the ticket open with the larger scope explicitly deferred, and re-evaluate B/C post-1.0 when Studio's own shape has settled.
