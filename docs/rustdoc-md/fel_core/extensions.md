**fel_core > extensions**

# Module: extensions

## Contents

**Structs**

- [`BuiltinFunctionCatalogEntry`](#builtinfunctioncatalogentry) - Metadata for a built-in FEL function exposed to tooling surfaces (WASM catalog, docs).
- [`ExtensionFunc`](#extensionfunc) - A registered extension function.
- [`ExtensionRegistry`](#extensionregistry) - Registry of extension functions.

**Enums**

- [`ExtensionError`](#extensionerror) - Error type for extension registration failures.

**Functions**

- [`builtin_function_catalog`](#builtin_function_catalog) - Slice of all built-in functions (names reserved for [`ExtensionRegistry::register`]).
- [`builtin_function_catalog_json_value`](#builtin_function_catalog_json_value) - Built-in catalog as a JSON array for WASM / tooling.

**Type Aliases**

- [`ExtensionFn`](#extensionfn) - Type alias for extension function implementations.

---

## fel_core::extensions::BuiltinFunctionCatalogEntry

*Struct*

Metadata for a built-in FEL function exposed to tooling surfaces (WASM catalog, docs).

**Fields:**
- `name: &'static str` - Function name as in FEL source.
- `category: &'static str` - Grouping (e.g. `aggregate`, `string`, `repeat`).
- `signature: &'static str` - Human-readable arity and types.
- `description: &'static str` - Short description for UI or generated docs.



## fel_core::extensions::ExtensionError

*Enum*

Error type for extension registration failures.

**Variants:**
- `NameConflict(String)` - Registration rejected: name matches a reserved word or built-in function.

**Traits:** Error

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ExtensionError`
- **Display**
  - `fn fmt(self: &Self, f: & mut std::fmt::Formatter) -> std::fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::extensions::ExtensionFn

*Type Alias*: `Box<dyn Fn>`

Type alias for extension function implementations.



## fel_core::extensions::ExtensionFunc

*Struct*

A registered extension function.

**Fields:**
- `name: String` - Human-readable name for diagnostics.
- `min_args: usize` - Minimum number of arguments.
- `max_args: Option<usize>` - Maximum number of arguments (None = unbounded).
- `func: ExtensionFn` - The implementation: receives pre-evaluated args, returns a value.



## fel_core::extensions::ExtensionRegistry

*Struct*

Registry of extension functions.

**Methods:**

- `fn new() -> Self` - Empty registry (no custom extensions).
- `fn register<impl Into<String>, impl Fn(&[FelValue]) -> FelValue + Send + Sync + 'static>(self: & mut Self, name: impl Trait, min_args: usize, max_args: Option<usize>, func: impl Trait) -> Result<(), ExtensionError>` - Register an extension function.
- `fn get(self: &Self, name: &str) -> Option<&ExtensionFunc>` - Look up an extension function by name.
- `fn contains(self: &Self, name: &str) -> bool` - Check if a name is registered.
- `fn call(self: &Self, name: &str, args: &[FelValue]) -> Option<FelValue>` - Call an extension function with null propagation.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`



## fel_core::extensions::builtin_function_catalog

*Function*

Slice of all built-in functions (names reserved for [`ExtensionRegistry::register`]).

```rust
fn builtin_function_catalog() -> &'static [BuiltinFunctionCatalogEntry]
```



## fel_core::extensions::builtin_function_catalog_json_value

*Function*

Built-in catalog as a JSON array for WASM / tooling.

```rust
fn builtin_function_catalog_json_value() -> serde_json::Value
```



