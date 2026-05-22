---
title: FEL Host-Supplied Context Bindings Protocol
date: 2026-05-22
status: active
owner: spec-author
related:
  - ../../../formspec/thoughts/plans/2026-05-22-response-actions-spec.md
  - ../../specs/fel/fel-grammar.md
---

# FEL Host-Supplied Context Bindings Protocol

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans. Steps use `- [ ]` syntax.

**Goal:** Formalize the protocol by which a FEL host specification (Response Actions, Mapping, Experience, etc.) declares named context variables that FEL expressions resolve at evaluation time. The protocol uses FEL's existing `ContextRef` grammar (`@name` per fel-grammar §6) — no grammar change. What lands is the normative *declaration discipline* a host spec MUST follow, plus a conformance test asserting an evaluator rejects unbound `@name` references. Closes the FEL-context-undefined finding (Expert MAJOR 4) from the Response Actions plan reviews, and enables every future FEL host (Response Actions today, Mapping/Experience tomorrow) to declare bindings uniformly.

**Architecture:** Strictly additive to `specs/fel/fel-grammar.md` (new normative §6.3 "Host-Supplied Context Bindings", inserted between §6.2 Path Resolution Rules and §7 Conformance). New conformance test under `conformance/` asserting host-binding registration + unbound-ref rejection. No source-level changes to the `fel-core` Rust crate beyond an optional API to register a binding catalog (DI-shaped port — host evaluators plug in). The crate change is small and worth landing now: a `ContextBindingCatalog` trait + a no-op default implementation, exposed via the existing evaluator entry points.

**Tech Stack:** Markdown (W3C-style, BCP-14), `cargo nextest`, conformance JSONL.

**Sequencing:** Spec prose first (the protocol is the source of truth) → Rust trait additions → conformance test → host spec adopters reference the new section. Response Actions §4.2 will cite §6.3 of the FEL grammar; this plan is its precondition.

**Citations:** "FEL §" = `fel-core/specs/fel/fel-grammar.md`. "RA-plan" = `formspec/thoughts/plans/2026-05-22-response-actions-spec.md`.

---

## File Structure

### Created

| Path | Responsibility |
|---|---|
| `conformance/host-bindings/unbound-context-ref.json` | Fixture: expression `@unbound` MUST evaluate to a deterministic error when no host binding registers `unbound`. |
| `conformance/host-bindings/registered-binding.json` | Fixture: expression `@response.field` MUST resolve when the host has registered `response` as an object-typed binding with `field` accessible. |
| `tests/host_bindings.rs` | Integration test for the new `ContextBindingCatalog` trait. |

### Modified

| Path | Why |
|---|---|
| `specs/fel/fel-grammar.md` | Insert §6.3 "Host-Supplied Context Bindings" between §6.2 and §7. Define the protocol: how a host spec declares bindings, how a FEL evaluator MUST treat unregistered names, what binding metadata is required, and the relationship to §6 path-resolution rules. |
| `src/evaluator.rs` (or equivalent crate-internal evaluator module) | Add `ContextBindingCatalog` trait + default no-op `EmptyCatalog`. Existing evaluator entry points gain a generic `catalog: &impl ContextBindingCatalog` parameter (or accept a catalog handle on the evaluator struct). Pure additive — existing callers pass `EmptyCatalog::default()`. |
| `conformance/fel-conformance.jsonl` | Append cases for `@unbound` rejection + `@bound` acceptance via a registered catalog. |
| `conformance/README.md` | Note that host-binding fixtures live under `host-bindings/`. |
| `CHANGELOG.md` | Document §6.3 addition + new public trait. |

### Explicitly NOT in scope

- **Changes to FEL grammar productions.** Existing `ContextRef` rule already supports `@name` and `@name.path`. No new syntax.
- **Implicit binding inference.** A FEL evaluator MUST NOT silently auto-bind names. Host specs declare; the evaluator enforces.
- **Cross-host binding sharing.** Each host evaluator instance owns its catalog. A Mapping evaluator does not see a Response Actions binding catalog. (A future SharedCatalog spec MAY emerge if a real need appears.)
- **Type checking beyond shape contracts.** §6.3 specifies what a host spec MUST document about binding type; static type-checking of FEL expressions remains out of scope per existing grammar §7.

---

## Self-Review Note

