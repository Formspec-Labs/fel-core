#!/usr/bin/env python3
"""Converts libfuzzer crash artifacts into fuzz regression corpus rows.

Usage:
  python3 scripts/fuzz_to_regression.py [--corpus fuzz/corpus/fel_pipeline/]

Reads minimized crash inputs from cargo-fuzz corpus, deduplicates by SHA-256,
appends JSONL rows to `tests/corpus/fuzz_regression.jsonl`, and enriches each
new row with `mustParse` / `displayOracle` via `emit-fuzz-regression-corpus`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path


def sha256_prefix(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()[:12]


def artifact_as_bytes(path: str) -> bytes:
    with open(path, "rb") as fh:
        return fh.read()


def load_existing_ids(corpus_path: Path) -> set[str]:
    if not corpus_path.is_file():
        return set()
    ids: set[str] = set()
    with corpus_path.open(encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            row = json.loads(line)
            ids.add(row["id"])
    return ids


def enrich_row(row: dict[str, object], repo_root: Path) -> dict[str, object]:
    proc = subprocess.run(
        ["cargo", "run", "-q", "--bin", "emit-fuzz-regression-corpus"],
        input=(json.dumps(row, ensure_ascii=False) + "\n").encode("utf-8"),
        cwd=repo_root,
        capture_output=True,
        check=False,
    )
    if proc.returncode != 0:
        stderr = proc.stderr.decode("utf-8", errors="replace")
        raise RuntimeError(f"emit-fuzz-regression-corpus failed: {stderr}")
    return json.loads(proc.stdout.decode("utf-8").splitlines()[0])


def append_row(corpus_path: Path, row: dict[str, object]) -> None:
    corpus_path.parent.mkdir(parents=True, exist_ok=True)
    with corpus_path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(row, ensure_ascii=False))
        fh.write("\n")


def main() -> None:
    parser = argparse.ArgumentParser(description="Convert fuzz artifacts to regression corpus rows")
    parser.add_argument(
        "--corpus",
        default="fuzz/corpus/fel_pipeline/",
        help="Path to libfuzzer corpus directory",
    )
    parser.add_argument(
        "--output",
        default="tests/corpus/fuzz_regression.jsonl",
        help="Path to fuzz regression JSONL corpus",
    )
    args = parser.parse_args()
    repo_root = Path(__file__).resolve().parents[1]
    corpus_path = repo_root / args.output

    if not os.path.isdir(args.corpus):
        print(f"corpus directory not found: {args.corpus}", file=sys.stderr)
        sys.exit(1)

    artifacts = [
        os.path.join(args.corpus, f)
        for f in os.listdir(args.corpus)
        if os.path.isfile(os.path.join(args.corpus, f))
    ]

    seen_sha: set[str] = set()
    existing_ids = load_existing_ids(corpus_path)
    count = 0
    for artifact in sorted(artifacts):
        raw = artifact_as_bytes(artifact)
        sha = sha256_prefix(raw)
        if sha in seen_sha or sha in existing_ids:
            continue
        seen_sha.add(sha)
        try:
            src = raw.decode("utf-8", errors="replace")
            if len(src) > 512:
                continue
            if any(ord(c) < 0x20 and c not in "\n\r\t" for c in src):
                continue
            seed = {"id": sha, "expression": src}
            row = enrich_row(seed, repo_root)
            append_row(corpus_path, row)
            existing_ids.add(sha)
            count += 1
            print(f"  appended corpus row fuzz_regression_{sha}")
        except Exception as exc:
            print(f"  skipped {artifact}: {exc}", file=sys.stderr)
            continue

    print(f"emitted {count} corpus rows to {corpus_path}")


if __name__ == "__main__":
    main()
