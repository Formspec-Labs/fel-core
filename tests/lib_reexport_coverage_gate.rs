//! Phase 3 CI gate — `src/lib.rs` re-export coverage.
//!
//! Asserts that every `pub use` symbol in `src/lib.rs` has an entry in
//! `tests/lib_reexport_coverage.toml`, and that each entry either cites a
//! resolving proptest function or declares an E1..E8 exemption with the
//! citations the design requires.
//!
//! ## Status
//!
//! Activated in Phase 3c — runs by default under `cargo test`. The
//! manifest carries one row per `pub use` symbol in `src/lib.rs`; every
//! row is either GREEN (cites a resolving proptest) or EXEMPT (E1..E8
//! with the citations the design requires). See
//! `thoughts/2026-05-23-phase-3-ci-gate-design.md` §Implementation plan
//! for the rollout history.
//!
//! ## Cite-resolution hardening (Phase 3e)
//!
//! `resolve_example_cite` rejects two fake-pass shapes the audit caught:
//!
//! 1. **Whole-file cites** — `:L1-L<EOF>` patterns evade scrutiny by
//!    "citing the file" without pinning an assertion block. The resolver
//!    rejects ranges spanning more than [`MAX_EXAMPLE_CITE_LINES`] lines.
//! 2. **Assertion-free cites** — a range that contains no `assert!`,
//!    `assert_eq!`, `assert_ne!`, or `panic!` macro invocation is not an
//!    example test; the resolver rejects it.
//!
//! These checks are structural — they cannot verify that the assertions
//! actually pin the cited symbol's behavior (that remains reviewer
//! judgment, encoded in the `notes` field). But they raise the floor:
//! a cite that satisfies the resolver carries a focused assertion block,
//! not a placeholder.

#![allow(clippy::missing_docs_in_private_items)]

use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Crate root resolved from `CARGO_MANIFEST_DIR` so the test runs identically
/// under `cargo test` (workspace root) and direct `cargo test --manifest-path`
/// invocations.
fn crate_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// One row in `tests/lib_reexport_coverage.toml`. Exactly one of
/// `exemption = "E..."` or non-empty `proptests` is the COVERED state; an
/// empty `proptests` with no exemption is a GAP. The gate is active
/// (Phase 3c activated; no `#[ignore]`), so any GAP entry fails the build.
#[derive(Debug, Deserialize)]
struct ManifestEntry {
    /// `module::symbol` exactly as it appears under `pub use module::{...}`
    /// in `src/lib.rs`.
    symbol: String,
    /// `tests/<file>.rs::<fn_name>` cites of proptest functions covering the
    /// symbol directly. Optional; absent ⇒ same as `[]`.
    #[serde(default)]
    proptests: Vec<String>,
    /// E1..E8 exemption category per design §Exemption categories.
    #[serde(default)]
    exemption: Option<String>,
    /// Required for `exemption = "E3" | "E4" | "E5" | "E6" | "E7"` per
    /// design §Exemption — cites the consumer-of-the-symbol proptest.
    #[serde(default)]
    consumer_proptests: Vec<String>,
    /// Required for `exemption = "E5" | "E8"` per design §Exemption — cites
    /// `<file>.rs:L<start>-L<end>` covering the Err branch (E5) or the
    /// constant-snapshot test (E8).
    #[serde(default)]
    example_tests: Vec<String>,
    /// One-line reviewer-readable justification of the row's disposition.
    /// Read by humans during code review; not checked by the gate (the
    /// gate is structural — semantic adequacy of the cited proptest is
    /// reviewer judgment, per design §Mechanism design).
    #[serde(default)]
    #[allow(dead_code)]
    notes: String,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    /// `src/lib.rs` sha the audit table was prepared against. Drift between
    /// `audited_at_sha` and the live `src/lib.rs` is fine *iff* the live
    /// re-export set still matches the manifest entries (which the gate
    /// checks); the field is reviewer-facing provenance, not a hard check.
    #[allow(dead_code)]
    audited_at_sha: String,
    /// Line range in `src/lib.rs` covered by the audit; reviewer-facing.
    #[allow(dead_code)]
    audited_against_lib_rs_lines: String,
    /// One row per `pub use` symbol.
    entries: Vec<ManifestEntry>,
}