- The protocol uses **existing grammar** (`ContextRef`); no parser change. The conformance bar is observable evaluator behavior, not parse-tree shape.
- The trait addition is small and **inverts dependency**: the evaluator depends on an abstract catalog, not on hardcoded binding lists. Greenfield-correct DI: host specs can compose without touching evaluator internals.
- The protocol scales to all FEL hosts (Response Actions, Mapping `@source`/`@target`, future Experience `ActionRef` expressions). The Mapping spec already uses `@source` / `@target` — this plan documents that as the first reference implementation of §6.3 (no Mapping spec change needed; FEL §6.3 codifies what Mapping was already doing).
- Cold-read test: a future agent reading §6.3 alone can produce a conforming Response Actions evaluator without referring to the FEL evaluator source.

---

## Task 1: Author §6.3 "Host-Supplied Context Bindings"

**Files:**
- Modify: `specs/fel/fel-grammar.md`

- [ ] **Step 1: Insert §6.3**

Insert the following block immediately after the existing §6.2 "Path Resolution Rules" and before §7 "Conformance":

```markdown
### 6.3 Host-Supplied Context Bindings

FEL is host-agnostic: the same grammar embeds in Formspec Definitions, Mapping documents, Response Actions documents, Experience documents, and other future host specifications. Each host MAY require additional context that the FEL grammar reserves slot for but does NOT itself enumerate. Such context flows through the `ContextRef` production (§6) using `@name` syntax. This section specifies the **protocol** a host spec MUST follow when declaring such bindings, and what a conformant FEL evaluator MUST do at evaluation time.

#### 6.3.1 Declaration shape

A host specification declaring host-supplied context bindings MUST publish a **binding catalog**: a closed list of identifiers, each entry carrying:

| Field | Required | Description |
|---|---|---|
| `name` | Yes | The identifier following `@`. MUST satisfy the `Identifier` lexical rule (§3.2). MUST NOT collide with grammar-reserved context names (`current`, `index`, `count`, `instance`, `source`, `target`). |
| `kind` | Yes | One of `value`, `object`, `function`. Determines path access semantics (§6.3.4). |
| `type` | Yes | Informative human-readable type contract (e.g., "object with fields `id`, `attempt`", "datetime", "function() -> datetime"). Not normatively type-checked by FEL evaluators. |
| `purity` | Yes | One of `pure` (deterministic over the bound value) or `impure` (evaluation MAY produce a different value across invocations even within a single expression evaluation, e.g., a `now()` clock read). |
| `evaluationTiming` | Yes | One of `eager` (resolved once before expression evaluation begins) or `lazy` (resolved on demand at the access site). |
| `scope` | Yes | One of `expression` (bound for the entire expression) or `subexpression` (bound only within a specific scope; rare; reserved for future use). |

Host specs MUST publish the catalog in normative prose adjacent to wherever the FEL expression is declared. The catalog MUST be closed: a FEL evaluator MUST reject any `@name` whose `name` is not registered.

#### 6.3.2 Evaluator obligations

A conformant FEL evaluator that supports host-supplied bindings:

1. **MUST** accept a host-supplied binding catalog at the start of evaluation. The crate-level API for accepting the catalog is implementation-defined; the conformance requirement is observable behavior, not API shape.
2. **MUST** reject a `@name` reference whose `name` is not in the active catalog AND is not a grammar-reserved context name (§6 ContextRef list). Rejection is an evaluation error (NOT a parse error — the expression parsed correctly; only resolution failed).
3. **MUST** resolve `@name` references according to the catalog entry's `kind`:
   - `value`: `@name` returns the bound scalar. `@name.suffix` is an evaluation error.
   - `object`: `@name` returns the root bound object. The evaluator (NOT the catalog) traverses dot-segments per §6.2 path-resolution rules. The catalog MUST NOT short-circuit traversal; this keeps traversal-error behavior uniform across implementations.
   - `function`: `@name(args…)` invokes the bound function. Bare `@name` (no call) is an evaluation error.
4. **MUST** honor `evaluationTiming`:
   - `eager`: the binding's root value is resolved once at the start of expression evaluation.
   - `lazy`: the binding's root value is resolved on first access within the expression. For `object`-kind lazy bindings, the ROOT resolves lazily; once materialized, every traversal segment in §6.2 path-resolution applies eagerly against the resolved root. The catalog MUST NOT be re-consulted mid-traversal.
5. **MUST** isolate catalogs across evaluator instances. A catalog supplied to one evaluation MUST NOT leak into another evaluation that did not receive it.

#### 6.3.3 Examples (non-normative)

A Response Actions document declares (per `specs/response-actions/response-actions-spec.md §4.2`):

| name | kind | type | purity | evaluationTiming |
|---|---|---|---|---|
| `response` | object | Current Response snapshot | pure | eager |
| `definition` | object | Pinned Definition | pure | eager |
| `action` | object | `{ id, intent, actor }` | pure | eager |
| `now` | function | `() -> datetime` | impure | lazy |
| `validation` | object | `{ lastReport: ValidationReport | null }` | pure | eager |
| `invocation` | object | `{ id: string, attempt: integer }` | pure | eager |

**Grammar-built-ins are a separate category.** The grammar-reserved context names (`current`, `index`, `count`, `instance`, `source`, `target` per §6.1 ContextRef table) are NOT host-supplied bindings — they are normative parts of the FEL grammar with semantics defined by Formspec Core (for `current` / `index` / `count` / `instance`) and the Mapping spec (for `source` / `target`). Host specs MUST NOT re-declare grammar-built-ins in their §6.3 catalog. A host spec that needs a Mapping-style source/target view MUST use the grammar-built-ins; conversely, a host spec that needs a different semantic (e.g., a Response Actions `@response`) MUST register a separately-named catalog entry.

The Mapping spec does NOT adopt §6.3. Mapping's `@source` / `@target` are spec-owned at grammar level (§6.1) and remain there.

#### 6.3.4 Relationship to §6.2

Host-supplied object bindings interact with §6.2 path-resolution rules as follows:

- §6.2.1 (lexical scope) does NOT apply to host bindings. `@name` is global to the expression; it is not affected by repeatable-group scope.
- §6.2.2 (index bounds) applies to host bindings whose object structure contains arrays.
- §6.2.3 (instance lookup) is for `@instance('name')`, which is grammar-reserved and not subject to §6.3 registration.
- §6.2.4 (chaining) applies: `@response.items[*].amount` is legal when `response` is registered as an `object`-kind binding and the path traverses validly.

#### 6.3.5 Conformance hook

A host spec adopting §6.3 MUST publish:

1. The closed catalog (§6.3.1 fields populated).
2. The relationship between the host's evaluation moments and the binding's `evaluationTiming` (e.g., "Response Actions precondition evaluation reads `response` as an eager snapshot at invocation time").
3. Negative conformance fixtures asserting unbound `@name` references are rejected.

These three are the §6.3 acceptance bar for a host adopter.
```

