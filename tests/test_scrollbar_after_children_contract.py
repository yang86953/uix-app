# -*- coding: utf-8 -*-
"""冻结 ScrollView 滚动条在子树完成后进入可见绘制的共享契约。"""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCROLL_VIEW = ROOT / "src" / "ui" / "widgets" / "containers" / "scroll_view" / "mod.rs"
SCENE = ROOT / "src" / "draw" / "scene" / "layer_tree" / "render.rs"


class ScrollbarAfterChildrenContractTests(unittest.TestCase):
    # 组件声明与场景调度必须同时存在，否则 render 中的滚动条代码永远不会执行。
    def test_scrollbar_overlay_is_scheduled_after_children(self) -> None:
        scroll_view = SCROLL_VIEW.read_text(encoding="utf-8")
        scene = SCENE.read_text(encoding="utf-8")

        self.assertIn("paint_after_children => (&self) -> bool", scroll_view)
        self.assertIn("PaintPass::AfterChildren", scroll_view)
        self.assertIn("scrollbar_v", scroll_view)
        self.assertIn("scene.node_paints_after_children", scene)
        self.assertIn("PaintPass::AfterChildren", scene)


if __name__ == "__main__":
    unittest.main()
