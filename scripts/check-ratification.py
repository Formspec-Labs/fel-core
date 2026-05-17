#!/usr/bin/env python3
"""Validate FEL candidate-ratification artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


REQUIRED_SPEC_HEADINGS = [
    "## Status of This Specification",
    "## Normative References",
    "## Conformance Classes",
    "## Security Considerations",
    "## Privacy Considerations",
    "## Internationalization Considerations",
    "## Extensibility and Versioning",
]

REQUIRED_GRAMMAR_HEADINGS = [
    "## 2. Notation",
    "## 4. Expression Grammar",
    "## 7. Conformance",
]


def fail(message: str) -> None:
    print(f"ratification check failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def read_text(path: str) -> str:
    full_path = ROOT / path
    if not full_path.exists():
        fail(f"missing {path}")
    return full_path.read_text(encoding="utf-8")


def sha256_bytes(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def validate_required_headings() -> None:
    spec = read_text("docs/SPEC.md")
    for heading in REQUIRED_SPEC_HEADINGS:
        if heading not in spec:
            fail(f"docs/SPEC.md missing heading {heading!r}")

    grammar = read_text("specs/fel/fel-grammar.md")
    for heading in REQUIRED_GRAMMAR_HEADINGS:
        if heading not in grammar:
            fail(f"specs/fel/fel-grammar.md missing heading {heading!r}")

    if "```peg" not in grammar:
        fail("normative grammar does not contain PEG blocks")


def validate_conformance_corpus() -> dict[str, object]:
    corpus_path = ROOT / "conformance" / "fel-conformance.jsonl"
    if not corpus_path.exists():
        fail("missing conformance/fel-conformance.jsonl")

    raw = corpus_path.read_bytes()
    lines = raw.decode("utf-8").splitlines()
    if len(lines) < 200:
        fail(f"conformance corpus too small: {len(lines)} fixtures")

    required = {"expression", "environment", "expectedValue", "expectedDiagnosticKinds"}
    expressions: set[str] = set()
    diagnostic_kind_seen = False
    env_case_seen = False

    for index, line in enumerate(lines, start=1):
        try:
            case = json.loads(line)
        except json.JSONDecodeError as exc:
            fail(f"invalid JSONL at conformance line {index}: {exc}")

        missing = required - set(case)
        if missing:
            fail(f"conformance line {index} missing keys: {sorted(missing)}")

        if not isinstance(case["expression"], str) or not case["expression"]:
            fail(f"conformance line {index} has invalid expression")
        if not isinstance(case["environment"], dict):
            fail(f"conformance line {index} has non-object environment")
        if not isinstance(case["expectedDiagnosticKinds"], list):
            fail(f"conformance line {index} has non-array expectedDiagnosticKinds")

        expressions.add(case["expression"])
        diagnostic_kind_seen = diagnostic_kind_seen or bool(case["expectedDiagnosticKinds"])
        env_case_seen = env_case_seen or bool(case["environment"])

    if len(expressions) < 175:
        fail(f"conformance corpus has too few distinct expressions: {len(expressions)}")
    if not diagnostic_kind_seen:
        fail("conformance corpus has no diagnostic-kind cases")
    if not env_case_seen:
        fail("conformance corpus has no environment-bound cases")

    return {
        "lineCount": len(lines),
        "sha256": sha256_bytes(raw),
        "distinctExpressions": len(expressions),
    }


def validate_manifest(corpus_summary: dict[str, object]) -> None:
    manifest_path = ROOT / "conformance" / "manifest.json"
    if not manifest_path.exists():
        fail("missing conformance/manifest.json")

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    corpus = manifest.get("corpus", {})
    for key in ("lineCount", "sha256"):
        if corpus.get(key) != corpus_summary[key]:
            fail(
                "conformance manifest corpus "
                f"{key}={corpus.get(key)!r} does not match actual {corpus_summary[key]!r}"
            )

    required_docs = manifest.get("normativeDocuments", [])
    for path in required_docs:
        if not (ROOT / path).exists():
            fail(f"manifest references missing normative document: {path}")

    gates = manifest.get("ratificationGates", [])
    if "make ratify" not in gates:
        fail("manifest must list make ratify as the local ratification gate")


def validate_todo_surface() -> None:
    todo = read_text("TODO.md")
    if "No open ratification blockers remain." not in todo:
        fail("TODO.md does not state the current ratification-blocker posture")
    if "#[deprecated]" in todo:
        fail("TODO.md still carries stale deprecated-wrapper debt")


def verify_generated_corpus() -> None:
    proc = subprocess.run(
        [
            "cargo",
            "run",
            "--features",
            "proptest-strategies",
            "--bin",
            "emit-conformance-fixtures",
            "--",
            "200",
        ],
        cwd=ROOT,
        check=False,
        text=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if proc.returncode != 0:
        sys.stderr.buffer.write(proc.stderr)
        fail("conformance generator failed")

    generated = proc.stdout
    committed = (ROOT / "conformance" / "fel-conformance.jsonl").read_bytes()
    if generated != committed:
        fail("generated conformance corpus differs from conformance/fel-conformance.jsonl")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--verify-generated",
        action="store_true",
        help="also regenerate the public conformance corpus and compare bytes",
    )
    args = parser.parse_args()

    validate_required_headings()
    corpus_summary = validate_conformance_corpus()
    validate_manifest(corpus_summary)
    validate_todo_surface()

    if args.verify_generated:
        verify_generated_corpus()

    print(
        "ratification artifacts ok: "
        f"{corpus_summary['lineCount']} fixtures, "
        f"{corpus_summary['distinctExpressions']} distinct expressions"
    )


if __name__ == "__main__":
    main()
