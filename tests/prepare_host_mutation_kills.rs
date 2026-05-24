//! Targeted mutation-kill tests for `prepare_host.rs` survivors classified
//! in `thoughts/2026-05-23-mutation-survivor-followups.md` (FUT-16).
//!
//! Each test cluster names the mutant(s) it kills with a `src/prepare_host.rs:LINE:COL`
//! locator and a one-line summary of the substitution. These are direct
//! integration tests against the `prepare`, `prepare_for_host`, and
//! `host_options_from_json` public entry points — complementing the
//! property tests in `prepare_host_proptest.rs` which cover broader
//! invariants but undergenerate on these specific boundary cases.
//!
//! Investigation note (FUT-16-INVESTIGATE): the followups doc flagged
//! `:312:5` and `:320:5` as "suspicious survivors despite seemingly-
//! covering inline tests." Hand-trace + cargo-mutants re-run confirmed
//! both are genuinely test-coverage equivalent for the existing
//! `repeat_alias_implicit_and_explicit` test (positions either
//! short-circuit at `i == 0` or are blocked by adjacent ident/`$`/`.`
//! chars regardless of mutation). The "investigate cargo-mutants config"
//! hypothesis was wrong — no harness bug. Kill tests below pin both
//! mutants by constructing inputs where the prefix is genuinely not
//! blocked and the needle would not naturally match.
#![allow(clippy::missing_docs_in_private_items)]

use std::collections::HashMap;

use fel_core::{
    PrepareHostInput, PrepareHostOptions, host_options_from_json, prepare, prepare_for_host,
};
use serde_json::{Map, Value, json};

fn prep(
    expression: &str,
    current_item_path: &str,
    replace_self_ref: bool,
    repeats: &[(&str, u32)],
    paths: &[&str],
) -> String {
    let rc: HashMap<String, u32> = repeats.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    let fp: Vec<String> = paths.iter().map(|s| (*s).to_string()).collect();
    prepare_for_host(PrepareHostInput {
        expression,
        current_item_path,
        replace_self_ref,
        repeat_counts: &rc,
        field_paths: &fp,
    })
}

// ─────────────────────────────────────────────────────────────────────
// Cluster P2 — is_ident_start edge cases (src/prepare_host.rs:50:5, 50:34)
// ─────────────────────────────────────────────────────────────────────

/// Kills `:50:5 → true` (every char is an ident-start). With the mutant,
/// `$group.1foo` would extract "1foo" as a field name (and rewrite),
/// because digit char passes `is_ident_start`. Original rejects (digits
/// are not ident starts) and leaves `$group.1foo` unchanged.
///
/// Also kills `:50:5 → false` (no char is an ident-start) — original
/// rewrites `$group.foo`, mutant leaves it unchanged.
#[test]
fn qualified_group_ref_with_digit_after_dot_is_not_rewritten() {
    // path supplies repeat ancestor; expression has `$group.1foo` (digit-led).
    let out = prep("$group.1foo + 1", "group[0].total", false, &[("group", 2)], &[]);
    // Original: 1foo is not a valid ident start, so the whole `$group.1foo`
    // tail is not consumed → leaves text as-is for the qualified-group pass
    // to skip; the `.` then prevents trailing rewrite. Expected: unchanged.
    assert_eq!(out, "$group.1foo + 1");
}

/// Companion to the above — confirms the normal (alphabetic) field is
/// rewritten so the digit-rejection is a real discrimination. Pins the
/// `:50:5 → false` mutant by asserting the rewrite happens.
#[test]
fn qualified_group_ref_with_alpha_after_dot_is_rewritten() {
    let out = prep("$group.foo + 1", "group[0].total", false, &[("group", 2)], &[]);
    assert_eq!(out, "foo + 1");
}

/// Kills `:50:34 delete match` (the underscore branch). Original allows
/// `_foo` as a field name; mutant rejects → leaves `$group._foo` unchanged.
#[test]
fn qualified_group_ref_with_underscore_field_is_rewritten() {
    let out = prep(
        "$group._foo + 1",
        "group[0].total",
        false,
        &[("group", 2)],
        &[],
    );
    assert_eq!(out, "_foo + 1");
}

