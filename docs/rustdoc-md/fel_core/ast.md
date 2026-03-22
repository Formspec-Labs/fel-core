**fel_core > ast**

# Module: ast

## Contents

**Enums**

- [`BinaryOp`](#binaryop) - Binary and logical operators (precedence enforced in the parser).
- [`Expr`](#expr) - Expression AST for Formspec Expression Language (FEL).
- [`PathSegment`](#pathsegment) - A path segment for field references and postfix access (`$a.b`, `$a[1]`, `$a[*]`).
- [`UnaryOp`](#unaryop) - Unary operators (`not`, unary `-`).

---

## fel_core::ast::BinaryOp

*Enum*

Binary and logical operators (precedence enforced in the parser).

**Variants:**
- `Add` - `+`
- `Sub` - `-`
- `Mul` - `*`
- `Div` - `/`
- `Mod` - `%`
- `Concat` - `&` string concatenation.
- `Eq` - `=` or `==`
- `NotEq` - `!=`
- `Lt` - `<`
- `Gt` - `>`
- `LtEq` - `<=`
- `GtEq` - `>=`
- `And` - `and`
- `Or` - `or`

**Traits:** Eq, Copy

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &BinaryOp) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> BinaryOp`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::ast::Expr

*Enum*

Expression AST for Formspec Expression Language (FEL).

Covers literals, operators, `let`/`if`, function calls, `$` field refs, and `@` context refs.
Shape follows `specs/fel/fel-grammar.llm.md` in the Formspec repo.

**Variants:**
- `Null`
- `Boolean(bool)`
- `Number(rust_decimal::Decimal)`
- `String(String)`
- `DateLiteral(String)`
- `DateTimeLiteral(String)`
- `Array(Vec<Expr>)`
- `Object(Vec<(String, Expr)>)`
- `FieldRef{ name: Option<String>, path: Vec<PathSegment> }`
- `ContextRef{ name: String, arg: Option<String>, tail: Vec<String> }`
- `UnaryOp{ op: UnaryOp, operand: Box<Expr> }`
- `BinaryOp{ op: BinaryOp, left: Box<Expr>, right: Box<Expr> }`
- `Ternary{ condition: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr> }`
- `IfThenElse{ condition: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr> }`
- `Membership{ value: Box<Expr>, container: Box<Expr>, negated: bool }`
- `NullCoalesce{ left: Box<Expr>, right: Box<Expr> }`
- `LetBinding{ name: String, value: Box<Expr>, body: Box<Expr> }`
- `FunctionCall{ name: String, args: Vec<Expr> }`
- `PostfixAccess{ expr: Box<Expr>, path: Vec<PathSegment> }`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Expr`
- **PartialEq**
  - `fn eq(self: &Self, other: &Expr) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::ast::PathSegment

*Enum*

A path segment for field references and postfix access (`$a.b`, `$a[1]`, `$a[*]`).

**Variants:**
- `Dot(String)` - Property after a dot (identifier name).
- `Index(usize)` - Numeric index inside `[` `]`.
- `Wildcard` - Repeat wildcard `[*]`.

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &PathSegment) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> PathSegment`



## fel_core::ast::UnaryOp

*Enum*

Unary operators (`not`, unary `-`).

**Variants:**
- `Not` - Logical not.
- `Neg` - Arithmetic negation.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &UnaryOp) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> UnaryOp`



