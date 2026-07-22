# -*- coding: utf-8 -*-
"""Validate the machine-readable UIX 0.0.1 function closure ledger."""
from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEDGER_REL = Path("docs") / "\u8fdb\u5ea6" / "0.0.1\u529f\u80fd\u8d26\u672c.md"
LEDGER_START = "<!-- 0.0.1-FUNCTION-LEDGER:START -->"
LEDGER_END = "<!-- 0.0.1-FUNCTION-LEDGER:END -->"

FIELDS = (
    "id",
    "batch",
    "component",
    "public_doc",
    "public_entry",
    "consume_source",
    "test_ref",
    "interaction_semantics_boundary",
    "targeted_status",
    "candidate_commit",
    "full_gate_status",
    "true_window_status",
    "closure_status",
)

EXPECTED_RANGES = {
    4: range(4, 25),
    5: range(1, 24),
    6: range(1, 31),
    7: range(1, 25),
    8: range(1, 13),
}
EXPECTED_IDS = tuple(
    f"{batch}.{item}"
    for batch, items in EXPECTED_RANGES.items()
    for item in items
)
EXPECTED_BATCH_COUNTS = {batch: len(items) for batch, items in EXPECTED_RANGES.items()}

TEST_NAME_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
SHA_RE = re.compile(r"^[0-9a-fA-F]{40}$")
FORBIDDEN_STATUS_RE = re.compile(r"\b(?:pass(?:ed)?|complete(?:d)?|completed)\b", re.I)


class LedgerFormatError(ValueError):
    """Raised when the ledger block cannot be parsed."""


def parse_ledger_text(text: str) -> list[dict[str, str]]:
    if text.count(LEDGER_START) != 1 or text.count(LEDGER_END) != 1:
        raise LedgerFormatError("ledger must contain exactly one start and end marker")
    block = text.split(LEDGER_START, 1)[1].split(LEDGER_END, 1)[0]
    lines = [line.strip() for line in block.splitlines() if line.strip()]
    if not lines:
        raise LedgerFormatError("ledger block is empty")
    header = tuple(part.strip() for part in lines[0].split("|"))
    if header != FIELDS:
        raise LedgerFormatError(
            f"ledger header mismatch: expected {'|'.join(FIELDS)}, got {'|'.join(header)}"
        )

    records: list[dict[str, str]] = []
    for line_number, line in enumerate(lines[1:], start=2):
        values = tuple(part.strip() for part in line.split("|"))
        if len(values) != len(FIELDS):
            raise LedgerFormatError(
                f"ledger record {line_number} has {len(values)} fields; expected {len(FIELDS)}"
            )
        records.append(dict(zip(FIELDS, values, strict=True)))
    return records


def load_ledger(root: Path = ROOT) -> list[dict[str, str]]:
    ledger = root / LEDGER_REL
    if not ledger.is_file():
        raise LedgerFormatError(f"missing ledger: {LEDGER_REL.as_posix()}")
    return parse_ledger_text(ledger.read_text(encoding="utf-8"))


def _resolve_reference(root: Path, value: str) -> tuple[Path | None, str | None, str | None]:
    raw_path, separator, needle = value.partition("#")
    if not separator or not raw_path.strip() or not needle.strip():
        return None, None, "reference must use relative/path#stable-marker"
    relative = Path(raw_path.strip())
    if relative.is_absolute():
        return None, None, "reference path must be repository-relative"
    resolved_root = root.resolve()
    resolved = (resolved_root / relative).resolve()
    if not resolved.is_relative_to(resolved_root):
        return None, None, "reference escapes repository root"
    if not resolved.is_file():
        return None, None, f"referenced file does not exist: {relative.as_posix()}"
    return resolved, needle.strip(), None


def _validate_marker_reference(root: Path, value: str, label: str) -> str | None:
    path, needle, error = _resolve_reference(root, value)
    if error:
        return f"{label}: {error}"
    assert path is not None and needle is not None
    if needle not in path.read_text(encoding="utf-8"):
        return f"{label}: marker {needle!r} not found in {path.relative_to(root).as_posix()}"
    return None


def _validate_test_reference(root: Path, value: str) -> str | None:
    path, test_name, error = _resolve_reference(root, value)
    if error:
        return f"test_ref: {error}"
    assert path is not None and test_name is not None
    if not TEST_NAME_RE.fullmatch(test_name):
        return f"test_ref: invalid exact test name {test_name!r}"
    source = path.read_text(encoding="utf-8")
    pattern = re.compile(rf"^\s*fn\s+{re.escape(test_name)}\s*\(", re.M)
    if not pattern.search(source):
        return (
            f"test_ref: exact test function {test_name!r} not found in "
            f"{path.relative_to(root).as_posix()}"
        )
    return None


def _validate_evidence_status(root: Path, value: str, label: str) -> list[str]:
    if value == "pending" or value.startswith("blocked:"):
        return []
    for prefix in ("verified:", "equivalent:"):
        if value.startswith(prefix):
            reference = value[len(prefix) :]
            error = _validate_marker_reference(root, reference, label)
            return [error] if error else []
    return [f"{label}: expected pending, blocked:<reason>, verified:<reference>, or equivalent:<reference>"]


