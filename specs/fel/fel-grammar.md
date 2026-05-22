# Formspec Expression Language (FEL) — Normative Grammar

**Version:** 1.0
**Status:** Normative companion to Formspec v1.0 §3

---

## 1. Introduction

This document defines the **normative** Parsing Expression Grammar (PEG) for
the Formspec Expression Language (FEL). It is a companion to the Formspec
specification v1.0 §3 and supersedes the **informative** grammar given in §3.7
of that specification.

A conformant FEL parser MUST accept exactly the language described by this
grammar. The semantics of each construct are defined in §§3.2–3.12 of the
Formspec specification; this document defines only the syntax.

## 2. Notation

This grammar uses the Parsing Expression Grammar (PEG) formalism as defined by
Bryan Ford in *"Parsing Expression Grammars: A Recognition-Based Syntactic
Foundation"* (POPL 2004). PEGs are unambiguous by construction: for every
input there is at most one valid parse tree.

The following notation conventions are used throughout:

| Notation | Meaning |
|----------|------------------------------------------|
| `'text'` | Literal string match |
| `[a-z]` | Character class (any character in range) |
| `e1 e2` | Sequence: match `e1` then `e2` |
| `e1 / e2`| Ordered choice: try `e1`; if it fails, try `e2` |
| `e*` | Zero or more repetitions of `e` |
| `e+` | One or more repetitions of `e` |
| `e?` | Optional: zero or one occurrence of `e` |
| `!e` | Negative lookahead: succeeds iff `e` fails; consumes no input |
| `&e` | Positive lookahead: succeeds iff `e` succeeds; consumes no input |
| `( )` | Grouping |
| `{n}` | Exactly `n` repetitions |
| `.` | Any single character |

Non-terminals are written in `PascalCase`. Terminals (literal strings and
character classes) appear inline. Rule definitions use the `←` arrow.

## 3. Lexical Grammar

The lexical grammar defines the low-level tokens of FEL. These rules are
referenced by the expression grammar in §4.

### 3.1 Whitespace and Comments

Whitespace and comments are **insignificant** except inside string literals.
The `_` production matches optional whitespace and/or comments and may appear
between any two tokens.

```peg
_              ← (Whitespace / Comment)*
Whitespace     ← [ \t\n\r]
Comment        ← LineComment / BlockComment
LineComment    ← '//' (!LineTerminator .)* LineTerminator?
BlockComment   ← '/*' (!'*/' .)* '*/'
LineTerminator ← '\n' / '\r\n' / '\r'
```

Block comments do **not** nest. The input `/* a /* b */ c */` is a block
comment `/* a /* b */` followed by the tokens `c`, `*`, `/`.

### 3.2 Identifiers

Identifiers are ASCII-only in FEL v1.0.

```peg
Identifier     ← !ReservedWord [a-zA-Z_] [a-zA-Z0-9_]*
```

An identifier MUST NOT be a reserved word (§3.5) when used as a function name.
Field keys referenced via `$` are not subject to this restriction because the
`$` sigil disambiguates them.

### 3.3 Reserved Words

The following words are reserved. They MUST NOT be used as function names
(built-in or extension).

```peg
ReservedWord   ← ('true' / 'false' / 'null'
               /  'and' / 'or' / 'not' / 'in'
               /  'if' / 'then' / 'else' / 'let')
                  ![a-zA-Z0-9_]
```

The trailing negative lookahead ensures that identifiers such as `notify` or
`informal` are not incorrectly matched as containing a reserved word.

### 3.4 String Literals

FEL supports single-quoted and double-quoted strings with escape sequences.

```peg
StringSQ       ← '\'' (EscapeSeq / !('\'' / '\\') .)* '\''
StringDQ       ← '"' (EscapeSeq / !('"' / '\\') .)* '"'
StringLiteral  ← StringDQ / StringSQ

EscapeSeq      ← '\\' [\\'"nrt]
               / '\\u' HexDigit{4}
HexDigit       ← [0-9a-fA-F]
```

Defined escape sequences:

| Escape | Character |
|--------|-----------|
| `\\` | Backslash (`U+005C`) |
| `\'` | Single quote (`U+0027`) |
| `\"` | Double quote (`U+0022`) |
| `\n` | Line feed (`U+000A`) |
| `\r` | Carriage return (`U+000D`) |
| `\t` | Horizontal tab (`U+0009`) |
| `\uXXXX` | Unicode code point (4 hex digits) |

