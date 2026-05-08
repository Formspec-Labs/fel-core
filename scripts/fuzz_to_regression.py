#!/usr/bin/env python3
"""Converts libfuzzer crash artifacts into permanent regression tests.

Usage: python3 scripts/fuzz_to_regression.py [--corpus fuzz/corpus/fel_pipeline/]

Reads minimized crash inputs from cargo-fuzz corpus, deduplicates by SHA-256,
and appends named test functions to `tests/evaluator_edge_cases.rs`.
"""

import argparse
import hashlib
import os
import sys


def sha256_prefix(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()[:12]


def artifact_as_bytes(path: str) -> bytes:
    with open(path, "rb") as fh:
        return fh.read()


def emit_test(src: str, sha_prefix: str, output_path: str) -> None:
    escaped = src.replace("\\", "\\\\")
    escaped = escaped.replace("'", "\\'")
    escaped = escaped.replace("\n", "\\n")
    escaped = escaped.replace("\r", "\\r")
    escaped = escaped.replace("\t", "\\t")
    escaped = escaped.replace('"', '\\"')
    test_name = f"fuzz_regression_{sha_prefix}"
    test_body = f'''
#[test]
fn {test_name}() {{
    let src = "{escaped}";
    let expr = parse(src).unwrap();
    let env = MapEnvironment::new();
    let result = evaluate(&expr, &env);
    // Fuzz-discovered edge case: expression must not panic.
    let _ = format!("{{}}", result.value);
}}
'''
    with open(output_path, "a") as fh:
        fh.write(test_body)
    print(f"  appended test {test_name}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Convert fuzz artifacts to regression tests")
    parser.add_argument(
        "--corpus",
        default="fuzz/corpus/fel_pipeline/",
        help="Path to libfuzzer corpus directory",
    )
    parser.add_argument(
        "--output",
        default="tests/evaluator_edge_cases.rs",
        help="Path to append regression tests to",
    )
    args = parser.parse_args()

    if not os.path.isdir(args.corpus):
        print(f"corpus directory not found: {args.corpus}", file=sys.stderr)
        sys.exit(1)

    artifacts = [
        os.path.join(args.corpus, f)
        for f in os.listdir(args.corpus)
        if os.path.isfile(os.path.join(args.corpus, f))
    ]

    seen = set()
    count = 0
    for artifact in sorted(artifacts):
        raw = artifact_as_bytes(artifact)
        sha = sha256_prefix(raw)
        if sha in seen:
            continue
        seen.add(sha)
        try:
            src = raw.decode("utf-8", errors="replace")
            if len(src) > 512:
                continue
            if any(ord(c) < 0x20 and c not in "\n\r\t" for c in src):
                continue
            emit_test(src, sha, args.output)
            count += 1
        except Exception:
            continue

    print(f"emitted {count} regression tests")


if __name__ == "__main__":
    main()
