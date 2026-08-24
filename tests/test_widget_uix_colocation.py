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

# Rust 完整静态表或 Visual::default 数值债务已经清零；禁止重新引入双源事实。
RUST_VISUAL_DEFAULT_DEBT: set[str] = set()

# 巨型位置参数视觉壳债务已经清零；新组件必须使用具名 Visual 或真实子树。
POSITIONAL_VISUAL_SHELL_DEBT: set[str] = set()

# 已完成同目录 UIX 视觉迁移的 widget 目录；删除声明文件不得伪装成债务清零。
MIGRATED_WIDGET_DIRS = {
    "containers/back_top",
    "containers/affix",
    "containers/container",
    "containers/grid",
    "containers/layout",
    "containers/layout/content",
    "containers/layout/footer",
    "containers/layout/header",
    "containers/layout/sider",
    "containers/splitter",
    "containers/scroll_view",
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
    "feedback/drawer",
    "feedback/message",
    "feedback/modal",
    "feedback/notification",
    "feedback/popover",
    "feedback/popconfirm",
    "feedback/progress",
    "feedback/spin",
    "feedback/tooltip",
    "general/button",
    "general/button_group",
    "general/divider",
    "general/float_button",
    "general/float_button/back_top",
    "general/float_button/group",
    "general/icon",
    "general/label",
    "general/space",
    "general/typography",
    "input/autocomplete",
    "input/cascader",
    "input/checkbox",
    "input/color_picker",
    "input/date_picker",
    "input/date_range_picker",
    "input/input",
    "input/mentions",
    "input/select",
    "input/time_picker",
    "input/transfer",
    "input/tree_select",
    "input/upload",
    "input/input_group",
    "input/input_number",
    "input/radio",
    "input/range_slider",
    "input/rate",
    "input/segmented",
    "input/slider",
    "input/switch",
    "other/theme_toggle",
    "tooltip_primitives",
    "window_controls",
}

# 没有可见树或视觉参数的声明租约不是公开视觉 widget，不能伪造 UIX 根。
NON_VISUAL_WIDGET_TYPES = {"MessageDeclaration", "NotificationDeclaration"}

# 这些目录保存组合声明或框架内核视觉，不对应独立的 widget! 公开结构体。
MIGRATED_SUPPORT_DIRS = {
    "general/button_group",
    "general/float_button/back_top",
    "input/input_group",
    "tooltip_primitives",
    "window_controls",
}

# 公开视觉 widget 的剩余迁移债务；完成一项时必须同步删除，最终目标为空集合。
UNMIGRATED_PUBLIC_WIDGET_DIRS = {
    "display/chart/advanced",
    "navigation/anchor",
    "navigation/breadcrumb",
    "navigation/dropdown",
    "navigation/menu",
    "navigation/nav",
    "navigation/navigation_shell",
    "navigation/pagination",
    "navigation/steps",
    "navigation/tabs",
}

# 提取 widget! 声明的公开结构体；Button 是唯一手写 Widget 实现，单独登记。
PUBLIC_WIDGET_DECLARATION = re.compile(
    r"widget!\s*\{.*?pub\s+struct\s+([A-Za-z0-9_]+)", re.DOTALL
)


def public_visual_widget_dirs() -> set[str]:
    """从真实 widget! 声明推导每个公开视觉组件应拥有的独立目录。"""

    directories = {"general/button"}
    for rust_path in sorted(WIDGETS_ROOT.rglob("*.rs")):
        source = rust_path.read_text(encoding="utf-8")
        names = PUBLIC_WIDGET_DECLARATION.findall(source)
        if not names or all(name in NON_VISUAL_WIDGET_TYPES for name in names):
            continue
        # mod.rs/widget.rs 已位于组件目录；分类根下的单文件组件目标目录取文件名。
        owner = (
            rust_path.parent
            if rust_path.name in {"mod.rs", "widget.rs"}
            else rust_path.with_suffix("")
        )
        directories.add(owner.relative_to(WIDGETS_ROOT).as_posix())
    return directories


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
    # 所有公开视觉 widget 必须显式落入“已迁移”或“剩余债务”，不能被绿灯漏掉。
    def test_public_visual_widget_inventory_is_complete(self) -> None:
        actual = public_visual_widget_dirs()
        self.assertEqual(len(actual), 84)
        self.assertEqual(actual - MIGRATED_WIDGET_DIRS, UNMIGRATED_PUBLIC_WIDGET_DIRS)
        self.assertEqual(MIGRATED_WIDGET_DIRS - actual, MIGRATED_SUPPORT_DIRS)

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
