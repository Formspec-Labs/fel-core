# fel_core

FEL parser, evaluator, and dependency analysis with base-10 decimal arithmetic.

Uses `rust_decimal` for base-10 arithmetic per spec S3.4.1 (minimum 18 significant digits).

## Docs

- Human overview: crate `README.md` (architecture, pipeline, module map).
- API reference: `cargo doc -p fel-core --no-deps --open`.
- Markdown API export: `docs/rustdoc-md/` (regenerate with `cargo doc-md`; see crate README).

## Modules

### [`fel_core`](fel_core.md)

*1 struct, 13 modules, 4 functions*

### [`ast`](ast.md)

*4 enums*

### [`context_json`](context_json.md)

*1 function*

### [`convert`](convert.md)

*4 functions*

### [`dependencies`](dependencies.md)

*1 struct, 3 functions*

### [`environment`](environment.md)

*3 structs*

### [`error`](error.md)

*1 struct, 2 enums, 2 functions*

### [`evaluator`](evaluator.md)

*1 function, 1 trait, 3 structs*

### [`extensions`](extensions.md)

*1 enum, 1 type alias, 2 functions, 3 structs*

### [`lexer`](lexer.md)

*1 enum, 3 structs*

### [`parser`](parser.md)

*1 function, 1 struct*

### [`printer`](printer.md)

*1 function*

### [`types`](types.md)

*1 struct, 2 enums, 6 functions*

### [`wire_style`](wire_style.md)

*1 enum*