/// Loads and parses `tests/lib_reexport_coverage.toml`.
fn load_manifest() -> Manifest {
    let path = crate_root().join("tests/lib_reexport_coverage.toml");
    let src = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read manifest {}: {e}", path.display()));
    toml::from_str(&src).unwrap_or_else(|e| panic!("parse manifest {}: {e}", path.display()))
}

/// Reads `src/lib.rs` and extracts every symbol re-exported via `pub use`.
///
/// Returns a sorted `Vec<String>` of `module::symbol` strings, matching the
/// shape used by the manifest's `symbol` field.
///
/// Parser approach: strip line- and block-comments from the source, then scan
/// for `pub use ` anchored at start-of-line (the previous char must be `\n`
/// or position 0; this rejects doc-comment occurrences and `re-pub use` style
/// false matches). For each match, read until the matching `;`, then parse
/// the brace-list or single symbol.
///
/// Intentionally hand-rolled — using `syn` would pull a proc-macro-grade
/// dependency for what is structurally `pub use foo::{a, b};` blocks. The
/// audited table in the design doc enumerates the set; this parser exists to
/// detect drift, not to handle arbitrary Rust grammar.
fn enumerate_lib_rs_pub_use_symbols() -> Vec<String> {
    let path = crate_root().join("src/lib.rs");
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let src = strip_comments(&raw);

    let mut out = Vec::new();
    let mut cursor = 0;
    while let Some(start) = src[cursor..].find("pub use ") {
        let abs_match = cursor + start;
        // Anchor at start-of-line: char before `pub` must be `\n` or this is
        // position 0. Rejects matches inside doc-comment text, string
        // literals (already stripped), or any non-item-level occurrence.
        let at_line_start = abs_match == 0 || src.as_bytes()[abs_match - 1] == b'\n';
        if !at_line_start {
            cursor = abs_match + "pub use ".len();
            continue;
        }

        let abs_start = abs_match + "pub use ".len();
        let end_rel = src[abs_start..]
            .find(';')
            .unwrap_or_else(|| panic!("unterminated `pub use` near byte {abs_start}"));
        let block = &src[abs_start..abs_start + end_rel];
        cursor = abs_start + end_rel + 1;

        // `block` is e.g. "convert::{\n    fel_to_json, fel_to_ui_json,\n}" or
        // "ast::Expr" or "parser::parse".
        parse_pub_use_block(block, &mut out);
    }
    out.sort();
    out
}

/// Strips Rust line-comments (`//...\n`) and block-comments (`/* ... */`)
/// from `src`, preserving newlines so byte offsets land near their original
/// lines. String-literal contents are NOT scanned (lib.rs has no string
/// literals embedding `pub use ...;` and the gate's scope is `pub use`
/// re-exports only).
///
/// Block comments are not nested per Rust grammar (`/* /* */ */` IS nested
/// in Rust, but lib.rs has none and the gate fails loudly on unterminated
/// blocks rather than silently mis-parsing).
fn strip_comments(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            // Line comment — skip to (but not including) the newline.
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            // Let the newline through so line numbers/anchoring stay sane.
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            // Block comment — skip to `*/`. Replace internal newlines with
            // themselves (preserve line count) but drop all other bytes.
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                if bytes[i] == b'\n' {
                    out.push('\n');
                }
                i += 1;
            }
            if i + 1 >= bytes.len() {
                panic!("unterminated block comment in source");
            }
            i += 2; // consume `*/`
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// Parses one `pub use ...` block into `module::symbol` strings, appended to
/// `out`. Handles three shapes plus alias-stripping:
///
/// * `module::symbol` (single).
/// * `module::{a, b, c}` (brace-list, single-line).
/// * `module::{\n    a, b,\n    c,\n}` (brace-list, multi-line).
/// * `module::Foo as Bar` and `module::{Foo as Bar, Baz}` — the alias
///   (post-`as` ident) is what consumers see; the gate records the alias.
fn parse_pub_use_block(block: &str, out: &mut Vec<String>) {
    let block = block.trim();
    if let Some(brace_open) = block.find('{') {
        // Brace-list shape. Module path is everything before the first `{`,
        // stripped of trailing `::`.
        let module = block[..brace_open].trim().trim_end_matches("::").trim();
        let brace_close = block
            .rfind('}')
            .unwrap_or_else(|| panic!("unterminated brace-list in `pub use {module} {{...`"));
        let inner = &block[brace_open + 1..brace_close];
        for raw in inner.split(',') {
            let sym = strip_alias(raw.trim());
            if sym.is_empty() {
                continue;
            }
            out.push(format!("{module}::{sym}"));
        }
    } else {
        // Single-symbol shape. The whole block is `module::symbol` or
        // `module::Foo as Bar`. Split off the module path, strip the alias
        // off the symbol tail.
        if let Some(last_sep) = block.rfind("::") {
            let module = &block[..last_sep];
            let tail = strip_alias(block[last_sep + 2..].trim());
            out.push(format!("{module}::{tail}"));
        } else {
            // Bare ident (no `::`) — shouldn't appear in lib.rs but record
            // it so the parser is total.
            out.push(strip_alias(block).to_string());
        }
    }
}

