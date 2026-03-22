**fel_core > context_json**

# Module: context_json

## Contents

**Functions**

- [`formspec_environment_from_json_map`](#formspec_environment_from_json_map) - Populate a [`FormspecEnvironment`] from a JSON object (e.g. WASM `evalFELWithContext` payload).

---

## fel_core::context_json::formspec_environment_from_json_map

*Function*

Populate a [`FormspecEnvironment`] from a JSON object (e.g. WASM `evalFELWithContext` payload).

Recognized keys: `nowIso` / `now_iso`, `fields`, `variables`, `mipStates` / `mip_states`,
`repeatContext` / `repeat_context`, `instances`.

```rust
fn formspec_environment_from_json_map(ctx: &serde_json::Map<String, serde_json::Value>) -> crate::FormspecEnvironment
```



