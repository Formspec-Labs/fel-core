# Security Policy

`fel-core` is pre-1.0 as a Rust API, but the language semantics and evaluator
behavior are part of the Formspec runtime trust boundary.

## Reporting

Do not open a public issue for a suspected vulnerability.

Use GitHub private vulnerability reporting when it is available. If private
reporting is unavailable, email `security@formspec.org` with the affected
version or commit, impact, reproduction steps, and proof of concept.

## Scope

In scope:

- Parser, lexer, evaluator, dependency analysis, and host-environment behavior.
- Resource-budget enforcement and denial-of-service risks for untrusted FEL.
- JSON conversion, diagnostic wire shapes, and conformance fixtures.
- Build, release, and fixture-generation tooling that could change shipped
  behavior or generated artifacts.

Out of scope:

- Pure documentation typo reports with no security impact.
- Vulnerabilities in third-party dependencies with no demonstrated `fel-core`
  impact. Report those upstream first.
- Local developer resource consumption that does not affect a deployed parser,
  evaluator, host binding, or generated artifact.

## Response

The project aims to acknowledge valid reports within 7 calendar days. Fix
timing depends on severity and release exposure.

Security fixes must preserve conformance truth. Do not hide a failure by
weakening a fixture, deleting a regression, or changing a diagnostic shape
without a matching spec update and test.
