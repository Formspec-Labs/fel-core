**fel_core > environment**

# Module: environment

## Contents

**Structs**

- [`FormspecEnvironment`](#formspecenvironment) - A full-featured environment for FEL evaluation within a Formspec engine.
- [`MipState`](#mipstate) - XForms Model Item Properties for a single field path.
- [`RepeatContext`](#repeatcontext) - Repeat-group iteration context (§4.3).

---

## fel_core::environment::FormspecEnvironment

*Struct*

A full-featured environment for FEL evaluation within a Formspec engine.

Supports:
- Field resolution via `$field.path` (walks nested data dict)
- Named instances via `@instance('name')`
- Repeat context via `@current`, `@index`, `@count`
- MIP state queries via `valid()`, `relevant()`, etc.
- Definition variables via `@variableName`
- Mapping context via `@source`, `@target`

**Fields:**
- `data: std::collections::HashMap<String, crate::types::FelValue>` - Primary data dict — backs `$field` references.
- `instances: std::collections::HashMap<String, crate::types::FelValue>` - Named secondary instances — backs `@instance('name')`.
- `mip_states: std::collections::HashMap<String, MipState>` - MIP states per dotted field path.
- `variables: std::collections::HashMap<String, crate::types::FelValue>` - Definition variables — backs `@variableName`.
- `repeat_context: Option<RepeatContext>` - Current repeat context (if inside a repeat iteration).
- `current_datetime: Option<crate::types::FelDate>` - Current runtime date for today()/now().

**Methods:**

- `fn new() -> Self` - Empty environment (no data, instances, or repeat context).
- `fn set_field(self: & mut Self, path: &str, value: FelValue)` - Set a field value by dotted path (e.g., "address.city").
- `fn set_instance(self: & mut Self, name: &str, value: FelValue)` - Set a named instance.
- `fn set_mip(self: & mut Self, path: &str, state: MipState)` - Set MIP state for a field path.
- `fn set_variable(self: & mut Self, name: &str, value: FelValue)` - Set a variable value.
- `fn set_now_from_iso(self: & mut Self, iso: &str)` - Set the current runtime datetime from an ISO string.
- `fn push_repeat(self: & mut Self, current: FelValue, index: usize, count: usize, collection: Vec<FelValue>)` - Enter a repeat context.
- `fn pop_repeat(self: & mut Self)` - Leave the current repeat context, restoring the parent.

**Trait Implementations:**

- **Environment**
  - `fn resolve_field(self: &Self, segments: &[String]) -> FelValue`
  - `fn resolve_context(self: &Self, name: &str, arg: Option<&str>, tail: &[String]) -> FelValue`
  - `fn mip_valid(self: &Self, path: &[String]) -> FelValue`
  - `fn mip_relevant(self: &Self, path: &[String]) -> FelValue`
  - `fn mip_readonly(self: &Self, path: &[String]) -> FelValue`
  - `fn mip_required(self: &Self, path: &[String]) -> FelValue`
  - `fn repeat_prev(self: &Self) -> FelValue`
  - `fn repeat_next(self: &Self) -> FelValue`
  - `fn repeat_parent(self: &Self) -> FelValue`
  - `fn current_date(self: &Self) -> Option<FelDate>`
  - `fn current_datetime(self: &Self) -> Option<FelDate>`
- **Default**
  - `fn default() -> Self`



## fel_core::environment::MipState

*Struct*

XForms Model Item Properties for a single field path.

**Fields:**
- `valid: bool` - `valid($path)` result when set for this path.
- `relevant: bool` - `relevant($path)`.
- `readonly: bool` - `readonly($path)`.
- `required: bool` - `required($path)`.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> MipState`



## fel_core::environment::RepeatContext

*Struct*

Repeat-group iteration context (§4.3).

**Fields:**
- `current: crate::types::FelValue` - The current row value.
- `index: usize` - 1-based index within the repeat group.
- `count: usize` - Total instance count.
- `parent: Option<Box<RepeatContext>>` - Parent repeat context (for nested repeats).
- `collection: Vec<crate::types::FelValue>` - All rows in the collection (for prev/next navigation).

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> RepeatContext`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