- [ ] **Step 2: Commit**

```bash
cd fel-core && git add specs/fel/fel-grammar.md
git commit -m "feat(spec): add FEL §6.3 host-supplied context bindings protocol

Formalizes how host specs (Response Actions, Mapping, Experience) declare
@name context variables. Uses existing ContextRef grammar; no parser
change. Evaluator obligation: reject unbound @name as evaluation error."
```

---

## Task 2: Add `ContextBindingCatalog` trait to the evaluator

**Files:**
- Modify: `src/evaluator.rs` (or whichever module owns evaluator entry points; verify by inspection)
- Modify: `src/lib.rs` to re-export the trait

- [ ] **Step 1: Define the trait + default catalog**

Add to the evaluator module:

```rust
/// Host-supplied context binding catalog.
///
/// A FEL evaluator consults this catalog whenever it encounters a
/// `@name` ContextRef whose name is not grammar-reserved
/// (`current`, `index`, `count`, `instance`, `source`, `target`).
/// See fel-grammar.md §6.3.
///
/// Host specs (Response Actions, Experience, future host specs)
/// implement this trait to publish their declared bindings.
/// Mapping does NOT use this trait — its `@source`/`@target` are
/// grammar-built-ins, not host-supplied.
pub trait ContextBindingCatalog {
    /// Returns the bound root value for `name`, or `None` if the name
    /// is not registered. An evaluator MUST treat `None` as an
    /// evaluation error (`EvalError::UnboundContextRef`).
    ///
    /// The catalog returns the ROOT only. Dot-segment traversal after
    /// `@name` is performed by the evaluator per §6.2 path-resolution
    /// rules — the catalog MUST NOT short-circuit traversal. This keeps
    /// traversal-error behavior uniform across implementations.
    fn resolve<'a>(&'a self, name: &str) -> Option<BoundValue<'a>>;
}

/// No-op catalog used when the host supplies no bindings beyond the
/// grammar-reserved set. Existing call sites that did not previously
/// pass a catalog MUST use this.
#[derive(Default, Debug, Clone, Copy)]
pub struct EmptyCatalog;

impl ContextBindingCatalog for EmptyCatalog {
    fn resolve<'a>(&'a self, _name: &str) -> Option<BoundValue<'a>> {
        None
    }
}

/// Borrowed view of a bound value. Variant set MUST cover the FEL value
/// kinds the evaluator already handles internally; pick the exact
/// shape from the existing evaluator's value type.
pub enum BoundValue<'a> {
    Value(crate::Value<'a>),
    Object(&'a crate::Object),
    Function(crate::FunctionHandle<'a>),
}
```