// ─────────────────────────────────────────────────────────────────────
// Cluster P3 — step_quote escape handler (src/prepare_host.rs:83:18, 83:26, 83:38, 83:42)
// ─────────────────────────────────────────────────────────────────────

/// Kills `:83:18 ==→!=` and `:83:26 &&→||`. The output stream for both
/// mutants happens to be identical to the original when chars inside a
/// quote are pair-consumed — `'abc'` reads as `[a,b][c,']` (pairs) in
/// mutant vs `[a],[b],[c],[']` in original, but the OUTPUT chars are
/// the same in both cases.
///
/// The real discriminator: when c is the CLOSING quote `'`, original
/// hits `c == q` and resets `quote = None`. Mutant `!=` (or `||`) sees
/// `'` != `\\` and takes the escape branch instead, SKIPPING the
/// quote-end check. The cursor then stays in "in-quote" state past the
/// close quote, so the bare-`$` substitution in the outer loop is
/// suppressed for chars that should be outside the quote.
///
/// Discriminating input: a bare `$` AFTER a closing quote. Original
/// substitutes it (out of quote); mutant treats it as in-quote (skips
/// substitution).
#[test]
fn step_quote_detects_close_quote_against_dollar_substitution() {
    let out = prep("$ + 'a' + $", "items[0].qty", true, &[], &[]);
    // Original: `$qty + 'a' + $qty` — both `$` are bare and outside any
    // quote, so both get substituted.
    // Mutant `:83:18 ==→!=` and `:83:26 &&→||`: cursor stays in-quote
    // past the close `'`, so the second `$` is treated as in-quote and
    // skipped → `$qty + 'a' + $`.
    assert_eq!(out, "$qty + 'a' + $qty");
}

/// Kills `:83:38 +→-`, `:83:38 +→*`, `:83:42 <→<=` — all three are OOB
/// shapes on the bounds check `self.idx + 1 < self.len()`. The
/// discriminating input is a backslash AT the very last position of the
/// entire expression (inside an unclosed quote).
///
/// - Original: `idx + 1 < len` is FALSE at `idx == len - 1` → escape
///   branch skipped → safe.
/// - `+→-`: `idx - 1 < len` is TRUE for `idx >= 1` → fires → reads
///   `chars[idx + 1]` = `chars[len]` → panic.
/// - `+→*`: `idx * 1 = idx` and `idx < len` is TRUE → fires → panic.
/// - `<→<=`: `idx + 1 <= len` is TRUE at `idx == len - 1` → fires → panic.
///
/// `prepare_for_host` doesn't parse the input; an unclosed quote with a
/// trailing backslash is valid as a normalization input. Original returns
/// the normalized string (with the leading `$` rewritten). Mutants panic
/// before returning, which the test runner catches as a failure.
#[test]
fn step_quote_handles_trailing_backslash_at_expression_end() {
    let out = prep("$ + '\\", "items[0].qty", true, &[], &[]);
    // Expression ends with `\` inside an unclosed quote — last char of
    // the whole expression. Original safely skips the escape branch.
    // Mutants index past `chars.len()` and panic.
    assert_eq!(out, "$qty + '\\");
}

/// Kills `:83:18 ==→!=`, `:83:26 &&→||` further by exercising a real
/// in-string escape pair (`\n` inside the quote). Original consumes
/// the `\` + `n` as an atomic pair; mutants either over-consume or
/// mis-recognize the close quote.
#[test]
fn step_quote_handles_backslash_escape_sequence_inside_quote() {
    // `'a\nb'` inside the expression: the escape handler must consume
    // `\` AND `n` as a pair, then resume scanning. After the pair the
    // cursor is at `b`, then the close quote `'`.
    let out = prep("$ + 'a\\nb' + $", "items[0].qty", true, &[], &[]);
    // Both `$` are outside the quote. Original substitutes both →
    // `$qty + 'a\nb' + $qty`. Mutants that mis-handle the escape end
    // up still in-quote past the close `'`, suppressing the second
    // substitution.
    assert_eq!(out, "$qty + 'a\\nb' + $qty");
}

