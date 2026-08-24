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

# Rust 仍保存完整静态表或 Visual::default 数值的存量双源事实；只允许逐项删除。
RUST_VISUAL_DEFAULT_DEBT = {
    "display/card/mod.rs",
    "display/carousel/mod.rs",
    "display/chart/bar_chart/presentation.rs",
    "display/chart/line_chart/presentation.rs",
    "display/chart/pie_chart/presentation.rs",
    "display/rich_text/presentation.rs",
    "display/table/presentation.rs",
    "feedback/alert/presentation.rs",
    "feedback/popover/presentation.rs",
    "feedback/tooltip/presentation.rs",
    "tooltip_primitives.rs",
}

# 仍以单个巨型位置参数调用表达视觉的存量 UIX；新组件必须改用具名 Visual 或真实子树。
POSITIONAL_VISUAL_SHELL_DEBT = {
    "display/card/card.uix",
    "display/carousel/carousel.uix",
    "display/chart/bar_chart/bar_chart.uix",
    "display/chart/line_chart/line_chart.uix",
    "display/chart/pie_chart/pie_chart.uix",
    "display/rich_text/rich_text.uix",
    "display/table/table.uix",
    "feedback/alert/alert.uix",
    "feedback/popover/popover.uix",
    "feedback/tooltip/tooltip.uix",
}

# 已完成同目录 UIX 视觉迁移的 widget 目录；删除声明文件不得伪装成债务清零。
MIGRATED_WIDGET_DIRS = {
    "containers/back_top",
    "display/avatar",
    "display/badge",
    "display/card",
    "display/carousel",
    "display/calendar",
    "display/chart/bar_chart",
    "display/chart/line_chart",
    "display/chart/pie_chart",
    "display/collapse",
    "display/descriptions",
    "display/empty",
    "display/image",
    "display/image_group",
    "display/list",
    "display/qrcode",
    "display/result",
    "display/rich_text",
    "display/selectable_list",
    "display/skeleton",
    "display/table",
    "display/tag",
    "display/timeline",
    "display/tree",
    "display/watermark",
    "feedback/alert",
    "feedback/popover",
    "feedback/progress",
    "feedback/spin",
    "feedback/tooltip",
    "general/button_group",
    "general/divider",
    "general/icon",
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


def is_positional_visual_shell(source: str) -> bool:
    """识别没有具名 Visual、只靠 build_*_view 巨型位置调用表达视觉的根。"""

    without_comments = re.sub(r"//[^\n]*", "", source)
    normalized = " ".join(without_comments.split())
    return "<Visual" not in normalized and re.fullmatch(
        r"<KernelView\s+value=\{build_[A-Za-z0-9_]+_view\(.*\)\}\s*/>",
        normalized,
    ) is not None


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

    # 完整 Rust 默认视觉表会形成第二事实源；锁定存量并禁止新建。
    def test_rust_visual_default_debt_only_shrinks(self) -> None:
        pattern = re.compile(
            r"\bstatic\s+DEFAULT_[A-Z0-9_]+_VISUAL\b"
            r"|\bimpl\s+Default\s+for\s+[A-Za-z0-9_]+Visual\b"
        )
        actual = {
            path.relative_to(WIDGETS_ROOT).as_posix()
            for path in WIDGETS_ROOT.rglob("*.rs")
            if pattern.search(path.read_text(encoding="utf-8"))
        }
        self.assertEqual(actual, RUST_VISUAL_DEFAULT_DEBT)

    # 巨型位置参数壳会把 UIX 与 Rust 函数签名耦合；锁定存量并禁止新增。
    def test_positional_visual_shell_debt_only_shrinks(self) -> None:
        actual = {
            path.relative_to(WIDGETS_ROOT).as_posix()
            for path in WIDGETS_ROOT.rglob("*.uix")
            if is_positional_visual_shell(path.read_text(encoding="utf-8"))
        }
        self.assertEqual(actual, POSITIONAL_VISUAL_SHELL_DEBT)

    # Visual 必须由同目录 Rust 模块通过 uix_items! 生成，不能只写声明而继续用 Rust 副本。
    def test_visual_declarations_generate_module_items(self) -> None:
        violations: list[str] = []
        for uix_path in sorted(WIDGETS_ROOT.rglob("*.uix")):
            source = uix_path.read_text(encoding="utf-8")
            if "<Visual" not in source:
                continue
            rust_source = "\n".join(
                path.read_text(encoding="utf-8")
                for path in sorted(uix_path.parent.glob("*.rs"))
            )
            relative = uix_path.relative_to(ROOT).as_posix()
            if f'crate::uix_items!("{relative}")' not in rust_source:
                violations.append(relative)
        self.assertEqual(violations, [])


if __name__ == "__main__":
    unittest.main()