The exact `BoundValue` variants and lifetimes MUST match the existing evaluator's value type. If the evaluator uses an owned value enum, drop the lifetime parameter; do not introduce new lifetimes.

- [ ] **Step 2: Wire catalog into evaluator entry points**

Existing evaluator entry points (likely `evaluate(expr, ctx)` or `Evaluator::new(...)`) gain a catalog parameter. Strict additive option: add a sibling entry that accepts a catalog and have the existing entry forward to it with `EmptyCatalog`. Example shape:

```rust
pub fn evaluate(expr: &Expr, ctx: &Context) -> Result<Value, EvalError> {
    evaluate_with_catalog(expr, ctx, &EmptyCatalog)
}

pub fn evaluate_with_catalog<C: ContextBindingCatalog>(
    expr: &Expr,
    ctx: &Context,
    catalog: &C,
) -> Result<Value, EvalError> {
    /* existing logic; ContextRef branch consults catalog for non-reserved names */
}
```

Internal ContextRef branch: when the name is NOT in the grammar-reserved set, call `catalog.resolve(name, &path_segments)`. `None` becomes `EvalError::UnboundContextRef { name: name.to_owned() }`.

- [ ] **Step 3: Cargo build + nextest**

```bash
cd fel-core && cargo build && cargo nextest run --workspace
```

Expected: build passes; existing tests pass (no behavioral change for callers using `EmptyCatalog`).

- [ ] **Step 4: Commit**

```bash
cd fel-core && git add src/
git commit -m "feat(fel): add ContextBindingCatalog trait + EmptyCatalog default

Inverts evaluator dependency on hardcoded context names. Implements
the §6.3 evaluator obligation: reject unbound @name as EvalError.
Existing call sites unchanged via EmptyCatalog forwarding."
```

---

## Task 3: Conformance fixtures + integration test

**Files:**
- Create: `conformance/host-bindings/unbound-context-ref.json`
- Create: `conformance/host-bindings/registered-binding.json`
- Create: `tests/host_bindings.rs`

- [ ] **Step 1: Fixture — unbound reference rejected**

`unbound-context-ref.json`:

```json
{
  "name": "unbound-context-ref",
  "description": "FEL evaluator MUST reject @unbound when no host catalog registers `unbound`. Pins §6.3.2 evaluator obligation.",
  "expression": "@unbound",
  "context": {},
  "expected": { "error": "UnboundContextRef" }
}
```

- [ ] **Step 2: Fixture — registered binding resolves**

`registered-binding.json`:

```json
{
  "name": "registered-binding",
  "description": "FEL evaluator MUST resolve @response when host catalog registers `response` as an object-kind binding.",
  "expression": "@response.applicantName",
  "catalog": {
    "response": { "kind": "object", "value": { "applicantName": "Alice" } }
  },
  "expected": { "result": "Alice" }
}
```

(`catalog` is a test-harness shape, not normative wire format; the harness in Step 3 maps it to a `ContextBindingCatalog` impl.)

- [ ] **Step 3: Integration test**

`tests/host_bindings.rs`:

```rust
//! §6.3 host-supplied context bindings — conformance assertions.

use fel_core::{evaluate_with_catalog, parse, BoundValue, ContextBindingCatalog, Context, EmptyCatalog};

struct TestCatalog<'a> {
    response: &'a serde_json::Value,
}

impl<'a> ContextBindingCatalog for TestCatalog<'a> {
    fn resolve<'b>(&'b self, name: &str) -> Option<BoundValue<'b>> {
        match name {
            "response" => Some(BoundValue::Object(self.response)),
            _ => None,
        }
    }
}

#[test]
fn unbound_context_ref_is_evaluation_error() {
    let expr = parse("@unbound").unwrap();
    let err = evaluate_with_catalog(&expr, &Context::default(), &EmptyCatalog).unwrap_err();
    assert!(matches!(err, fel_core::EvalError::UnboundContextRef { .. }),
            "unexpected error: {err:?}");
}

#[test]
fn registered_binding_resolves() {
    let response = serde_json::json!({ "applicantName": "Alice" });
    let catalog = TestCatalog { response: &response };
    let expr = parse("@response.applicantName").unwrap();
    let result = evaluate_with_catalog(&expr, &Context::default(), &catalog).unwrap();
    assert_eq!(result.as_str(), Some("Alice"));
}
```

