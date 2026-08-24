# 验证高频命中排序复用窗口树工作区且不恢复逐节点临时容器。
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
TREE_CORE = ROOT / "src/ui/widget_runtime/widget/tree_core/mod.rs"
EVENTS = ROOT / "src/ui/widget_runtime/widget/tree_events/events.rs"


class HitTestSortScratchContractTests(unittest.TestCase):
    """锁定二维与三维命中排序的复用、顺序和重入契约。"""

    def test_widget_tree_owns_one_reusable_order_scratch(self) -> None:
        source = TREE_CORE.read_text(encoding="utf-8")
        self.assertIn(
            "hit_test_order_scratch: std::cell::RefCell<Vec<(WidgetId, usize)>>",
            source,
        )
        self.assertEqual(source.count("hit_test_order_scratch:"), 2)

    def test_recursive_hit_paths_do_not_clone_child_vectors(self) -> None:
        source = EVENTS.read_text(encoding="utf-8")
        start = source.index("    fn hit_test_3d_internal(")
        end = source.index("    // 验证屏幕点是否位于目标节点", start)
        hit_paths = source[start:end]

        self.assertNotIn("children().to_vec()", hit_paths)
        self.assertNotIn("sort_by(", hit_paths)
        self.assertGreaterEqual(hit_paths.count("sort_unstable_by("), 2)
        self.assertGreaterEqual(hit_paths.count("order_scratch.truncate(children_start)"), 2)

    def test_order_and_reentrant_fallback_are_explicit(self) -> None:
        source = EVENTS.read_text(encoding="utf-8")
        self.assertIn("original_a.cmp(&original_b)", source)
        self.assertIn("original_b.cmp(&original_a)", source)
        self.assertGreaterEqual(source.count("try_borrow_mut()"), 2)
        self.assertGreaterEqual(source.count("&mut Vec::new()"), 2)


if __name__ == "__main__":
    unittest.main()
