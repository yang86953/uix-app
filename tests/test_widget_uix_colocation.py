# -*- coding: utf-8 -*-
"""守护内置 widget 的 UIX 与 Rust 同目录契约。"""

from __future__ import annotations

import re
import unittest
from pathlib import Path


# 定位仓库与内置组件源码根目录。
ROOT = Path(__file__).resolve().parents[1]
WIDGETS_ROOT = ROOT / "src" / "ui" / "widgets"

# 已完成同目录和公开根迁移、但尚未把静态视觉配置移入 UIX 的存量债务。
# 该集合只能缩小；新迁移不得加入，修复一项时必须同步删除对应路径。
VISUAL_OWNERSHIP_DEBT: set[str] = set()

# 已完成同目录 UIX 视觉迁移的 widget 目录；删除声明文件不得伪装成债务清零。
MIGRATED_WIDGET_DIRS = {
    "containers/back_top",
    "display/avatar",
    "display/badge",
    "display/card",
    "display/carousel",
    "display/calendar",
    "display/collapse",
    "display/descriptions",
    "display/empty",
    "display/image",
    "display/image_group",
    "display/list",
    "display/qrcode",
    "display/result",
    "display/selectable_list",
    "display/skeleton",
    "display/tag",
    "display/timeline",
    "display/watermark",
    "feedback/progress",
    "feedback/spin",
    "general/button_group",
    "general/divider",
    "other/theme_toggle",
    "window_controls",
}


def is_visual_passthrough(source: str) -> bool:
    """识别只把 kernel/children 原样交回 Rust 的单 KernelView 壳。"""

    without_comments = re.sub(r"//[^\n]*", "", source)
    normalized = " ".join(without_comments.split())
    match = re.fullmatch(
        r"<KernelView\s+value=\{build_[A-Za-z0-9_]+_view\(([^()]*)\)\}\s*/>",
        normalized,
    )
    if match is None:
        return False
    arguments = [argument.strip() for argument in match.group(1).split(",")]
    return bool(arguments) and all(
        argument in {"kernel", "children"} for argument in arguments
    )


class WidgetUixColocationTests(unittest.TestCase):
    # 已完成迁移的目录集合只能显式扩展，不能通过删除 UIX 文件规避守卫。
    def test_migrated_widget_directory_set_is_locked(self) -> None:
        actual = {
            path.parent.relative_to(WIDGETS_ROOT).as_posix()
            for path in WIDGETS_ROOT.rglob("*.uix")
        }
        self.assertEqual(actual, MIGRATED_WIDGET_DIRS)

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

    # 纯 KernelView 透传尚不满足“视觉由 UIX 描述”；锁定存量且禁止新增。
    def test_visual_passthrough_debt_only_shrinks(self) -> None:
        actual = {
            path.relative_to(WIDGETS_ROOT).as_posix()
            for path in WIDGETS_ROOT.rglob("*.uix")
            if is_visual_passthrough(path.read_text(encoding="utf-8"))
        }
        self.assertEqual(actual, VISUAL_OWNERSHIP_DEBT)


if __name__ == "__main__":
    unittest.main()