// ─────────────────────────────────────────────────────────────────────
// Cluster P4 — replace_bare_current_field_refs prefix check
// (src/prepare_host.rs:216:37, 216:60 -→+)
// ─────────────────────────────────────────────────────────────────────

/// Kills `:216:37 delete !` (the prefix-check `!is_ident_char` inversion).
/// For `a$ + 1`: original `prev_ok = !is_ident_char('a') = false` → no
/// rewrite → output unchanged. Mutant: `prev_ok = is_ident_char('a') = true`
/// → fires rewrite → output `a$qty + 1`.
#[test]
fn bare_dollar_with_ident_prefix_is_not_rewritten() {
    let out = prep("a$ + 1", "items[0].qty", true, &[], &[]);
    assert_eq!(out, "a$ + 1");
}

/// Kills `:216:60 - → +` (chars[i-1] → chars[i+1] — wrong char indexed
/// for prefix check). Discriminating shape: `$` at a non-start position
/// where chars[i-1] is ident (blocked in orig) but chars[i+1] is NOT
/// ident (unblocked in mutant). E.g., `0$ + 1` at i=1: orig prev='0'
/// (ident, blocked); mutant prev=chars[2]=' ' (not ident, unblocked).
/// next_ok at i+1=' ' is also unblocked → mutant fires → `0$qty + 1`.
/// Orig: no fire → `0$ + 1`.
#[test]
fn bare_dollar_with_digit_prefix_is_not_rewritten() {
    let out = prep("0$ + 1", "items[0].qty", true, &[], &[]);
    assert_eq!(out, "0$ + 1");
}

// ─────────────────────────────────────────────────────────────────────
// Cluster P5 — replace_qualified_group_ref_outside_quotes arithmetic
// (src/prepare_host.rs:281:27, 281:31, 281:51, 285:58 ×2, 285:62, 289:25 ×2)
// ─────────────────────────────────────────────────────────────────────

/// Umbrella test for `:281:27 +→*`, `:281:31 +→*` — both make `has_group`
/// always false, blocking ALL `$group.field` rewrites. Any non-trivial
/// qualified rewrite kills both.
///
/// Already covered by `qualified_group_ref_with_alpha_after_dot_is_rewritten`
/// above. This test adds a multi-char field name to distinguish off-by-one
/// boundary mutants from total-failure mutants.
#[test]
fn qualified_group_ref_extracts_full_field_name() {
    // Multi-char field name discriminates `:289:25 <→==` and `:289:25 <→>`
    // which truncate the field name to ≤1 char.
    let out = prep(
        "$group.longfieldname + 1",
        "group[0].total",
        false,
        &[("group", 2)],
        &[],
    );
    assert_eq!(out, "longfieldname + 1");
}

/// Kills `:281:51 <→<=` (off-by-one allows `dot_idx == chars.len()` →
/// `chars[dot_idx]` panics). Input: `$group` at the very end of the
/// expression, with NOTHING following. Original `i + 1 + len < chars.len()`
/// is FALSE for this case (no room for the dot) → skip. Mutant: `<=` →
/// TRUE → tries to access `chars[dot_idx]` where dot_idx == len → panic.
#[test]
fn qualified_group_ref_at_end_of_expr_without_dot_does_not_panic() {
    // Expression ends with `$group` — no dot, no field. Original: has_group
    // is false (no room for `.field`), so skip. Mutant `:281:51 <=`: index
    // OOB → panic. Test catches panic by asserting normal return.
    let out = prep("a + $group", "group[0].total", false, &[("group", 2)], &[]);
    // No dot after $group → no qualified rewrite; expression unchanged.
    assert_eq!(out, "a + $group");
}

