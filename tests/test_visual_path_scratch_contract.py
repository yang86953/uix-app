# 验证 WidgetTree 视觉坐标查询复用树级路径容器。
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
TREE_CORE = ROOT / "src/ui/widget_runtime/widget/tree_core/mod.rs"
TREE_TRANSFORM = ROOT / "src/ui/widget_runtime/widget/tree_transform.rs"


class VisualPathScratchContractTests(unittest.TestCase):
    """锁定视觉路径的所有权、重入和零临时容器契约。"""

    def test_widget_tree_owns_one_visual_path_scratch(self) -> None:
        source = TREE_CORE.read_text(encoding="utf-8")
        self.assertIn(
            "visual_path_scratch: std::cell::RefCell<Vec<WidgetId>>",
            source,
        )
        self.assertEqual(source.count("visual_path_scratch:"), 2)

    def test_visual_queries_share_fill_and_consume_helpers(self) -> None:
        source = TREE_TRANSFORM.read_text(encoding="utf-8")
        self.assertIn("fn fill_visual_path(", source)
        self.assertIn("fn with_visual_path<R>(", source)
        self.assertIn("self.visual_path_scratch.try_borrow_mut()", source)
        self.assertEqual(source.count("self.with_visual_path("), 2)
        self.assertEqual(source.count("self.with_visible_visual_path("), 1)
        self.assertNotIn("fn visual_path(&self, id: WidgetId) -> Vec<WidgetId>", source)

    def test_clipped_projection_keeps_full_visibility_walk_without_local_path(self) -> None:
        source = TREE_TRANSFORM.read_text(encoding="utf-8")
        visible_start = source.index("    fn fill_visible_visual_path(")
        visible_end = source.index("    /// 在复用工作区上执行", visible_start)
        visible_fill = source[visible_start:visible_end]
        start = source.index("    pub(crate) fn clipped_visual_rect(")
        end = source.index("    /// 计算节点在屏幕上的可见视觉矩形", start)
        clipped = source[start:end]

        self.assertIn("while let Some(current_id) = current", visible_fill)
        self.assertIn("if !node.visible()", visible_fill)
        self.assertIn("self.with_visible_visual_path(id, |path|", clipped)
        self.assertNotIn("let mut path = Vec::new()", clipped)


if __name__ == "__main__":
    unittest.main()