An unrecognised escape sequence (e.g., `\a`) is a syntax error.

### 3.5 Number Literals

```peg
NumberLiteral  ← '-'? IntegerPart ('.' [0-9]+)? Exponent?
IntegerPart    ← '0' / [1-9] [0-9]*
Exponent       ← ('e' / 'E') ('+' / '-')? [0-9]+
```

- A leading dot is **not** permitted: `.5` is invalid; write `0.5`.
- A trailing dot is **not** permitted: `5.` is invalid; write `5` or `5.0`.
- A bare minus sign is the unary negation operator, not part of a number
  literal. However, the grammar allows a leading `-` on `NumberLiteral` so
  that negative constants are parsed as a single token in literal position.

### 3.6 Date and DateTime Literals

```peg
DateTimeLiteral ← '@' Digit{4} '-' Digit{2} '-' Digit{2} 'T'
                   Digit{2} ':' Digit{2} ':' Digit{2}
                   TimeZone?
DateLiteral     ← '@' Digit{4} '-' Digit{2} '-' Digit{2}

TimeZone        ← 'Z' / [+-] Digit{2} ':' Digit{2}
Digit           ← [0-9]
```

Note: `DateTimeLiteral` MUST be tried before `DateLiteral` (ordered choice)
to avoid partial matching.

### 3.7 Boolean and Null Literals

```peg
BooleanLiteral ← 'true' / 'false'
NullLiteral    ← 'null'
```

These are listed under `ReservedWord` (§3.3) and the trailing `![a-zA-Z0-9_]`
lookahead applies to prevent `trueValue` from being parsed as `true` + `Value`.

### 3.8 Integer (Array Index)

```peg
Integer        ← [0-9]+
```

Used only in array-index position within path expressions.

## 4. Expression Grammar

The expression grammar defines the full syntax of FEL expressions. Operator
precedence is encoded structurally: lower-precedence operators appear higher
in the grammar (closer to the start symbol).

```peg
# ============================================================
# Formspec Expression Language (FEL) — Normative PEG Grammar
# Version 1.0
# ============================================================

Expression     ← _ LetExpr _

# --- Let binding ---
# The let-value position uses LetValue (not LetExpr/IfExpr) to avoid
# ambiguity with the 'in' keyword. The 'in' membership operator is not
# available as a bare operator in let-value position; parenthesise it:
#   let x = (1 in $arr) in ...  
LetExpr        ← 'let' _ Identifier _ '=' _ LetValue _ 'in' _ LetExpr
               / IfExpr
LetValue       ← IfExpr   # but with Membership production omitted

# --- If-then-else (keyword form) ---
IfExpr         ← 'if' _ Ternary _ 'then' _ IfExpr _ 'else' _ IfExpr
               / Ternary

# --- Ternary conditional (precedence 1, right-associative) ---
Ternary        ← LogicalOr (_ '?' _ Expression _ ':' _ Expression)?

# --- Logical OR (precedence 2, left-associative) ---
LogicalOr      ← LogicalAnd (_ 'or' !IdContinue _ LogicalAnd)*

# --- Logical AND (precedence 3, left-associative) ---
LogicalAnd     ← Equality (_ 'and' !IdContinue _ Equality)*

# --- Equality (precedence 4, left-associative) ---
Equality       ← Comparison ((_ '!=' / _ '=') _ Comparison)*

# --- Comparison (precedence 5, left-associative) ---
Comparison     ← Membership ((_ '<=' / _ '>=' / _ '<' / _ '>') _ Membership)*

# --- Membership (precedence 6, non-associative) ---
Membership     ← NullCoalesce (_ 'not' _ 'in' _ NullCoalesce
                              / _ 'in' !IdContinue _ NullCoalesce)?

# --- Null-coalescing (precedence 7, left-associative) ---
NullCoalesce   ← Addition (_ '??' _ Addition)*

# --- Addition / concatenation (precedence 8, left-associative) ---
Addition       ← Multiplication ((_ '+' / _ '-' / _ '&') _ Multiplication)*

# --- Multiplication (precedence 9, left-associative) ---
Multiplication ← Unary ((_ '*' / _ '/' / _ '%') _ Unary)*

# --- Unary prefix (precedence 10, right-associative) ---
Unary          ← 'not' !IdContinue _ Unary
               / '!' _ Unary
               / '-' _ Unary
               / Postfix

# --- Postfix (dot/index access after any atom) ---
Postfix        ← Atom PathTail*

# --- Atoms ---
Atom           ← IfCall
               / FunctionCall
               / FieldRef
               / ObjectLiteral
               / ArrayLiteral
               / Literal
               / '(' _ Expression _ ')'

# --- Helper: identifier-continue character ---
IdContinue     ← [a-zA-Z0-9_]
```