def validate_records(records: list[dict[str, str]], root: Path = ROOT) -> list[str]:
    errors: list[str] = []
    ids = [record.get("id", "") for record in records]
    counts = Counter(ids)
    duplicates = sorted(item_id for item_id, count in counts.items() if count > 1)
    if duplicates:
        errors.append(f"duplicate item ids: {', '.join(duplicates)}")

    expected = set(EXPECTED_IDS)
    actual = set(ids)
    missing = sorted(expected - actual, key=lambda value: tuple(map(int, value.split("."))))
    unexpected = sorted(actual - expected)
    if missing:
        errors.append(f"missing item ids: {', '.join(missing)}")
    if unexpected:
        errors.append(f"unexpected item ids: {', '.join(unexpected)}")
    if len(records) != len(EXPECTED_IDS):
        errors.append(f"ledger has {len(records)} records; expected {len(EXPECTED_IDS)}")

    batch_counts = Counter(record.get("batch", "") for record in records)
    for batch, expected_count in EXPECTED_BATCH_COUNTS.items():
        actual_count = batch_counts[str(batch)]
        if actual_count != expected_count:
            errors.append(
                f"batch {batch} has {actual_count} records; expected {expected_count}"
            )

    for index, record in enumerate(records, start=1):
        item_id = record.get("id", f"record-{index}")
        for field in FIELDS:
            if not record.get(field, "").strip():
                errors.append(f"{item_id}: missing field {field}")
        if "." in item_id and record.get("batch") != item_id.split(".", 1)[0]:
            errors.append(f"{item_id}: batch field does not match id prefix")

        public_doc = record.get("public_doc", "")
        if not public_doc.partition("#")[0].endswith(".md"):
            errors.append(f"{item_id}: public_doc must reference a Markdown file")
        for field in ("public_doc", "public_entry", "consume_source"):
            if error := _validate_marker_reference(root, record.get(field, ""), field):
                errors.append(f"{item_id}: {error}")
        if error := _validate_test_reference(root, record.get("test_ref", "")):
            errors.append(f"{item_id}: {error}")

        state = record.get("interaction_semantics_boundary", "")
        for section in ("interaction=", "semantic=", "boundary="):
            if not re.search(rf"(?:^|;){re.escape(section)}[^;]+", state):
                errors.append(f"{item_id}: interaction/semantic/boundary field lacks {section}")

        if record.get("targeted_status") != "targeted-verified":
            errors.append(f"{item_id}: targeted_status must be targeted-verified")

        candidate = record.get("candidate_commit", "")
        if candidate != "pending" and not SHA_RE.fullmatch(candidate):
            errors.append(f"{item_id}: candidate_commit must be pending or a full 40-hex commit")

        for field in ("targeted_status", "full_gate_status", "true_window_status", "closure_status"):
            value = record.get(field, "")
            if FORBIDDEN_STATUS_RE.search(value):
                errors.append(f"{item_id}: forbidden completion word in {field}: {value!r}")

        errors.extend(
            f"{item_id}: {error}"
            for error in _validate_evidence_status(
                root, record.get("full_gate_status", ""), "full_gate_status"
            )
        )
        errors.extend(
            f"{item_id}: {error}"
            for error in _validate_evidence_status(
                root, record.get("true_window_status", ""), "true_window_status"
            )
        )

        closure = record.get("closure_status", "")
        if closure not in {"open", "closed"}:
            errors.append(f"{item_id}: closure_status must be open or closed")
        if closure == "closed":
            full_gate = record.get("full_gate_status", "")
            true_window = record.get("true_window_status", "")
            if not SHA_RE.fullmatch(candidate):
                errors.append(f"{item_id}: closed item lacks a frozen candidate commit")
            if not full_gate.startswith("verified:"):
                errors.append(f"{item_id}: closed item lacks verified full-gate evidence")
            if not true_window.startswith(("verified:", "equivalent:")):
                errors.append(f"{item_id}: closed item lacks true-window/equivalent evidence")

        if candidate == "pending" and (
            record.get("full_gate_status", "").startswith("verified:")
            or record.get("true_window_status", "").startswith(("verified:", "equivalent:"))
        ):
            errors.append(f"{item_id}: candidate-bound evidence cannot precede candidate freeze")

    return errors


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    root = Path(args[0]).resolve() if args else ROOT
    try:
        records = load_ledger(root)
    except (OSError, LedgerFormatError, UnicodeError) as error:
        print(f"FAIL: {error}")
        return 1
    errors = validate_records(records, root)
    batch_counts = Counter(record["batch"] for record in records)
    print(
        "ledger records: "
        f"{len(records)}; batches: "
        + ", ".join(f"{batch}={batch_counts[str(batch)]}" for batch in EXPECTED_RANGES)
    )
    if errors:
        print(f"FAIL: {len(errors)} issue(s)")
        for error in errors:
            print(f"  {error}")
        return 1
    print("OK: 0.0.1 function ledger is structurally valid; closure remains evidence-gated")
    return 0


if __name__ == "__main__":
    sys.exit(main())
