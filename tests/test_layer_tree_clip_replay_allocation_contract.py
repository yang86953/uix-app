# 验证 LayerTree 的逐片裁剪重放不在每帧复制完整片段集合。
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
LAYER_RENDER = ROOT / "src/draw/scene/layer_tree/render.rs"


class LayerTreeClipReplayAllocationContractTests(unittest.TestCase):
    """锁定带裁剪节点的零临时容器重放契约。"""

    def test_render_node_copies_rect_values_without_cloning_vec(self) -> None:
        source = LAYER_RENDER.read_text(encoding="utf-8")
        start = source.index("    fn render_node(")
        end = source.index("    fn render_node_unclipped(", start)
        render_node = source[start:end]

        self.assertIn(
            "let clip_region_count = node.clip_regions().map(<[Rect]>::len);",
            render_node,
        )
        self.assertIn("for clip_region_index in 0..clip_region_count", render_node)
        self.assertIn("[clip_region_index]", render_node)
        self.assertNotIn("to_vec", render_node)
        self.assertNotIn("collect", render_node)
        self.assertNotIn("Vec::", render_node)

    def test_empty_and_fragmented_clip_semantics_remain_explicit(self) -> None:
        source = LAYER_RENDER.read_text(encoding="utf-8")
        start = source.index("    fn render_node(")
        end = source.index("    fn render_node_unclipped(", start)
        render_node = source[start:end]

        self.assertIn("if clip_region_count == 0", render_node)
        self.assertLess(render_node.index("push_clip(clip_region)"), render_node.index("pop_clip()"))
        self.assertLess(render_node.index("pop_clip()"), render_node.index("result?"))


if __name__ == "__main__":
    unittest.main()