/// Kills `:285:58 +→-`, `:285:58 +→*`, `:285:62 <→<=` — OOB on `$group.`
/// (trailing dot, no field char). Original: `dot_idx + 1 < len` is FALSE
/// when dot is at end → skip. Mutants: either index dot_idx-1 (always
/// passes for dot_idx >= 1), dot_idx (passes when dot_idx < len), or
/// dot_idx+1 == len (`<=` accepts) → `chars[field_start]` where
/// field_start == len → panic.
#[test]
fn qualified_group_ref_with_trailing_dot_no_field_does_not_panic() {
    let out = prep("a + $group.", "group[0].total", false, &[("group", 2)], &[]);
    // Trailing dot with nothing after — original skips the rewrite block;
    // expression unchanged. Mutants panic on the OOB access.
    assert_eq!(out, "a + $group.");
}

/// Kills `:289:25 <→==`. The discriminating shape is OOB panic: at a
/// single-char field where `field_start + 1 == chars.len()`, `j == len`
/// becomes true → `is_ident_char(chars[len])` indexes one past the end
/// → panic.
///
/// Note on `:289:25 <→>`: the outer-loop char-by-char fallback in
/// `replace_qualified_group_ref_outside_quotes` compensates for a
/// truncated field name in EVERY reachable case — the function pushes
/// the truncated field, then the outer cursor pushes remaining chars
/// individually, and since the qualified-rewrite only fires on `$`
/// prefixes (and `$` is not an ident-char so the original loop also
/// stops there), output bytes are identical. `<→>` is genuinely test-
/// coverage equivalent → documented as RESIDUAL, not pinnable without
/// a code change that observes field length directly.
#[test]
fn qualified_group_ref_single_char_field_at_end_does_not_panic() {
    let out = prep("$group.t", "group[0].x", false, &[("group", 2)], &[]);
    // Original: `8 < 8` false → loop skipped → field = "t" → output "t".
    // Mutant `<→==`: `8 == 8` true → enters loop, indexes chars[8] → panic.
    assert_eq!(out, "t");
}

// ─────────────────────────────────────────────────────────────────────
// Cluster P6 — is_blocked_implicit_prefix / slice_eq_chars substitutions
// (src/prepare_host.rs:312:5 →true, 320:5 →true)
// ─────────────────────────────────────────────────────────────────────

/// Kills `:312:5 → true` (is_blocked_implicit_prefix always true → implicit
/// alias rewrites never fire at non-start positions).
///
/// Discriminating input: alias appears at position > 0 with a prefix that
/// is NOT in the blocked set (e.g., space, `+`, `(`). The existing
/// `repeat_alias_implicit_and_explicit` test has aliases only at i==0 or
/// with blocked prefixes (`$`, `.`), which is why the mutant survived.
#[test]
fn implicit_alias_with_unblocked_prefix_is_rewritten_at_non_start() {
    // Alias = "rows.score" (from field_paths). Expression: `(rows.score)`
    // — alias at position 1, prefix = '(' (not in blocked set).
    // Original: fires → `($rows[*].score)`.
    // Mutant `:312:5 → true`: blocked → no fire → `(rows.score)`.
    let out = prep(
        "(rows.score)",
        "",
        false,
        &[],
        &["rows[0].score", "rows[1].score"],
    );
    assert_eq!(out, "($rows[*].score)");
}

/// Kills `:320:5 → true` (slice_eq_chars always true → implicit alias
/// rewrites fire at positions where the alias text does NOT actually
/// appear).
///
/// Discriminating input: an expression whose first chars are NOT the
/// alias, with no ident-char prefix and a non-continuing suffix at
/// position `alias.len()`. With the mutant, slice_eq lies and the
/// rewrite fires anyway, replacing non-alias text with the wildcard.
#[test]
fn implicit_alias_does_not_fire_when_needle_does_not_match() {
    // Alias = "rows.score" (10 chars). Expression: `abcdefghij + 1` —
    // first 10 chars are NOT "rows.score", suffix at pos 10 is ' '
    // (not continuing), i == 0 (prefix ok).
    // Original: no slice match → no fire → `abcdefghij + 1`.
    // Mutant `:320:5 → true`: slice match lies → fires → `$rows[*].score + 1`.
    let out = prep(
        "abcdefghij + 1",
        "",
        false,
        &[],
        &["rows[0].score", "rows[1].score"],
    );
    assert_eq!(out, "abcdefghij + 1");
}