/// Strips a `Foo as Bar` alias to its consumer-visible name (`Bar`). If `s`
/// contains no ` as `, returns `s` unchanged.
fn strip_alias(s: &str) -> &str {
    if let Some(pos) = s.rfind(" as ") {
        s[pos + " as ".len()..].trim()
    } else {
        s
    }
}

/// Resolves a `tests/<file>.rs::<fn_name>` (or `src/<path>.rs::<...>::fn_name`)
/// cite by reading the file and grepping for `fn <fn_name>(`.
///
/// Returns `Ok(())` on resolve, `Err(msg)` on miss with a contributor-readable
/// reason. Failure messages are the gate's contributor experience — they're
/// the first thing a CI failure shows.
///
/// ## Known limitation (TODO: future tightening)
///
/// The resolver substring-matches `fn <fn_name>(` against the WHOLE file. If
/// a file defines two functions of the same name in different modules (e.g.
/// `mod a { fn foo(...) }` + `mod b { fn foo(...) }`), the resolver will
/// match either and not detect the collision. Today no `tests/*.rs` file in
/// the crate exhibits this shape; if a manifest cite ever uses the full
/// `tests/file.rs::mod::fn_name` path, the module segment is currently
/// stripped and ignored. Tighten when a real collision appears.
fn resolve_proptest_cite(cite: &str) -> Result<(), String> {
    let (file_rel, fn_name) = cite
        .split_once("::")
        .ok_or_else(|| format!("malformed proptest cite `{cite}` — expected `file.rs::fn_name`"))?;
    // For `src/foo/bar.rs::mod::fn_name` shape, the function name is the
    // LAST `::`-segment; everything between is module-path that we don't
    // verify (it would require macro-expansion to do so soundly).
    let fn_name = fn_name.rsplit("::").next().unwrap_or(fn_name);

    let abs = crate_root().join(file_rel);
    let src = fs::read_to_string(&abs).map_err(|e| {
        format!(
            "proptest cite `{cite}` — could not read `{}`: {e}",
            abs.display()
        )
    })?;
    let needle = format!("fn {fn_name}(");
    if src.contains(&needle) {
        Ok(())
    } else {
        Err(format!(
            "proptest cite `{cite}` — function `fn {fn_name}(` not found in `{}`",
            abs.display()
        ))
    }
}

/// Upper bound on the line-span of an `example_tests` cite. A focused
/// assertion block fits well under this; a cite that spans more lines is
/// almost certainly a "cite the whole file" placeholder rather than a
/// pinned assertion block. Per Phase 3e audit (BLOCKER ARCH-H1).
const MAX_EXAMPLE_CITE_LINES: usize = 80;

/// Macro tokens whose presence is treated as evidence the cited range
/// contains an assertion. `panic!` is included because some example tests
/// assert via `match ... { _ => panic!(...) }` rather than `assert!`.
const ASSERT_TOKENS: &[&str] = &["assert!", "assert_eq!", "assert_ne!", "panic!"];

