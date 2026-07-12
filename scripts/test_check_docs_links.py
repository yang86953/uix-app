import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.check_docs_links import (
    ROOT,
    duplicate_explicit_anchors,
    validate_local_target,
)


class CheckDocsLinksTests(unittest.TestCase):
    def test_missing_anchor_is_fatal(self) -> None:
        error = validate_local_target(
            ROOT / "AGENTS.md", "docs/产品.md#不存在", {}
        )
        self.assertIsNotNone(error)
        self.assertIn("missing anchor", error or "")

    def test_existing_anchor_passes(self) -> None:
        error = validate_local_target(ROOT / "AGENTS.md", "docs/产品.md#产品", {})
        self.assertIsNone(error)

    def test_duplicate_explicit_anchor_is_reported_case_insensitively(self) -> None:
        with TemporaryDirectory() as directory:
            path = Path(directory) / "duplicate.md"
            path.write_text('<a id="REQ-001"></a>\n<a id="req-001"></a>\n', encoding="utf-8")
            self.assertEqual(duplicate_explicit_anchors(path), ["req-001"])


if __name__ == "__main__":
    unittest.main()