### 4.1 Function Calls

```peg
IfCall         ← 'if' _ '(' _ ArgList? _ ')'
FunctionCall   ← Identifier '(' _ ArgList? _ ')'
ArgList        ← Expression (_ ',' _ Expression)*
```

The `IfCall` production handles `if(cond, a, b)` as a built-in function call.
Because `if` is a reserved word (§3.3), it cannot match the `Identifier`
production in `FunctionCall`. The parser MUST try `IfCall` before
`FunctionCall`. The opening parenthesis disambiguates `if(...)` (function
call) from `if ... then ... else ...` (keyword conditional).

All other function names are `Identifier`s and MUST NOT be reserved words.

### 4.2 Object Literals

```peg
ObjectLiteral  ← '{' _ ObjectEntries? _ '}'
ObjectEntries  ← ObjectEntry (_ ',' _ ObjectEntry)*
ObjectEntry    ← ObjectKey _ ':' _ Expression
ObjectKey      ← Identifier / StringLiteral
```

Object literal keys are either bare identifiers or string literals. Duplicate
keys within a single object literal are a syntax error.

### 4.3 Array Literals

```peg
ArrayLiteral   ← '[' _ (Expression (_ ',' _ Expression)*)? _ ']'
```

All elements of an array literal MUST be of the same type (enforced during
type checking, not at the grammar level).

### 4.4 Literals

```peg
Literal        ← DateTimeLiteral
               / DateLiteral
               / NumberLiteral
               / StringLiteral
               / BooleanLiteral !IdContinue
               / NullLiteral !IdContinue
```

`DateTimeLiteral` MUST be tried before `DateLiteral` (ordered choice) to
prevent the date prefix from matching prematurely.

## 5. Operator Precedence Table

The following table lists all FEL operators from **lowest** to **highest**
precedence. This table is normative and matches the structural encoding in §4.

| Prec. | Operator(s) | Category | Assoc. | Example |
|:-----:|-------------|----------|:------:|------------------------------------------|
| 0 | `let … = … in …` | Binding | Right | `let x = 1 in x + 2` |
| 0 | `if … then … else …` | Conditional | Right | `if $a then 'yes' else 'no'` |
| 1 | `? :` | Ternary | Right | `$a > 0 ? 'pos' : 'neg'` |
| 2 | `or` | Logical OR | Left | `$a or $b` |
| 3 | `and` | Logical AND | Left | `$a and $b` |
| 4 | `=` `!=` | Equality | Left | `$x = 5` |
| 5 | `<` `>` `<=` `>=` | Comparison | Left | `$age >= 18` |
| 6 | `in` `not in` | Membership | Non | `$s in ['a','b']` |
| 7 | `??` | Null-coalescing| Left | `$x ?? 0` |
| 8 | `+` `-` `&` | Add / Concat | Left | `$a + $b`, `$s & '!'` |
| 9 | `*` `/` `%` | Multiply | Left | `$a * $b` |
| 10 | `not` / `!` (prefix), `-` (negate) | Unary | Right | `not $flag`, `!$flag`, `-$x` |

Parenthesised sub-expressions (`( … )`) override precedence as usual.
Postfix operators (`.field`, `[index]`) bind tighter than all prefix
operators, enabling `prev().field` and `(expr).field`.

## 6. Path Expressions

Path expressions reference Instance data. They are **not** general
expressions — they appear as atoms in the expression grammar.

```peg
FieldRef       ← '$' Identifier PathTail*
               / '$'
               / ContextRef PathTail*

PathTail       ← '.' Identifier
               / '[' _ ( Integer / '*' ) _ ']'

ContextRef     ← '@' Identifier ('(' _ StringLiteral? _ ')')?
```

### 6.1 Reference Forms