/// Resolves an `<file>.rs:L<start>-L<end>` example-test cite by checking:
///
/// 1. The file exists and the range is within the file's line count.
/// 2. The range spans no more than [`MAX_EXAMPLE_CITE_LINES`] lines
///    (rejects whole-file `:L1-L<EOF>` placeholders).
/// 3. The cited line slice contains at least one assertion-shaped token
///    (`assert!`, `assert_eq!`, `assert_ne!`, or `panic!`).
///
/// Does NOT verify the assertions actually pin the cited symbol's behavior
/// — that's reviewer judgment, encoded in the manifest's `notes` field.
fn resolve_example_cite(cite: &str) -> Result<(), String> {
    let (file_rel, range) = cite.split_once(':').ok_or_else(|| {
        format!("malformed example_tests cite `{cite}` — expected `file.rs:Lstart-Lend`")
    })?;
    let range = range.trim_start_matches('L');
    let (start_str, end_str) = range
        .split_once("-L")
        .ok_or_else(|| format!("malformed example_tests cite `{cite}` — expected `Lstart-Lend`"))?;
    let start: usize = start_str
        .parse()
        .map_err(|_| format!("example_tests cite `{cite}` — start `{start_str}` not a number"))?;
    let end: usize = end_str
        .parse()
        .map_err(|_| format!("example_tests cite `{cite}` — end `{end_str}` not a number"))?;
    if start > end {
        return Err(format!(
            "example_tests cite `{cite}` — start L{start} > end L{end}"
        ));
    }
    // Span check — reject whole-file cites BEFORE doing file I/O. Bounds
    // failures already point the reviewer at the manifest, not the file.
    let span = end - start + 1;
    if span > MAX_EXAMPLE_CITE_LINES {
        return Err(format!(
            "example_tests cite `{cite}` — span {span} lines exceeds max {MAX_EXAMPLE_CITE_LINES}; \
             example cites must pin a focused assertion block, not a whole file. \
             Narrow the range to the assertions that pin the symbol's behavior."
        ));
    }
    let abs = crate_root().join(file_rel);
    let src = fs::read_to_string(&abs).map_err(|e| {
        format!(
            "example_tests cite `{cite}` — could not read `{}`: {e}",
            abs.display()
        )
    })?;
    let lines: Vec<&str> = src.lines().collect();
    let line_count = lines.len();
    if end > line_count {
        return Err(format!(
            "example_tests cite `{cite}` — end L{end} exceeds file line count {line_count} in `{}`",
            abs.display()
        ));
    }
    // Assertion-content check — the cited slice must contain at least one
    // assertion macro invocation. Lines are 1-indexed in cites; slice
    // accordingly (`start..=end`).
    let cited_slice = lines[start - 1..end].join("\n");
    let has_assert = ASSERT_TOKENS
        .iter()
        .any(|token| cited_slice.contains(token));
    if !has_assert {
        return Err(format!(
            "example_tests cite `{cite}` — cited slice L{start}-L{end} contains no assertion \
             macro (one of {ASSERT_TOKENS:?}). A range that contains no assertion is not an \
             example test. Either tighten the range or fix the cited tests."
        ));
    }
    Ok(())
}

/// Allowed exemption categories per design §Exemption categories.
const EXEMPTION_CATEGORIES: &[&str] = &["E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8"];

/// Categories that REQUIRE `consumer_proptests = [...]` per design §Exemption.
/// E5 additionally requires `example_tests`; E8 ONLY requires `example_tests`.
const CONSUMER_CITE_REQUIRED: &[&str] = &["E3", "E4", "E5", "E6", "E7"];

/// Categories that REQUIRE `example_tests = [...]` per design §Exemption.
const EXAMPLE_CITE_REQUIRED: &[&str] = &["E5", "E8"];

