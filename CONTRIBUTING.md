# Contributing to fel-core

Thanks for improving `fel-core`.

## Development Workflow

Use red-green-refactor for behavior changes:

1. Add or update a test that demonstrates the bug, gap, or new requirement.
2. Make the smallest implementation change that satisfies the test.
3. Refactor without changing behavior.
4. Run the focused test first, then the full gate before review.

## Required Checks

Run the local gate before opening a pull request:

```sh
make ci
```

For changes that affect cross-runtime semantics, also run:

```sh
make ratify-external
```

`make ratify-external` requires the sibling Formspec Python and WASM runtimes to
be installed or built in the checkout.

## Commit Convention

Use Conventional Commit style with a clear scope when useful:

- `feat(fel-core): ...`
- `fix(fel-core): ...`
- `docs(fel-core): ...`
- `test(fel-core): ...`
- `build(fel-core): ...`
- `chore(fel-core): ...`

## License Terms

By submitting a pull request, you agree to license your contribution under
Apache-2.0. See [LICENSING.md](LICENSING.md).
