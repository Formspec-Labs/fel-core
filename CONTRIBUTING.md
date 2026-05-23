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

## Test Maintenance

### Updating `insta` snapshot tests

`tests/snapshot_tests.rs` uses inline `insta::assert_snapshot!(@"…")` to pin
exact error-message wording. When an intentional message change causes a
snapshot test to fail:

```sh
cargo install cargo-insta            # one-time
cargo insta test --review            # accept/reject each diff interactively
```

For inline snapshots, the macro rewrites the test source in place. Review
the diff carefully — message changes are intentional contract changes;
spurious whitespace changes signal implementation drift.

### Refreshing the fuzz regression corpus

`tests/corpus/fuzz_regression.jsonl` is corpus-driven; when the canonical
`Value::Display` representation changes intentionally, regenerate the
`displayOracle` column:

```sh
make fuzz-regression-refresh
```

See `tests/corpus/README.md` for the row format.

### Mutation gate

Mutation testing runs weekly in CI (`.github/workflows/mutants.yml`).
Locally:

```sh
make mutants-install                 # one-time, pinned cargo-mutants version
make mutants-<file>                  # per-file (parser, lexer, evaluator, ...)
make mutants-p0                      # full P0 sweep (~1 hour on a 14-core host)
```

After a re-baseline, append rows with:

```sh
python3 scripts/mutation_baseline.py --append
```

The script is idempotent on (file, sha) — re-running on the same outcomes
won't duplicate rows. See `thoughts/2026-05-23-mutation-survivor-followups.md`
for the survivor classification policy.

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