/// The gate proper. Activated in Phase 3c — runs by default under
/// `cargo test`. Activation commit subject:
/// `test(gate): activate lib_reexport_coverage_gate` per design
/// §Implementation plan — Commit message convention.
#[test]
fn lib_reexport_coverage_gate() {
    let manifest = load_manifest();
    let lib_symbols = enumerate_lib_rs_pub_use_symbols();

    // Index manifest entries by symbol for O(1) lookup and duplicate detection.
    let mut by_symbol: std::collections::HashMap<&str, &ManifestEntry> =
        std::collections::HashMap::new();
    let mut errors: Vec<String> = Vec::new();
    for entry in &manifest.entries {
        if by_symbol.insert(entry.symbol.as_str(), entry).is_some() {
            errors.push(format!(
                "DUPLICATE manifest entry for symbol `{}` — every `pub use` must appear exactly once",
                entry.symbol
            ));
        }
    }

    let lib_set: HashSet<&str> = lib_symbols.iter().map(String::as_str).collect();
    let manifest_set: HashSet<&str> = by_symbol.keys().copied().collect();

    // 1) Every `pub use` in lib.rs must have a manifest entry.
    for sym in &lib_symbols {
        if !manifest_set.contains(sym.as_str()) {
            errors.push(format!(
                "MISSING manifest entry for `{sym}` — `src/lib.rs` declares this re-export but \
                 `tests/lib_reexport_coverage.toml` has no row. \
                 Add an entry per `thoughts/2026-05-23-phase-3-ci-gate-design.md` §Failure mode."
            ));
        }
    }

    // 2) Every manifest entry must correspond to a live `pub use` (stale entries
    //    are also a fail — they mask removed APIs).
    for sym in &manifest_set {
        if !lib_set.contains(sym) {
            errors.push(format!(
                "STALE manifest entry for `{sym}` — `tests/lib_reexport_coverage.toml` has a row \
                 but `src/lib.rs` no longer re-exports this symbol. Remove the entry or restore the \
                 re-export."
            ));
        }
    }

    // 3) Per-entry shape checks.
    for entry in &manifest.entries {
        let sym = &entry.symbol;

        // Exemption shape.
        if let Some(exemption) = &entry.exemption {
            if !EXEMPTION_CATEGORIES.contains(&exemption.as_str()) {
                errors.push(format!(
                    "`{sym}` exemption `{exemption}` is not one of E1..E8 \
                     (see design §Exemption categories)."
                ));
                continue;
            }
            if CONSUMER_CITE_REQUIRED.contains(&exemption.as_str())
                && entry.consumer_proptests.is_empty()
            {
                errors.push(format!(
                    "`{sym}` exemption `{exemption}` requires `consumer_proptests = [...]` \
                     (design §Exemption categories — E3/E4/E5/E6/E7 normative)."
                ));
            }
            if EXAMPLE_CITE_REQUIRED.contains(&exemption.as_str()) && entry.example_tests.is_empty()
            {
                errors.push(format!(
                    "`{sym}` exemption `{exemption}` requires `example_tests = [...]` \
                     (design §Exemption categories — E5/E8 normative)."
                ));
            }
        } else if entry.proptests.is_empty() {
            // GAP: no exemption AND no proptests. Allowed at landing time
            // (manifest can carry Phase 3a/3b-pending rows), forbidden at
            // activation. The activation commit fails until every GAP is
            // closed or exempt.
            errors.push(format!(
                "`{sym}` is a GAP — no proptests cited and no exemption. \
                 Either land a proptest and add the cite, or exempt with E1..E8 \
                 (see `thoughts/2026-05-23-phase-3-ci-gate-design.md` §Failure mode for the choices)."
            ));
        }

        // Cite resolution — applies regardless of exemption.
        for cite in &entry.proptests {
            if let Err(reason) = resolve_proptest_cite(cite) {
                errors.push(format!("`{sym}` — {reason}"));
            }
        }
        for cite in &entry.consumer_proptests {
            if let Err(reason) = resolve_proptest_cite(cite) {
                errors.push(format!("`{sym}` consumer_proptests — {reason}"));
            }
        }
        for cite in &entry.example_tests {
            if let Err(reason) = resolve_example_cite(cite) {
                errors.push(format!("`{sym}` example_tests — {reason}"));
            }
        }
    }

    if !errors.is_empty() {
        panic!(
            "lib_reexport_coverage_gate found {} issue(s):\n\n  - {}\n",
            errors.len(),
            errors.join("\n  - ")
        );
    }
}

// ── Self-tests for the parser/resolvers ─────────────────────────────────────
//
// These run UNDER `cargo test` (no `#[ignore]`) so the gate's infrastructure
// is exercised even before Phase 3c activation. They cover the
// `pub use` parser and the cite resolver against known shapes, NOT the
// manifest contents.

#[test]
fn parser_handles_single_symbol_pub_use() {
    let mut out = Vec::new();
    parse_pub_use_block("ast::Expr", &mut out);
    assert_eq!(out, vec!["ast::Expr".to_string()]);
}

