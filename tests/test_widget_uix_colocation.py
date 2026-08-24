# -*- coding: utf-8 -*-
"""守护内置 widget 的 UIX 与 Rust 同目录契约。"""

from __future__ import annotations

import unittest
from pathlib import Path


# 定位仓库与内置组件源码根目录。
ROOT = Path(__file__).resolve().parents[1]
WIDGETS_ROOT = ROOT / "src" / "ui" / "widgets"


class WidgetUixColocationTests(unittest.TestCase):
    # 已迁移的 UIX 文件必须位于拥有它的 Rust 组件目录。
    def test_uix_sources_are_colocated_with_rust_widget_modules(self) -> None:
        violations: list[str] = []
        for uix_path in sorted(WIDGETS_ROOT.rglob("*.uix")):
            # 独立组件目录以 mod.rs 作为 Rust 模块入口。
            if not (uix_path.parent / "mod.rs").is_file():
                violations.append(uix_path.relative_to(ROOT).as_posix())
        self.assertEqual(violations, [])

    # 旧共享目录会重新拆散组件所有权，因此禁止恢复。
    def test_shared_widget_uix_directory_is_absent(self) -> None:
        self.assertFalse((WIDGETS_ROOT / "uix").exists())


if __name__ == "__main__":
    unittest.main()
