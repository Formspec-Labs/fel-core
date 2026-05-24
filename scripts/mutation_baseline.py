#!/usr/bin/env python3
"""Convert cargo-mutants outcomes.json → conformance/mutation-baseline.jsonl row.

Phase 2 mutation gate (see thoughts/2026-05-23-test-suite-triage.md).

Reads `mutants.out/outcomes.json` after a `cargo mutants` run and appends one
line per mutated file to `conformance/mutation-baseline.jsonl`:

    {"file": "...", "total": N, "killed": K, "missed": M, "timeout": T,
     "unviable": U, "kill_rate": (K+T)/(K+M+T), "sha": "abc1234"}

`kill_rate` denominator excludes `unviable` (mutants that don't compile —
neither caught nor a test gap; they're just compile-time invariants).

Policy: timeouts credited as kills (FUT-17).
    See thoughts/2026-05-23-mutation-survivor-followups.md §FUT-17.
    A cargo-mutants `Timeout` outcome means the mutant ran past the
    per-mutant wall-clock budget (default 30s, well above any normal test).
    Per the per-mutant inspections recorded in that doc — lexer.rs at
    ba41e68+ (13 timeouts), parser.rs at 7726f86 (14 timeouts), and
    prepare_host.rs Cluster P1 (20 timeouts) — every diagnosed timeout
    traces to a genuine non-terminating mutant (cursor index frozen,
    advance-loop suppressed, recursion-depth reset removed, etc.). The
    suite detected the behavioral diff via wall-clock; that's a kill,
    just registered through a different signal than `cargo test` failure.

    Historical rows in the .jsonl predating this change retain the
    old-formula kill_rate values for audit-trail purposes; this script
    can re-emit any historical row under the new formula via
    `--recompute-historical` (rows appended with sha
    `historical-recompute@<current-sha>` so the trend is unambiguous).

Note on `floor_met`: the floors documented in followups.md §"Definition: floor met"
use `(Killed + Equivalent) / (Killed + Equivalent + Missed - PendingInvestigation)`,
which is computed manually from the per-file triage tables (not from this script).
The two metrics are deliberately distinct — this script's `kill_rate` is the
mechanically-derived audit trend; floor_met is the human-judgment-augmented
acceptance criterion. Updating this formula does not change floor_met semantics.

Usage:
    python3 scripts/mutation_baseline.py [--append]
    python3 scripts/mutation_baseline.py --recompute-historical

Without `--append` or `--recompute-historical`, prints rows to stdout.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import defaultdict
from pathlib import Path


def compute_kill_rate(killed: int, missed: int, timeout: int) -> float:
    """FUT-17 formula: credit timeouts as kills.

    kill_rate = (killed + timeout) / (killed + missed + timeout)

    `unviable` is excluded from both numerator and denominator (mutants that
    don't compile are not test gaps).
    """
    viable = killed + missed + timeout
    if viable <= 0:
        return 0.0
    return (killed + timeout) / viable


def current_sha() -> str:
    return subprocess.run(
        ["git", "rev-parse", "--short", "HEAD"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()


def recompute_historical(baseline_path: Path) -> int:
    """Re-emit every prior row under the FUT-17 formula.

    Rows are APPENDED to the baseline tagged with sha
    `historical-recompute@<current-sha>` so the audit trend file shows
    both the original (old-formula) row and the new (corrected) row for
    each (file, original-sha) pair. The original rows are preserved
    verbatim — they ARE the audit trail.

    Skips rows already present as historical-recomputes (idempotent).
    """
    if not baseline_path.exists():
        print(f"error: {baseline_path} not found", file=sys.stderr)
        return 1

    sha = current_sha()
    recompute_tag = f"historical-recompute@{sha}"

    rows: list[dict] = []
    already_recomputed: set[tuple[str, str]] = set()
    with baseline_path.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            rows.append(row)
            if isinstance(row.get("sha"), str) and row["sha"].startswith(
                "historical-recompute@"
            ):
                # Track (file, original_sha) so we don't double-recompute.
                orig = row.get("recomputed_from_sha")
                if orig:
                    already_recomputed.add((row["file"], orig))

    new_rows: list[dict] = []
    for row in rows:
        orig_sha = row.get("sha")
        if not isinstance(orig_sha, str) or orig_sha.startswith(
            "historical-recompute@"
        ):
            continue  # Skip non-string-sha rows and prior recomputes
        if (row["file"], orig_sha) in already_recomputed:
            continue  # Already recomputed at some earlier pass

        k = int(row.get("killed", 0))
        m = int(row.get("missed", 0))
        t = int(row.get("timeout", 0))
        new_rate = compute_kill_rate(k, m, t)

        # Skip rows where the formula change has zero effect (no timeouts).
        # Reduces noise — only the four P0 files (lexer, parser, prepare_host)
        # plus any future timeout-bearing files emit a recompute row.
        if t == 0:
            continue

        new_rows.append(
            {
                "file": row["file"],
                "total": row.get("total", 0),
                "killed": k,
                "missed": m,
                "timeout": t,
                "unviable": row.get("unviable", 0),
                "kill_rate": round(new_rate, 4),
                "sha": recompute_tag,
                "recomputed_from_sha": orig_sha,
                "prior_kill_rate": row.get("kill_rate"),
            }
        )

    with baseline_path.open("a") as f:
        for row in new_rows:
            f.write(json.dumps(row) + "\n")

    print(
        f"appended {len(new_rows)} historical-recompute row(s) to {baseline_path} "
        f"(tag: {recompute_tag})",
        file=sys.stderr,
    )

    # Sanity: no NaN, no negative kill_rate, no kill_rate > 1.
    for row in new_rows:
        rate = row["kill_rate"]
        assert 0.0 <= rate <= 1.0, f"invalid kill_rate {rate} for {row['file']}"
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--append",
        action="store_true",
        help="Append rows to conformance/mutation-baseline.jsonl (default: stdout only)",
    )
    parser.add_argument(
        "--recompute-historical",
        action="store_true",
        help="Re-emit historical rows under the FUT-17 formula "
        "(tagged sha `historical-recompute@<current-sha>`).",
    )
    parser.add_argument(
        "--outcomes",
        default="mutants.out/outcomes.json",
        help="Path to cargo-mutants outcomes.json",
    )
    args = parser.parse_args()

    if args.recompute_historical:
        return recompute_historical(Path("conformance/mutation-baseline.jsonl"))

    outcomes_path = Path(args.outcomes)
    if not outcomes_path.exists():
        print(f"error: {outcomes_path} not found — run `make mutants-*` first", file=sys.stderr)
        return 1

    try:
        with outcomes_path.open() as f:
            data = json.load(f)
    except json.JSONDecodeError as e:
        print(f"error: {outcomes_path} is malformed JSON: {e}", file=sys.stderr)
        return 1

    # cargo-mutants writes outcomes.json incrementally; a CI cancellation
    # or kill mid-run can leave it without the `outcomes` key. Guard against
    # that explicitly rather than KeyError-panicking on the audit script.
    outcomes_list = data.get("outcomes")
    if not isinstance(outcomes_list, list):
        print(
            f"error: {outcomes_path} has no `outcomes` array (partial / "
            f"cancelled run?). Re-run cargo-mutants to completion.",
            file=sys.stderr,
        )
        return 1

    sha = current_sha()

    # Group outcomes by mutated file.
    by_file: dict[str, dict[str, int]] = defaultdict(
        lambda: {"total": 0, "killed": 0, "missed": 0, "timeout": 0, "unviable": 0}
    )

    for outcome in outcomes_list:
        scenario = outcome.get("scenario")
        if not isinstance(scenario, dict) or "Mutant" not in scenario:
            continue  # Skip baseline + non-mutant entries
        mutant = scenario["Mutant"]
        source_file = mutant.get("file")
        if not source_file:
            continue

        summary = outcome.get("summary", "")
        bucket = {
            "MissedMutant": "missed",     # mutant survived test suite (test gap)
            "CaughtMutant": "killed",
            "Timeout": "timeout",          # FUT-17: credited as kill in kill_rate
            "Unviable": "unviable",
        }.get(summary, None)
        if bucket is None:
            print(f"warn: unknown summary {summary!r} for {source_file}", file=sys.stderr)
            continue

        by_file[source_file]["total"] += 1
        by_file[source_file][bucket] += 1

    rows = []
    for file_path, counts in sorted(by_file.items()):
        kill_rate = compute_kill_rate(
            counts["killed"], counts["missed"], counts["timeout"]
        )
        rows.append(
            {
                "file": file_path,
                "total": counts["total"],
                "killed": counts["killed"],
                "missed": counts["missed"],
                "timeout": counts["timeout"],
                "unviable": counts["unviable"],
                "kill_rate": round(kill_rate, 4),
                "sha": sha,
            }
        )

    if args.append:
        baseline_path = Path("conformance/mutation-baseline.jsonl")
        baseline_path.parent.mkdir(parents=True, exist_ok=True)

        # Idempotence: skip any row whose (file, sha) pair is already
        # present in the baseline. Re-running on the same outcomes.json
        # (e.g. CI rerun at the same sha) MUST NOT duplicate rows — the
        # baseline.jsonl is an audit trend artifact, not an event log.
        existing_pairs: set[tuple[str, str]] = set()
        if baseline_path.exists():
            with baseline_path.open() as f:
                for line in f:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        prior = json.loads(line)
                    except json.JSONDecodeError:
                        continue  # tolerate corrupt lines
                    pair = (prior.get("file"), prior.get("sha"))
                    if pair[0] and pair[1]:
                        existing_pairs.add(pair)  # type: ignore[arg-type]

        new_rows = [r for r in rows if (r["file"], r["sha"]) not in existing_pairs]
        skipped = len(rows) - len(new_rows)

        with baseline_path.open("a") as f:
            for row in new_rows:
                f.write(json.dumps(row) + "\n")
        msg = f"appended {len(new_rows)} row(s) to {baseline_path}"
        if skipped:
            msg += f" (skipped {skipped} duplicate (file, sha) row(s) already present)"
        print(msg, file=sys.stderr)
    else:
        for row in rows:
            print(json.dumps(row))

    return 0


if __name__ == "__main__":
    sys.exit(main())
