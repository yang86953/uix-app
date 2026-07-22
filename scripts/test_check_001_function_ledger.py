import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.check_001_function_ledger import EXPECTED_IDS, FIELDS, validate_records


class Check001FunctionLedgerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = TemporaryDirectory()
        self.root = Path(self.temporary.name)
        (self.root / "doc.md").write_text("uix-compile=proof\n", encoding="utf-8")
        (self.root / "public.rs").write_text("pub struct Proof;\n", encoding="utf-8")
        (self.root / "consume.rs").write_text("fn consume_proof() {}\n", encoding="utf-8")
        (self.root / "tests.rs").write_text("fn proof_case() {}\n", encoding="utf-8")
        self.records = [self.make_record(item_id) for item_id in EXPECTED_IDS]

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def make_record(item_id: str) -> dict[str, str]:
        record = {
            "id": item_id,
            "batch": item_id.split(".", 1)[0],
            "component": "Proof",
            "public_doc": "doc.md#uix-compile=proof",
            "public_entry": "public.rs#Proof",
            "consume_source": "consume.rs#consume_proof",
            "test_ref": "tests.rs#proof_case",
            "interaction_semantics_boundary": (
                "interaction=targeted;semantic=targeted;boundary=targeted"
            ),
            "targeted_status": "targeted-verified",
            "candidate_commit": "pending",
            "full_gate_status": "pending",
            "true_window_status": "pending",
            "closure_status": "open",
        }
        assert tuple(record) == FIELDS
        return record

    def test_missing_item_is_fatal(self) -> None:
        errors = validate_records(self.records[:-1], self.root)
        self.assertTrue(any("missing item ids" in error for error in errors), errors)

    def test_duplicate_item_is_fatal(self) -> None:
        records = [*self.records, dict(self.records[0])]
        errors = validate_records(records, self.root)
        self.assertTrue(any("duplicate item ids" in error for error in errors), errors)

    def test_nonexistent_reference_is_fatal(self) -> None:
        self.records[0]["consume_source"] = "missing.rs#consume_proof"
        errors = validate_records(self.records, self.root)
        self.assertTrue(
            any("referenced file does not exist" in error for error in errors), errors
        )

    def test_false_pass_without_candidate_or_window_evidence_is_fatal(self) -> None:
        self.records[0]["full_gate_status"] = "pass"
        self.records[0]["closure_status"] = "completed"
        errors = validate_records(self.records, self.root)
        self.assertTrue(
            any("forbidden completion word" in error for error in errors), errors
        )


if __name__ == "__main__":
    unittest.main()
