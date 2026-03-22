**fel_core > lexer**

# Module: lexer

## Contents

**Structs**

- [`Lexer`](#lexer) - Character-based lexer over a FEL expression string.
- [`Span`](#span) - Byte/char span in the original source (Unicode scalar indices, inclusive start, exclusive end).
- [`SpannedToken`](#spannedtoken) - A [`Token`] with its [`Span`].

**Enums**

- [`Token`](#token) - Lexical token for FEL source (literals, keywords, operators, punctuation).

---

## fel_core::lexer::Lexer

*Struct*

Character-based lexer over a FEL expression string.

**Generic Parameters:**
- 'a

**Methods:**

- `fn new(input: &'a str) -> Self` - Create a lexer for `input` (no allocation beyond char buffer).
- `fn tokenize(self: & mut Self) -> Result<Vec<SpannedToken>, String>` - Consume the entire input and return all tokens, ending with [`Token::Eof`].



## fel_core::lexer::Span

*Struct*

Byte/char span in the original source (Unicode scalar indices, inclusive start, exclusive end).

**Fields:**
- `start: usize` - Start offset.
- `end: usize` - End offset (exclusive).

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Span`



## fel_core::lexer::SpannedToken

*Struct*

A [`Token`] with its [`Span`].

**Fields:**
- `token: Token` - Classified token.
- `span: Span` - Position in source.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SpannedToken`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## fel_core::lexer::Token

*Enum*

Lexical token for FEL source (literals, keywords, operators, punctuation).

**Variants:**
- `Number(rust_decimal::Decimal)`
- `StringLit(String)`
- `True`
- `False`
- `Null`
- `DateLiteral(String)`
- `DateTimeLiteral(String)`
- `Identifier(String)`
- `Let`
- `In`
- `If`
- `Then`
- `Else`
- `And`
- `Or`
- `Not`
- `Plus`
- `Minus`
- `Star`
- `Slash`
- `Percent`
- `Ampersand`
- `Eq`
- `NotEq`
- `Lt`
- `Gt`
- `LtEq`
- `GtEq`
- `DoubleQuestion`
- `Question`
- `LParen`
- `RParen`
- `LBracket`
- `RBracket`
- `LBrace`
- `RBrace`
- `Comma`
- `Dot`
- `Colon`
- `Dollar`
- `At`
- `Eof`

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Token`
- **PartialEq**
  - `fn eq(self: &Self, other: &Token) -> bool`



