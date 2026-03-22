**fel_core > dependencies**

# Module: dependencies

## Contents

**Structs**

- [`Dependencies`](#dependencies) - Dependencies extracted from a FEL expression.

**Functions**

- [`dependencies_to_json_value`](#dependencies_to_json_value) - Serialize [`Dependencies`] for WASM / JSON FFI (camelCase keys).
- [`dependencies_to_json_value_styled`](#dependencies_to_json_value_styled) - Serialize [`Dependencies`] with explicit host key style.
- [`extract_dependencies`](#extract_dependencies) - Extract dependencies from an AST expression.

---

## fel_core::dependencies::Dependencies

*Struct*

Dependencies extracted from a FEL expression.

**Fields:**
- `fields: std::collections::HashSet<String>` - Field paths referenced (e.g., `["firstName", "address.city"]`).
- `context_refs: std::collections::HashSet<String>` - Context references (e.g., `["@current", "@index"]`).
- `instance_refs: std::collections::HashSet<String>` - Instance references from `@instance('name')`.
- `mip_deps: std::collections::HashSet<String>` - MIP dependencies: fields used in valid/relevant/readonly/required.
- `has_self_ref: bool` - Whether bare `$` (self-reference) appears.
- `has_wildcard: bool` - Whether any `[*]` wildcard appears.
- `uses_prev_next: bool` - Whether prev() or next() is called.

**Trait Implementations:**

- **Default**
  - `fn default() -> Dependencies`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Dependencies`



## fel_core::dependencies::dependencies_to_json_value

*Function*

Serialize [`Dependencies`] for WASM / JSON FFI (camelCase keys).

```rust
fn dependencies_to_json_value(deps: &Dependencies) -> serde_json::Value
```



## fel_core::dependencies::dependencies_to_json_value_styled

*Function*

Serialize [`Dependencies`] with explicit host key style.

```rust
fn dependencies_to_json_value_styled(deps: &Dependencies, style: crate::wire_style::JsonWireStyle) -> serde_json::Value
```



## fel_core::dependencies::extract_dependencies

*Function*

Extract dependencies from an AST expression.

```rust
fn extract_dependencies(expr: &Expr) -> Dependencies
```



