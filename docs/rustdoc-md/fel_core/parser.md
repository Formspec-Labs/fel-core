**fel_core > parser**

# Module: parser

## Contents

**Structs**

- [`Parser`](#parser) - Recursive-descent parser over a [`SpannedToken`] stream (use [`parse`] to build from source).

**Functions**

- [`parse`](#parse) - Parse a FEL expression string into an AST.

---

## fel_core::parser::Parser

*Struct*

Recursive-descent parser over a [`SpannedToken`] stream (use [`parse`] to build from source).



## fel_core::parser::parse

*Function*

Parse a FEL expression string into an AST.

```rust
fn parse(input: &str) -> Result<Expr, crate::error::FelError>
```