#[test]
fn parser_handles_single_line_brace_list() {
    let mut out = Vec::new();
    parse_pub_use_block(
        "environment::{FormspecEnvironment, MipState, RepeatContext}",
        &mut out,
    );
    assert_eq!(
        out,
        vec![
            "environment::FormspecEnvironment".to_string(),
            "environment::MipState".to_string(),
            "environment::RepeatContext".to_string(),
        ]
    );
}

#[test]
fn parser_handles_multi_line_brace_list_with_trailing_comma() {
    let block = "convert::{\n    fel_to_json, fel_to_ui_json,\n    json_to_fel,\n}";
    let mut out = Vec::new();
    parse_pub_use_block(block, &mut out);
    assert_eq!(
        out,
        vec![
            "convert::fel_to_json".to_string(),
            "convert::fel_to_ui_json".to_string(),
            "convert::json_to_fel".to_string(),
        ]
    );
}

#[test]
fn enumerate_picks_up_every_lib_rs_symbol() {
    // Smoke-test: the enumeration must return a non-empty, sorted, deduplicated
    // set covering known anchors across every `pub use` shape in lib.rs.
    // Failures here mean the parser broke, not the manifest.
    //
    // Anchors deliberately cover:
    //   - single-symbol `pub use ast::Expr;`                  (line 35)
    //   - single-symbol `pub use parser::parse;`              (line 68)
    //   - third-party single `pub use indexmap::IndexMap;`    (line 61)
    //   - single-line brace-list (none currently in lib.rs)
    //   - multi-line brace-list `pub use convert::{...};`     (line 37-40)
    //   - multi-line brace-list `pub use error::{...};`       (line 46-50)
    //   - single-segment brace-list element `evaluator::evaluate` (line 51-55)
    let symbols = enumerate_lib_rs_pub_use_symbols();
    assert!(!symbols.is_empty(), "lib.rs has zero pub use symbols?");
    let set: HashSet<&str> = symbols.iter().map(String::as_str).collect();
    for anchor in &[
        "ast::Expr",
        "parser::parse",
        "evaluator::evaluate",
        "indexmap::IndexMap",
        // Brace-list anchors (CODE-M3 — Phase 3e remediation).
        "convert::fel_to_json",
        "error::Error",
    ] {
        assert!(
            set.contains(anchor),
            "enumeration missed anchor `{anchor}` — parser regressed"
        );
    }
}

// ── Phase 3e parser hardening self-tests ────────────────────────────────────

#[test]
fn parser_strips_alias_in_single_symbol_pub_use() {
    // `pub use foo::Bar as Baz;` — the consumer-visible name is `Baz`,
    // so the parser must record `foo::Baz` (not `foo::Bar as Baz`).
    let mut out = Vec::new();
    parse_pub_use_block("foo::Bar as Baz", &mut out);
    assert_eq!(out, vec!["foo::Baz".to_string()]);
}

#[test]
fn parser_strips_alias_in_brace_list() {
    let mut out = Vec::new();
    parse_pub_use_block("foo::{Bar as Baz, Qux}", &mut out);
    assert_eq!(out, vec!["foo::Baz".to_string(), "foo::Qux".to_string()]);
}

#[test]
fn strip_comments_drops_line_comments() {
    // Doc-comments containing `pub use` literal must not survive the strip,
    // so they can never reach the `pub use ` scanner. Verify by scanning
    // the stripped output for the comment marker.
    let src = "// pub use foo::Bar;\npub use foo::Baz;\n";
    let stripped = strip_comments(src);
    assert!(
        !stripped.contains("//"),
        "line-comment marker must be stripped"
    );
    // The doc-comment payload is gone but the real `pub use` survives.
    assert!(
        stripped.contains("pub use foo::Baz;"),
        "real pub use must survive comment stripping"
    );
}

#[test]
fn strip_comments_drops_block_comments() {
    let src = "/* pub use phony::Ghost; */\npub use real::Symbol;\n";
    let stripped = strip_comments(src);
    assert!(
        !stripped.contains("phony"),
        "block-comment contents must be stripped"
    );
    assert!(
        stripped.contains("pub use real::Symbol;"),
        "real pub use after block comment must survive"
    );
}

