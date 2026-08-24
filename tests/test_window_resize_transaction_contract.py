# -*- coding: utf-8 -*-
"""冻结 Surface resize 后布局、绘制与呈现代际共同失效的窗口契约。"""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SUPPORT = ROOT / "src" / "app" / "window" / "window_driver" / "support.rs"
DRIVER = ROOT / "src" / "app" / "window" / "window_driver" / "driver.rs"
CONTAINER = ROOT / "src" / "ui" / "widgets" / "containers" / "container" / "mod.rs"
CONTAINER_UIX = ROOT / "src" / "ui" / "widgets" / "containers" / "container" / "container.uix"


class WindowResizeTransactionContractTests(unittest.TestCase):
    # 根 frame 改变必须产生 Layout 失效，不能只改几何并依赖主窗的临时布尔值。
    def test_root_surface_sync_publishes_layout_and_full_paint(self) -> None:
        support = SUPPORT.read_text(encoding="utf-8")

        sync = support[support.index("fn sync_root_surface_frame") :]
        sync = sync[: sync.index("/// 读取平台窗口协议提供的当前逻辑客户区")]
        self.assertIn("root.set_frame", sync)
        self.assertIn("tree.push_layout_invalidation(root_id)", sync)
        self.assertIn("tree.mark_full_frame_dirty()", sync)

    # Surface 重建后旧 retained 帧与外部 present 历史都必须作废。
    def test_successful_resize_resets_retained_present_state(self) -> None:
        driver = DRIVER.read_text(encoding="utf-8")

        resize = driver[driver.index("UiEventType::WindowResize =>") :]
        resize = resize[: resize.index("UiEventType::WindowMaximize =>")]
        self.assertIn("if resized", resize)
        self.assertIn("self.rendered_first = false", resize)
        self.assertIn("self.present_damage_tracker.reset()", resize)
        self.assertIn("sync_root_frame_exactly_to_engine", resize)

    # 已分配 frame 必须压过历史自然尺寸，确保最大化还原后整棵 Stretch 子树收缩。
    def test_container_cross_axis_uses_current_allocated_frame(self) -> None:
        container = CONTAINER.read_text(encoding="utf-8")
        container_uix = CONTAINER_UIX.read_text(encoding="utf-8")

        cross_axis = container[container.index("let cross_axis_indefinite =") :]
        cross_axis = cross_axis[: cross_axis.index("// 委托给统一的 FlexLayout")]
        self.assertIn(
            "content_rect.h <= self.visual.layout.bootstrap_cross_axis_threshold",
            cross_axis,
        )
        self.assertIn(
            "content_rect.w <= self.visual.layout.bootstrap_cross_axis_threshold",
            cross_axis,
        )
        self.assertIn("bootstrapCrossAxisThreshold={1.0}", container_uix)
        self.assertNotIn("s.width.is_none_or", cross_axis)
        self.assertNotIn("s.height.is_none_or", cross_axis)


if __name__ == "__main__":
    unittest.main()
