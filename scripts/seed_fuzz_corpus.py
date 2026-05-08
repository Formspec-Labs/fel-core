#!/usr/bin/env python3
"""Seed fuzz corpus from FEL conformance expressions."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    conformance_path = root.parent / "formspec" / "tests" / "conformance" / "fel-function-semantics.json"
    corpus_dir = root / "fuzz" / "corpus" / "fel_structured"
    corpus_dir.mkdir(parents=True, exist_ok=True)

    cases = json.loads(conformance_path.read_text(encoding="utf-8"))
    seeded = 0

    for case in cases:
        source_bytes = case["expr"].encode()
        file_name = hashlib.sha256(source_bytes).hexdigest()[:16]
        output_path = corpus_dir / file_name

        if not output_path.exists():
            output_path.write_bytes(source_bytes)
            seeded += 1

    print(f"seeded {seeded} new files ({len(cases)} total cases)")


if __name__ == "__main__":
    main()
