# -*- coding: utf-8 -*-
# 验证 FramePlan 绘制范围不能表达跨 Adapter 分歧的字段组合。
"""Keep DrawRange closed and identical across native graphics adapters."""

# 启用延迟注解解析，保持测试与其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 DrawPacket 与 DrawRange 契约。
SHARED = ROOT / "src/native/present/rhi/draw_packet.rs"
# 定位 FramePlan 绘制命令门禁。
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan.rs"
# 定位 OpenGL 绘制 Adapter。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 绘制分派 Adapter。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs"
# 定位 D3D11 原生命令 helper。
D3D11_HELPERS = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline"
# 列出全部生产 DrawPacket 的 Drawing 与探针源文件。
PRODUCERS = (
    # 通用 renderer 主路径。
    ROOT / "src/draw/backend/rhi_renderer.rs",
    # blur 双 pass 路径。
    ROOT / "src/draw/backend/rhi_renderer_blur.rs",
    # coverage 路径。
    ROOT / "src/draw/backend/rhi_renderer_coverage.rs",
    # gradient 路径。
    ROOT / "src/draw/backend/rhi_renderer_gradient.rs",
    # mixed 命令路径。
    ROOT / "src/draw/backend/rhi_renderer_mixed.rs",
    # sampled 路径。
    ROOT / "src/draw/backend/rhi_renderer_sampled.rs",
    # shadow 路径。
    ROOT / "src/draw/backend/rhi_renderer_shadow.rs",
    # shape 路径。
    ROOT / "src/draw/backend/rhi_renderer_shape.rs",
    # 原生 RHI 探针路径。
    ROOT / "src/native/present/rhi/probe.rs",
)


# 集中验证类型化范围、生产端和两个 Adapter 的共同子集。
class GraphicsRhiDrawRangeContractTests(unittest.TestCase):
    # DrawPacket 必须只拥有一个互斥绘制范围。
    def test_draw_packet_owns_one_closed_range(self) -> None:
        # 读取共享绘制契约。
        shared = SHARED.read_text(encoding="utf-8")
        # 共享层必须定义封闭范围枚举。
        self.assertIn("pub(crate) enum DrawRange", shared)
        # 非索引变体必须独立表达顶点范围。
        self.assertIn("Vertices {", shared)
        # 索引变体必须独立表达索引范围。
        self.assertIn("Indices {", shared)
        # 索引资源与格式只能由索引变体拥有。
        self.assertIn("binding: IndexBufferBinding", shared)
        # DrawPacket 必须只引用封闭范围。
        self.assertIn("pub(crate) range: DrawRange", shared)
        # DrawPacket 不得保留任何可形成矛盾组合的松散字段。
        for field in (
            # 独立索引资源字段。
            "pub(crate) index_buffer:",
            # 独立顶点数量字段。
            "pub(crate) vertex_count:",
            # 独立索引数量字段。
            "pub(crate) index_count:",
            # 独立首顶点字段。
            "pub(crate) first_vertex:",
            # 独立首索引字段。
            "pub(crate) first_index:",
            # OpenGL ES 共同子集以外的基顶点字段。
            "pub(crate) base_vertex:",
        ):
            # 每个旧字段都必须从共享 packet 消失。
            self.assertNotIn(field, shared)
        # packet 非空门禁必须委托给唯一范围。
        self.assertIn("self.range.is_non_empty()", shared)
        # FramePlan 必须继续通过 packet 门禁拒绝空 draw。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 锁定 FramePlan 到共享门禁的调用关系。
        self.assertIn("packet.is_non_empty()", frame_plan)

    # 所有既有生产端必须通过类型化构造器建立非索引范围。
    def test_all_current_producers_construct_typed_ranges(self) -> None:
        # 合并全部生产 DrawPacket 的源码。
        producers = "\n".join(path.read_text(encoding="utf-8") for path in PRODUCERS)
        # 当前十九个 packet 必须全部显式构造顶点范围。
        self.assertEqual(producers.count("range: DrawRange::vertices("), 19)
        # 生产 packet 不得再回填独立索引资源字段。
        self.assertNotIn("index_buffer:", producers)
        # 生产 packet 不得再回填独立基顶点字段。
        self.assertNotIn("base_vertex:", producers)

    # OpenGL 与 D3D11 必须只投影共享范围且不再分歧处理基顶点。
    def test_adapters_project_the_same_range_contract(self) -> None:
        # 读取 OpenGL Adapter 源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 读取 D3D11 Adapter 源码。
        d3d11 = D3D11_DRAW.read_text(encoding="utf-8")
        # 两个 Adapter 都必须先取得共享范围。
        self.assertIn("let range = packet.range;", opengl)
        # D3D11 也必须取得同一个共享范围。
        self.assertIn("let range = packet.range;", d3d11)
        # OpenGL 必须从范围取得索引绑定与两类计数。
        self.assertIn("range.index_binding()", opengl)
        # OpenGL 必须从范围取得索引数量。
        self.assertIn("range.index_count()", opengl)
        # OpenGL 必须从范围取得顶点数量。
        self.assertIn("range.vertex_count()", opengl)
        # D3D11 必须从范围取得索引绑定。
        self.assertIn("range.index_binding()", d3d11)
        # D3D11 必须从范围取得索引数量。
        self.assertIn("let index_count = range.index_count();", d3d11)
        # D3D11 必须从范围取得顶点数量。
        self.assertIn("let vertex_count = range.vertex_count();", d3d11)
        # 两个 Adapter 都不得读取已移除的基顶点字段。
        self.assertNotIn("packet.base_vertex", opengl + d3d11)
        # OpenGL 不得保留后端特有的非零基顶点失败分支。
        self.assertNotIn("nonzero base vertex", opengl)

    # D3D11 helper 必须把共同子集映射为固定零基顶点。
    def test_d3d11_helpers_encode_zero_base_vertex(self) -> None:
        # 合并全部 D3D11 薄 RHI helper 源码。
        helpers = "\n".join(
            # 只读取参与薄 RHI 绘制的 helper。
            path.read_text(encoding="utf-8")
            # 按稳定文件名顺序遍历。
            for path in sorted(D3D11_HELPERS.glob("rhi_*.rs"))
        )
        # 七个实际索引编码点都必须使用共享零基顶点。
        self.assertEqual(helpers.count("DrawIndexed(index_count, first_index, 0)"), 7)
        # helper 不得重新引入后端专属 base_vertex 参数。
        self.assertNotIn("base_vertex", helpers)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