The exact Rust types (`Value`, `Object`, `Context`) MUST match the crate's existing public surface — replace placeholders with the real types during implementation.

- [ ] **Step 4: Run**

```bash
cd fel-core && cargo nextest run --test host_bindings
```

Expected: both tests pass.

- [ ] **Step 5: Commit**

```bash
cd fel-core && git add conformance/host-bindings/ tests/host_bindings.rs
git commit -m "test(fel): conformance + integration tests for §6.3 host bindings

Unbound @name rejected as UnboundContextRef; registered binding
resolves through path traversal. Pins §6.3.2 evaluator obligations."
```

---

## Task 4: Append to fel-conformance.jsonl + README

**Files:**
- Modify: `conformance/fel-conformance.jsonl`
- Modify: `conformance/README.md`

- [ ] **Step 1: Append JSONL entries**

Add two lines (the harness reads JSONL, one fixture per line). Keep matching the existing JSONL shape (inspect the file for the field set before authoring):

```jsonl
{"name": "host-bindings/unbound-context-ref", "category": "host-bindings", "expression": "@unbound", "catalog": {}, "expects": "error:UnboundContextRef"}
{"name": "host-bindings/registered-binding", "category": "host-bindings", "expression": "@response.applicantName", "catalog": {"response": {"kind": "object", "value": {"applicantName": "Alice"}}}, "expects": "result:Alice"}
```

If the existing JSONL doesn't include a `catalog` field, leave the harness extension for a follow-up — the JSON files under `conformance/host-bindings/` already serve the test.

- [ ] **Step 2: Update README**

Add a paragraph under the existing fixture-organization section:

```markdown
### Host-supplied bindings (§6.3)

`conformance/host-bindings/` contains fixtures exercising the host-context binding protocol from `specs/fel/fel-grammar.md §6.3`. Each fixture either registers a catalog or asserts a `@name` rejection when no catalog applies. See `tests/host_bindings.rs` for the harness.
```

- [ ] **Step 3: Commit**

```bash
cd fel-core && git add conformance/
git commit -m "docs(fel): index host-bindings conformance fixtures"
```

---

## Task 5: Changelog + full sweep

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add entry**

Insert at the top of the unreleased section:

```markdown
### Added

- FEL grammar §6.3 "Host-Supplied Context Bindings": normative protocol for host specs (Response Actions, Experience, future host specs) to declare `@name` context variables. Mapping's `@source`/`@target` remain grammar-built-ins per §6.1 and are NOT subject to §6.3.
- `ContextBindingCatalog` public trait + `EmptyCatalog` default. Inverts evaluator dependency on hardcoded context names. Trait is intentionally narrow: returns the root binding only; traversal is the evaluator's responsibility.
- `evaluate_with_catalog` entry point. Existing `evaluate` forwards via `EmptyCatalog`.
- `EvalError::UnboundContextRef { name }` — new public error variant returned when a `@name` reference resolves to neither a grammar-built-in nor a catalog entry.
- Conformance fixtures under `conformance/host-bindings/` and integration test `tests/host_bindings.rs`.
```

- [ ] **Step 2: Full sweep**

```bash
cd fel-core && cargo nextest run --workspace && cargo clippy -- -D warnings
```

Expected: pass.

- [ ] **Step 3: Commit + push (after parent submodule bump)**

```bash
cd fel-core && git add CHANGELOG.md
git commit -m "docs: §6.3 host-context bindings + ContextBindingCatalog API"
```

---

## Sequencing Recap

```
Task 1: §6.3 spec prose          (canonical)
Task 2: ContextBindingCatalog    (crate API)
Task 3: integration test         (test)
Task 4: JSONL + README index     (conformance index)
Task 5: changelog + sweep        (release hygiene)
```

This plan MUST land before the Response Actions plan's §4.2 (which will declare its catalog citing §6.3). The Mapping spec will retroactively cite §6.3 as the protocol it was always using; that retroactive citation is a one-line note in the Mapping spec, NOT a behavior change.

## Out-of-scope reminders

- **Do not change the grammar productions.** §6.3 uses existing `ContextRef`.
- **Do not implicitly auto-bind.** An evaluator that silently resolves `@undeclared_name` is non-conforming.
- **Do not introduce a SharedCatalog or cross-host registry.** Each evaluator instance owns its catalog.
- **Do not type-check FEL expressions against the catalog's `type` field.** That field is informative; static type-checking remains out of scope.
