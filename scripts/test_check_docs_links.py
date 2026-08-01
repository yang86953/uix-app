# -*- coding: utf-8 -*-
"""Contract tests for the repository documentation link gate."""
from __future__ import annotations

import importlib.util
import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path


SCRIPT = Path(__file__).with_name("check_docs_links.py")
SPEC = importlib.util.spec_from_file_location("check_docs_links", SCRIPT)
if SPEC is None or SPEC.loader is None:  # pragma: no cover - import guard
    raise RuntimeError(f"cannot load {SCRIPT}")
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class CheckDocsLinksTests(unittest.TestCase):
    def test_file_uri_is_an_external_document_boundary(self) -> None:
        raw = (
            "<file:///C:/data/pi/note/我的项目/软件/UIX%20App/架构/"
            "platform/diagnostics.md#平台层全覆盖>"
        )

        self.assertEqual(CHECK.target_scheme(raw), "file")
        self.assertTrue(CHECK.is_external_target(raw))
        self.assertTrue(CHECK.is_external_document_target(raw))
        self.assertIsNone(
            CHECK.validate_local_target(CHECK.ROOT / "docs/进度.md", raw, {})
        )

    def test_angle_bracket_target_preserves_spaces(self) -> None:
        path, fragment = CHECK.split_target("<../folder/with space.md#An Anchor>")

        self.assertEqual(path, "../folder/with space.md")
        self.assertEqual(fragment, "An Anchor")

    def test_relative_target_still_uses_local_validation(self) -> None:
        md = CHECK.ROOT / "docs/进度.md"
        raw = "<进度/理想使用演进.md#进度总览>"

        self.assertFalse(CHECK.is_external_target(raw))
        self.assertIsNone(CHECK.validate_local_target(md, raw, {}))

    def test_current_repository_passes_the_gate(self) -> None:
        output = io.StringIO()
        with redirect_stdout(output):
            result = CHECK.main()

        self.assertEqual(result, 0, output.getvalue())
        self.assertIn("external document link(s) outside repository boundary", output.getvalue())


if __name__ == "__main__":
    unittest.main()
