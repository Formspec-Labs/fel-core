**fel_core > wire_style**

# Module: wire_style

## Contents

**Enums**

- [`JsonWireStyle`](#jsonwirestyle) - JSON object key style for WASM (`camelCase`) vs Python (`snake_case`) bindings.

---

## fel_core::wire_style::JsonWireStyle

*Enum*

JSON object key style for WASM (`camelCase`) vs Python (`snake_case`) bindings.

**Variants:**
- `JsCamel` - JavaScript / `wasm-bindgen` (camelCase keys).
- `PythonSnake` - Python `formspec_rust` surface (snake_case keys).

**Traits:** Eq, Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &JsonWireStyle) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> JsonWireStyle`



