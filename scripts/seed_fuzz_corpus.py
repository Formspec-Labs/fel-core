#!/usr/bin/env python3
"""Seed the libFuzzer corpus from conformance expressions.

Usage:
  python3 scripts/seed_fuzz_corpus.py

Set `FORMSPEC_ROOT` when the formspec sibling is not at `../formspec` relative
to this crate (stack layout only).
"""

from __future__ import annotations

import json
import os
import shutil
from pathlib import Path


def main() -> None:
    repo_root = Path(__file__).resolve().parents[1]
    formspec_root = Path(
        os.environ.get("FORMSPEC_ROOT", repo_root.parent / "formspec"),
    ).resolve()
    corpus_dir = repo_root / "fuzz" / "corpus" / "fel_pipeline"
    corpus_dir.mkdir(parents=True, exist_ok=True)

    jsonl = repo_root / "conformance" / "fel-conformance.jsonl"
    if not jsonl.is_file():
        print(f"missing conformance corpus: {jsonl}", flush=True)
        return

    count = 0
    with jsonl.open(encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            row = json.loads(line)
            expr = row.get("expression")
            if not isinstance(expr, str) or not expr:
                continue
            dest = corpus_dir / f"conf_{count:05d}.fel"
            dest.write_text(expr, encoding="utf-8")
            count += 1

    print(f"seeded {count} expressions into {corpus_dir}")
    if not formspec_root.is_dir():
        print(
            f"note: FORMSPEC_ROOT {formspec_root} is not a directory (seed used fel-core conformance only)",
            flush=True,
        )


if __name__ == "__main__":
    main()
