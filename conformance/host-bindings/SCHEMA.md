# Host-binding fixture format

This directory holds JSON fixtures consumed by
`tests/host_bindings.rs::host_binding_fixture_files_match_harness`. Each
fixture pins a single host-binding-catalog scenario per
`fel-grammar.md §6.3` and serves as a normative example for adopters
implementing the host-binding port in a non-Rust runtime.

## File schema

Each `.json` file in this directory is a single fixture object:

```json
{
  "name": "<stable-identifier>",
  "description": "<one-sentence prose>",
  "expression": "<FEL source>",
  "catalog": {
    "<binding-name>": {
      "kind": "value | object | function",
      "value": "<the resolved value, as a FEL-JSON value>"
    }
  },
  "expected": {
    "value": "<expected eval value, as FEL-JSON>",
    "diagnostics": [
      { "code": "<FEL-DIAG-CODE>", "message": "<substring match>" }
    ]
  }
}
```

### Field semantics

| Field | Type | Required | Meaning |
|---|---|---|---|
| `name` | string | yes | Stable identifier — match the filename (sans `.json`). |
| `description` | string | yes | Human-readable; appears in test failure messages. |
| `expression` | string | yes | FEL source to parse + evaluate. |
| `catalog` | object | yes | Map of binding name → `{kind, value}`. Empty `{}` for fixtures testing unbound refs. |
| `catalog.<name>.kind` | enum | yes | One of `"value"`, `"object"`, `"function"`. Per `ContextBindingKind`. |
| `catalog.<name>.value` | FEL-JSON | yes | The value the catalog resolves to. Decoded via `fel_core::json_to_fel`. |
| `expected.value` | FEL-JSON | yes | Required. Encoded via `fel_core::fel_to_ui_json` for comparison. |
| `expected.diagnostics` | array | yes | May be empty `[]`. Each entry asserts a substring match on the produced diagnostic. |
| `expected.diagnostics[].code` | string | no | Optional. Asserts `diagnostic.code == this`. |
| `expected.diagnostics[].message` | string | no | Optional. Asserts substring match within `diagnostic.message`. |

### FEL-JSON value encoding

Values in `catalog.<name>.value` and `expected.value` use the canonical
FEL-JSON encoding (the same shape `fel_core::fel_to_json` /
`fel_core::json_to_fel` round-trip):

- `null` → `null`
- `true` / `false` → `true` / `false`
- Numbers → `123` or `"123.456"` (string for non-JS-safe-integer values per `convert.rs`)
- Strings → `"..."`
- Dates → `{ "$type": "date", "value": "2024-01-15" }`
- Money → `{ "$type": "money", "amount": "100.50", "currency": "USD" }`
- Arrays → `[ ... ]`
- Objects → `{ ... }` (no `$type` key)

### Binding-kind dispatch contract

Per `fel-grammar.md §6.3.2`:

- `kind: "value"` — bare access only. Postfix traversal (`@name.x`) is rejected with `FEL-CONTEXT-BINDING-PATH`. Call syntax (`@name()`) is rejected with `FEL-CONTEXT-BINDING-NOT-CALLABLE`.
- `kind: "object"` — supports postfix traversal. Call syntax rejected with `FEL-CONTEXT-BINDING-NOT-CALLABLE`.
- `kind: "function"` — requires call syntax. Bare access rejected with `FEL-CONTEXT-BINDING-CALL-REQUIRED`.

Fixtures testing rejection paths set `expected.value` to `null` and
include the corresponding diagnostic code in `expected.diagnostics[]`.

## Adding a fixture

1. Pick a stable `name` matching the filename.
2. Write a one-sentence `description` (will appear in test failure messages).
3. Construct the `catalog` per §6.3.1.
4. Encode `expected.value` and `expected.diagnostics` per the canonical FEL-JSON shape above.
5. Run `cargo test --test host_bindings host_binding_fixture` to verify.

## Reserved context names (§6.3.4)

`@current`, `@index`, `@count` are reserved when the host catalog is
active. Fixtures testing these should declare them in `catalog` with
the appropriate kind, OR test the unbound-reference rejection path
(see `unbound-context-ref.json` for the pattern).