#[test]
fn enumerate_ignores_pub_use_in_line_comment() {
    // Surgical test for the doc-comment false-positive ARCH-H2/CODE-H1
    // closed: the parser scans STRIPPED source, so a `pub use` literal
    // inside a `//` comment cannot inject a phantom symbol.
    //
    // We exercise this through `strip_comments` + `parse_pub_use_block`
    // directly rather than mutating lib.rs. The real lib.rs assertion is
    // `enumerate_picks_up_every_lib_rs_symbol` above, which would FAIL if
    // the parser invented `module::Symbol` entries the manifest does not
    // declare (the gate enforces equality).
    let stripped = strip_comments("// pub use ghost::Phantom;\npub use real::Anchor;\n");
    let mut out = Vec::new();
    // Anchor-at-line-start emulation: the stripped source has the comment
    // line as whitespace-only (the `//...` removed, newline kept), so the
    // brace-list parser sees only the real `pub use`. We feed the BLOCK
    // (post-`pub use ` strip) to confirm the inner parsing also doesn't
    // get confused by surrounding whitespace.
    parse_pub_use_block("real::Anchor", &mut out);
    assert_eq!(out, vec!["real::Anchor".to_string()]);
    // The stripped form should not contain `ghost`.
    assert!(!stripped.contains("ghost"));
}

// ── Phase 3e example-cite hardening self-tests ──────────────────────────────

#[test]
fn example_resolver_rejects_whole_file_cite() {
    // `:L1-L<EOF>` patterns evade scrutiny. The resolver rejects ranges
    // wider than MAX_EXAMPLE_CITE_LINES regardless of file content
    // (ARCH-H1 — Phase 3e remediation).
    let err = resolve_example_cite("tests/parser_rejection_tests.rs:L1-L263")
        .expect_err("whole-file cite must be rejected by the span check");
    assert!(
        err.contains("span 263 lines exceeds max"),
        "error must name the span violation, got: {err}"
    );
}

#[test]
fn example_resolver_rejects_assertion_free_cite() {
    // A focused range that contains no assert/panic macro is not an
    // example test. parser_rejection_tests.rs lines 1-20 are the file
    // header (`//!` docs + imports) — no assertions.
    let err = resolve_example_cite("tests/parser_rejection_tests.rs:L1-L20")
        .expect_err("assertion-free cite must be rejected");
    assert!(
        err.contains("contains no assertion macro"),
        "error must name the missing-assertion violation, got: {err}"
    );
}

#[test]
fn resolver_finds_known_proptest() {
    // A real proptest function in the suite — failure here means the resolver
    // regressed, not the gate.
    resolve_proptest_cite("tests/ast_proptest.rs::parse_print_identity")
        .expect("known proptest must resolve");
}

#[test]
fn resolver_rejects_missing_function() {
    let err = resolve_proptest_cite("tests/ast_proptest.rs::definitely_not_a_real_fn_name")
        .expect_err("non-existent function must not resolve");
    assert!(err.contains("function `fn definitely_not_a_real_fn_name(`"));
}

#[test]
fn resolver_rejects_missing_file() {
    let err = resolve_proptest_cite("tests/no_such_file.rs::whatever")
        .expect_err("missing file must not resolve");
    assert!(err.contains("could not read"));
}

#[test]
fn example_resolver_validates_line_range() {
    // parser_rejection_tests.rs is 263 lines (per design §Failure mode anchor).
    resolve_example_cite("tests/parser_rejection_tests.rs:L225-L263")
        .expect("range within file must resolve");
}

#[test]
fn example_resolver_rejects_out_of_range() {
    // Cite a small (within-span) range past the file's EOF. The file has
    // 263 lines; cite L300-L320 (21 lines, well under the span cap).
    let err = resolve_example_cite("tests/parser_rejection_tests.rs:L300-L320")
        .expect_err("out-of-range must not resolve");
    assert!(
        err.contains("exceeds file line count"),
        "error must name the line-count violation, got: {err}"
    );
}

#[test]
fn example_resolver_rejects_inverted_range() {
    let err = resolve_example_cite("tests/parser_rejection_tests.rs:L200-L100")
        .expect_err("start > end must not resolve");
    assert!(err.contains("start L200 > end L100"));
}