// ─────────────────────────────────────────────────────────────────────
// Cluster P7 — alias-prefix arithmetic (src/prepare_host.rs:334:63 -→/, -→+)
// ─────────────────────────────────────────────────────────────────────

/// Kills `:334:63 - → /` and `:334:63 - → +`. The prefix-blocked guard
/// reads `chars[i - 1]`. Mutants substitute `chars[i / 1] = chars[i]`
/// (the alias's first char) or `chars[i + 1]` (the alias's second char)
/// — both ident-chars, both blocked in the mutant.
///
/// Discriminating input: alias at a non-start position with chars[i-1]
/// NOT ident (unblocked in orig). For ` rows.score + 1` at i=1: orig
/// prev=' ' (not blocked) → fires. Mutant -→/: chars[1]='r' (ident,
/// blocked) → no fire. Mutant -→+: chars[2]='o' (ident, blocked) →
/// no fire.
#[test]
fn implicit_alias_with_ident_prefix_at_non_start_is_not_rewritten() {
    let out = prep(
        " rows.score + 1",
        "",
        false,
        &[],
        &["rows[0].score", "rows[1].score"],
    );
    assert_eq!(out, " $rows[*].score + 1");
}

// ─────────────────────────────────────────────────────────────────────
// Cluster P8 — host_options_from_json u32 range check
// (src/prepare_host.rs:420:22 <=→>)
// ─────────────────────────────────────────────────────────────────────

/// Kills `:420:22 <= → >`. The guard `n <= u32::MAX as u64` accepts
/// normal repeat counts. Mutant accepts ONLY n > u32::MAX, so tiny counts
/// are silently dropped from `repeat_counts`.
///
/// This is also the FIRST integration test of `host_options_from_json`
/// (the function had zero integration tests per the triage doc).
#[test]
fn host_options_from_json_parses_realistic_repeat_counts() {
    let raw = json!({
        "expression": "$line_items.qty * $line_items.price",
        "currentItemPath": "line_items[0].total",
        "replaceSelfRef": false,
        "repeatCounts": {
            "line_items": 2,
            "orders": 1
        },
        "fieldPaths": [
            "line_items[0].qty",
            "line_items[0].price",
            "line_items[1].qty",
            "line_items[1].price"
        ]
    });
    let obj = raw.as_object().expect("json object").clone();
    let opts = host_options_from_json(&obj).expect("parse ok");

    assert_eq!(opts.expression, "$line_items.qty * $line_items.price");
    assert_eq!(opts.current_item_path, "line_items[0].total");
    assert!(!opts.replace_self_ref);
    assert_eq!(opts.repeat_counts.get("line_items"), Some(&2));
    assert_eq!(opts.repeat_counts.get("orders"), Some(&1));
    assert_eq!(opts.field_paths.len(), 4);

    // End-to-end through `prepare` — kills `:457:5` substitutions too
    // (Cluster P9) because the result MUST be the actual normalized
    // expression, not "xyzzy" or "".
    let out = prepare(&opts);
    assert_eq!(out, "qty * price");
}

/// Kills `:420:22 <=→>` more sharply by asserting an empty `repeat_counts`
/// would be observable downstream. With the mutant, `line_items` is
/// dropped → `get_repeat_ancestors` returns empty → `$line_items.qty`
/// passes through unchanged.
#[test]
fn host_options_from_json_repeat_counts_survive_downstream_rewrite() {
    let mut obj = Map::new();
    obj.insert("expression".into(), Value::String("$line_items.qty".into()));
    obj.insert("currentItemPath".into(), Value::String("line_items[0].total".into()));
    obj.insert("repeatCounts".into(), json!({"line_items": 3}));

    let opts = host_options_from_json(&obj).expect("parse ok");
    // Mutant `:420:22 <=→>`: opts.repeat_counts is empty.
    assert_eq!(opts.repeat_counts.get("line_items"), Some(&3));

    let out = prepare(&opts);
    // Original: ancestor exists (innermost) → `$line_items.qty` → `qty`.
    // Mutant: ancestor doesn't exist → `$line_items.qty` passes through.
    assert_eq!(out, "qty");
}

