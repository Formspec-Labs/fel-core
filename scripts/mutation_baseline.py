#!/usr/bin/env python3
"""Convert cargo-mutants outcomes.json → conformance/mutation-baseline.jsonl row.

Phase 2 mutation gate (see thoughts/2026-05-23-test-suite-triage.md).

Reads `mutants.out/outcomes.json` after a `cargo mutants` run and appends one
line per mutated file to `conformance/mutation-baseline.jsonl`:

    {"file": "...", "total": N, "killed": K, "missed": M, "timeout": T,
     "unviable": U, "kill_rate": K/(K+M+T), "sha": "abc1234"}

`kill_rate` denominator excludes `unviable` (mutants that don't compile —
neither caught nor a test gap; they're just compile-time invariants).

Usage:
    python3 scripts/mutation_baseline.py [--append]

Without `--append`, prints the rows to stdout for inspection.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import defaultdict
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--append",
        action="store_true",
        help="Append rows to conformance/mutation-baseline.jsonl (default: stdout only)",
    )
    parser.add_argument(
        "--outcomes",
        default="mutants.out/outcomes.json",
        help="Path to cargo-mutants outcomes.json",
    )
    args = parser.parse_args()

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

    sha = subprocess.run(
        ["git", "rev-parse", "--short", "HEAD"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()

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
            "Timeout": "timeout",
            "Unviable": "unviable",
        }.get(summary, None)
        if bucket is None:
            print(f"warn: unknown summary {summary!r} for {source_file}", file=sys.stderr)
            continue

        by_file[source_file]["total"] += 1
        by_file[source_file][bucket] += 1

    rows = []
    for file_path, counts in sorted(by_file.items()):
        viable = counts["killed"] + counts["missed"] + counts["timeout"]
        kill_rate = counts["killed"] / viable if viable > 0 else 0.0
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
