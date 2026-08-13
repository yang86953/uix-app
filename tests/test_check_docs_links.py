# -*- coding: utf-8 -*-
"""仓库文档链接测试。"""
from __future__ import annotations

import importlib.util
import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
# 文档检查实现只作为测试辅助模块存在。
SCRIPT = ROOT / "tests" / "support" / "check_docs_links.py"
SPEC = importlib.util.spec_from_file_location("check_docs_links", SCRIPT)
if SPEC is None or SPEC.loader is None:  # pragma: no cover - import guard
    raise RuntimeError(f"cannot load {SCRIPT}")
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class CheckDocsLinksTests(unittest.TestCase):
    def test_all_root_markdown_files_are_scanned(self) -> None:
        root_markdown = set(CHECK.ROOT.glob("*.md"))

        self.assertTrue(root_markdown)
        self.assertTrue(root_markdown.issubset(set(CHECK.SCAN)))

    def test_file_uri_is_an_external_document_boundary(self) -> None:
        raw = (
            "<file:///C:/data/note/程序开发/"
            "System%20Module%20Component%20设计模式.md#强制不变量>"
        )

        self.assertEqual(CHECK.target_scheme(raw), "file")
        self.assertTrue(CHECK.is_external_target(raw))
        self.assertTrue(CHECK.is_external_document_target(raw))
        self.assertIsNone(
            CHECK.validate_local_target(CHECK.ROOT / "docs/产品.md", raw, {})
        )

    def test_angle_bracket_target_preserves_spaces(self) -> None:
        path, fragment = CHECK.split_target("<../folder/with space.md#An Anchor>")

        self.assertEqual(path, "../folder/with space.md")
        self.assertEqual(fragment, "An Anchor")

    def test_relative_target_still_uses_local_validation(self) -> None:
        md = CHECK.ROOT / "docs/产品.md"
        raw = "<产品/能力.md#能力与边界>"

        self.assertFalse(CHECK.is_external_target(raw))
        self.assertIsNone(CHECK.validate_local_target(md, raw, {}))

    def test_all_three_part_indexes_are_required(self) -> None:
        required_names = {path.name for path in CHECK.REQUIRED_FILES}

        self.assertTrue({"产品.md", "使用.md", "架构.md"}.issubset(required_names))

    def test_legacy_vault_project_path_is_detected(self) -> None:
        text = r"C:\data\note\我的项目\软件\UIX App\使用.md"

        self.assertIsNotNone(CHECK.LEGACY_PROJECT_PATH_RE.search(text))

    def test_obsidian_wiki_link_is_detected(self) -> None:
        self.assertIsNotNone(CHECK.WIKI_LINK_RE.search("[[使用/快速开始|快速开始]]"))

    def test_usage_example_id_is_extracted_from_rust_fence(self) -> None:
        text = "```rust uix-compile=hello-world\nfn main() {}\n```"

        self.assertEqual(CHECK.USAGE_EXAMPLE_ID_RE.findall(text), ["hello-world"])

    def test_current_repository_passes_the_gate(self) -> None:
        output = io.StringIO()
        with redirect_stdout(output):
            result = CHECK.main()

        self.assertEqual(result, 0, output.getvalue())
        self.assertIn(
            "external document link(s) outside repository boundary",
            output.getvalue(),
        )


if __name__ == "__main__":
    unittest.main()