| Syntax | Description | Example |
|--------|-------------|---------|
| `$` | Current context node (self-reference). | `$ > 0` |
| `$ident` | Field reference resolved from nearest scope. | `$firstName` |
| `$a.b.c` | Nested field path through groups. | `$address.city` |
| `$a[n]` | 1-based index into a repeat collection. | `$items[1].name` |
| `$a[*]` | Wildcard — array of all values across repeat instances. | `sum($items[*].amt)` |
| `$a[n].b` | Field within indexed repeat instance. | `$items[2].qty` |
| `$a[*].b` | Field across all repeat instances (produces array). | `$items[*].qty` |
| `@current` | Explicit reference to the current repeat instance. | `@current.amount` |
| `@index` | 1-based position of current repeat instance. | `@index = 1` |
| `@count` | Total instances in current repeat collection. | `@count >= 1` |
| `@name` | Context identifier; host-supplied catalogs are governed by §6.3. | `@response.applicantName` |
| `@instance('n')` | Secondary data-source instance. | `@instance('prior').income` |
| `@source` | Source binding in mapping DSL. | `@source.fieldA` |
| `@target` | Target binding in mapping DSL. | `@target.fieldB` |

### 6.2 Path Resolution Rules

1. **Scope:** Field references are lexically scoped. Inside a repeatable
   group, `$sibling` resolves within the **same repeat instance**.
2. **Index bounds:** An explicit index `$repeat[n]` where `n < 1` or
   `n >` the number of instances MUST signal an evaluation error.
3. **Instance lookup:** `@instance('name')` MUST match a declared Data Source.
   If the named instance does not exist, the processor MUST signal a
   definition error.
4. **Chaining:** Dot-segments after an indexed or wildcard subscript continue
   path resolution into the selected object(s). Multiple subscripts may be
   chained: `$a[1].nested[*].value` is valid.
5. **Context postfix access:** `PathTail` applies after `ContextRef`, so
   `@response.items[1].amount` and `@effects[*].outcomeRef` use the same
   postfix traversal rules as `$` field references.

### 6.3 Host-Supplied Context Bindings

FEL is host-agnostic: the same grammar embeds in Formspec Definitions, Mapping
documents, Response Actions documents, Experience documents, and other future
host specifications. Each host MAY require additional context that the FEL
grammar reserves a slot for but does NOT itself enumerate. Such context flows
through the `ContextRef` production (§6) using `@name` syntax. This section
specifies the protocol a host spec MUST follow when declaring such bindings,
and what a conformant catalog-aware FEL evaluator MUST do at evaluation time.

#### 6.3.1 Declaration shape

A host specification declaring host-supplied context bindings MUST publish a
binding catalog: a closed list of identifiers, each entry carrying:

| Field | Required | Description |
|---|---|---|
| `name` | Yes | The identifier following `@`. MUST satisfy the `Identifier` lexical rule (§3.2). MUST NOT collide with grammar-reserved context names (`current`, `index`, `count`, `instance`). |
| `kind` | Yes | One of `value`, `object`, `function`. Determines path access semantics (§6.3.2). |
| `type` | Yes | Informative human-readable type contract (for example, "object with fields `id`, `attempt`", "datetime", or "function() -> datetime"). FEL evaluators do not normatively type-check this field. |
| `purity` | Yes | One of `pure` (deterministic over the bound value) or `impure` (evaluation MAY produce a different value across invocations, for example a clock read). |
| `evaluationTiming` | Yes | One of `eager` (resolved once before expression evaluation begins) or `lazy` (resolved on demand at the access site). |
| `scope` | Yes | One of `expression` (bound for the entire expression) or `subexpression` (bound only within a specific scope; reserved for future use). |

Host specs MUST publish the catalog in normative prose adjacent to wherever the
FEL expression is declared. The catalog MUST be closed: a catalog-aware FEL
evaluator MUST reject any `@name` whose `name` is not registered and is not a
grammar-reserved context name.

#### 6.3.2 Evaluator obligations

A conformant FEL evaluator that supports host-supplied bindings:

1. **MUST** accept a host-supplied binding catalog at the start of evaluation.
   The crate-level API for accepting the catalog is implementation-defined; the
   conformance requirement is observable behavior, not API shape.
2. **MUST** reject a `@name` reference whose `name` is not in the active catalog
   AND is not a grammar-reserved context name (§6.1). Rejection is an evaluation
   error, not a parse error.
3. **MUST** resolve `@name` references according to the catalog entry's `kind`:
   - `value`: `@name` returns the bound scalar. `@name.suffix` is an evaluation
     error.
   - `object`: `@name` returns the root bound object. The evaluator, not the
     catalog, traverses dot-segments per §6.2 path-resolution rules.
   - `function`: `@name(...)` invokes the bound function. Bare `@name` is an
     evaluation error. The existing grammar permits a context call with no
     arguments or a single string literal argument.