// ─────────────────────────────────────────────────────────────────────
// Cluster P9 — prepare wrapper substitution
// (src/prepare_host.rs:457:5 → "xyzzy" / → String::new())
// ─────────────────────────────────────────────────────────────────────

/// Kills both `:457:5 → "xyzzy"` and `:457:5 → String::new()` substitutions
/// of the `prepare` wrapper. The wrapper must delegate to `prepare_for_host`,
/// not return a sentinel.
///
/// (Also covered by `host_options_from_json_parses_realistic_repeat_counts`
/// above via the final `prepare(&opts)` assertion; this test pins the
/// wrapper directly without the JSON-parsing layer.)
#[test]
fn prepare_wrapper_delegates_to_prepare_for_host() {
    let opts = PrepareHostOptions {
        expression: "$line_items.qty + $line_items.price".to_string(),
        current_item_path: "line_items[0].total".to_string(),
        replace_self_ref: false,
        repeat_counts: HashMap::from([("line_items".to_string(), 2u32)]),
        field_paths: Vec::new(),
    };
    let out = prepare(&opts);
    // Mutant `→ "xyzzy"`: returns "xyzzy" — fails.
    // Mutant `→ String::new()`: returns "" — fails.
    // Original: delegates → "qty + price".
    assert_eq!(out, "qty + price");
}

// ─────────────────────────────────────────────────────────────────────
// Cluster P10 — prepare_for_host self-ref outer guard
// (src/prepare_host.rs:470:31 &&→||)
// ─────────────────────────────────────────────────────────────────────

/// Kills `:470:31 && → ||`. The guard `replace_self_ref && !leaf.is_empty()`
/// gates the `replace_bare_current_field_refs` call. With `&&→||`, the
/// call fires when EITHER `replace_self_ref` is true OR leaf is non-empty,
/// so `replace_self_ref=false` with a non-empty `current_item_path`
/// spuriously rewrites bare `$`.
///
/// Per the triage doc (line 690): this is the primary purpose of the
/// outer guard — the `replace_self_ref=false` short-circuit. The inner
/// guard at `:202` handles only the empty-leaf case, so `:470` is the
/// SOLE line of defense for `replace_self_ref=false` with a non-empty
/// path.
#[test]
fn prepare_for_host_skips_self_ref_when_replace_self_ref_is_false() {
    let out = prep("$ * 2", "items[0].qty", false, &[], &[]);
    // Original: replace_self_ref=false → short-circuits → "$ * 2" unchanged.
    // Mutant `&&→||`: leaf="qty" is non-empty → fires → "$qty * 2".
    assert_eq!(out, "$ * 2");
}

/// Companion test pinning the symmetric branch — `replace_self_ref=true`
/// with empty leaf should also short-circuit. This kills the same mutant
/// from the other direction (replace_self_ref=true || leaf.is_empty()=true
/// → mutant fires anyway with empty leaf, but inner `:202` guard catches
/// it; orig would have fired but inner guard also catches it, so this
/// branch is doubly-defended).
///
/// More importantly: this confirms the outer guard's intent is the
/// `replace_self_ref=false` short-circuit, NOT the empty-leaf check
/// (which is redundant with `:202`). If someone later refactors `:202`
/// away, the outer guard at `:470` still defends `replace_self_ref=false`.
#[test]
fn prepare_for_host_skips_self_ref_when_path_has_no_leaf() {
    let out = prep("$ * 2", "", true, &[], &[]);
    // Both orig and mutant produce "$ * 2" because the inner `:202` guard
    // bails on empty current_field. This test pins behavior at the
    // outer-guard layer.
    assert_eq!(out, "$ * 2");
}
