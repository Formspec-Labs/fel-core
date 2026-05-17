#!/usr/bin/env python3
"""One-shot: extract fuzz_regression_* bodies from evaluator_edge_cases.rs."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


def decode_rust_string_literal(raw: str) -> str:
    out: list[str] = []
    i = 0
    while i < len(raw):
        ch = raw[i]
        if ch != "\\":
            out.append(ch)
            i += 1
            continue
        i += 1
        if i >= len(raw):
            break
        esc = raw[i]
        i += 1
        mapping = {"n": "\n", "r": "\r", "t": "\t", "\\": "\\", '"': '"', "'": "'"}
        if esc in mapping:
            out.append(mapping[esc])
        elif esc == "u" and raw[i : i + 4].isascii() and len(raw[i : i + 4]) == 4:
            out.append(chr(int(raw[i : i + 4], 16)))
            i += 4
        else:
            out.append(esc)
    return "".join(out)


def collect_string_literals(src_block: str) -> str:
    parts: list[str] = []
    for m in re.finditer(r'"((?:\\.|[^"\\])*)"', src_block, re.DOTALL):
        parts.append(decode_rust_string_literal(m.group(1)))
    return "".join(parts)


def extract_entries(text: str) -> list[tuple[str, str]]:
    entries: list[tuple[str, str]] = []
    for m in re.finditer(r"fn fuzz_regression_([0-9a-f]+)\(\) \{", text):
        fid = m.group(1)
        start = m.end()
        depth = 1
        i = start
        while i < len(text) and depth:
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
            i += 1
        body = text[start : i - 1]
        src_match = re.search(r"let\s+src\s*=", body)
        if not src_match:
            print(f"warning: no src in fuzz_regression_{fid}", file=sys.stderr)
            continue
        after_src = body[src_match.end() :]
        end = re.search(r";\s*\n\s*let\s+", after_src)
        src_stmt = after_src[: end.start()] if end else after_src
        expression = collect_string_literals(src_stmt)
        entries.append((fid, expression))
    return entries


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--input",
        default="tests/evaluator_edge_cases.rs",
        help="Rust test file containing fuzz_regression_* tests",
    )
    parser.add_argument(
        "--output",
        default="tests/corpus/fuzz_regression.seed.jsonl",
        help="JSONL with id + expression (no oracles)",
    )
    args = parser.parse_args()
    text = Path(args.input).read_text(encoding="utf-8")
    entries = extract_entries(text)
    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8") as fh:
        for fid, expression in entries:
            fh.write(json.dumps({"id": fid, "expression": expression}, ensure_ascii=False))
            fh.write("\n")
    print(f"wrote {len(entries)} seed rows to {out}")


if __name__ == "__main__":
    main()