4. **MUST** pass function-call string arguments to the host binding resolver
   when the implementation exposes such an API. A function binding that does
   not accept the supplied argument MUST reject the access as an evaluation
   error.
5. **MUST** isolate catalogs across evaluator instances. A catalog supplied to
   one evaluation MUST NOT leak into another evaluation that did not receive it.

Evaluations that do not opt into a host-supplied catalog MAY retain
implementation-defined environment context behavior for backwards
compatibility. Once a host catalog is active, unregistered non-reserved names
MUST be rejected.

The host adapter or catalog implementation, not the grammar, is responsible for
honoring `evaluationTiming`: `eager` bindings are materialized once for the
evaluation, while `lazy` bindings are resolved on demand. A reference evaluator
MAY receive already-materialized roots as long as observable behavior satisfies
the host spec's declared timing.

#### 6.3.3 Examples (non-normative)

A Response Actions document can declare:

| name | kind | type | purity | evaluationTiming | scope |
|---|---|---|---|---|---|
| `response` | object | Current Response snapshot | pure | eager | expression |
| `definition` | object | Pinned Definition | pure | eager | expression |
| `action` | object | `{ id, intent, actor }` | pure | eager | expression |
| `now` | function | `() -> datetime` | impure | lazy | expression |
| `validation` | object | `{ lastReport: ValidationReport \| null }` | pure | eager | expression |
| `invocation` | object | `{ id: string, attempt: integer }` | pure | eager | expression |

Example expressions: `@response.applicantName != null`, `@now() >
@response.openedAt`, and `@invocation.attempt = 1`.

Grammar-built-ins are a separate category. The grammar-reserved context names
(`current`, `index`, `count`, `instance` per §6.1) are NOT host-supplied
bindings. They are normative parts of the FEL grammar. Host specs MUST NOT
re-declare grammar-built-ins in their §6.3 catalog.

The Mapping spec owns `@source` / `@target` as Mapping-context bindings. Those
names are not globally reserved outside Mapping; a catalog-aware non-Mapping
host that does not register them MUST reject them as unbound.

#### 6.3.4 Relationship to §6.2

Host-supplied object bindings interact with §6.2 path-resolution rules as
follows:

- §6.2.1 (lexical scope) does NOT apply to host bindings. `@name` is global to
  the expression; it is not affected by repeatable-group scope.
- §6.2.2 (index bounds) applies to host bindings whose object structure
  contains arrays; `@effects[1].outcomeRef` uses one-based FEL indexing.
- §6.2.3 (instance lookup) is for `@instance('name')`, which is
  grammar-reserved and not subject to §6.3 registration.
- §6.2.4 (chaining) applies: `@response.items[1].amount` is legal when
  `response` is registered as an `object`-kind binding and the path traverses
  validly.

#### 6.3.5 Conformance hook

A host spec adopting §6.3 MUST publish:

1. The closed catalog (§6.3.1 fields populated).
2. The relationship between the host's evaluation moments and each binding's
   `evaluationTiming`.
3. Negative conformance fixtures asserting unbound `@name` references are
   rejected.

These three items are the §6.3 acceptance bar for a host adopter.

## 7. Conformance

Locale formatting builtins (`formatNumber`, `formatDate`) and other cataloged
functions are not part of this grammar; their syntax is `Identifier '(' … ')'`
as in §4. Semantics and conformance fixtures are defined in
[`docs/SPEC.md`](../../docs/SPEC.md) and `conformance/fel-conformance.jsonl`.

A conformant FEL parser:

1. **MUST** accept all input strings that match the `Expression` production of
   this grammar.
2. **MUST** reject all input strings that do not match the `Expression`
   production. Rejection MUST include a diagnostic indicating the approximate
   position of the syntax error.
3. **MUST** treat whitespace and comments as insignificant except inside string
   literals.
4. **MUST** implement all escape sequences defined in §3.4. An unrecognised
   escape sequence MUST be rejected as a syntax error.
5. **MUST** enforce the reserved-word restriction in §3.3: reserved words MUST
   NOT be accepted as function names.
6. **MUST** parse the `|>` (pipe) character sequence as a syntax error in
   v1.0. This token is reserved for future use.
7. **SHOULD** produce a parse tree (or equivalent AST) that preserves the
   precedence and associativity encoded in this grammar. The parse tree
   structure determines evaluation order as specified in §3.3 and §3.8–3.10
   of the Formspec specification.

---

*End of normative grammar.*
