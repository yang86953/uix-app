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
    # Drawing GPU Module 探针路径。
    ROOT / "src/draw/backend/gpu/device_probe.rs",
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
        self.assertIn("range: DrawRange", shared)
        # DrawBufferBindings 必须把顶点与 uniform 绑定封装为不可拆的私有值。
        self.assertIn("pub(crate) struct DrawBufferBindings", shared)
        # 两个 buffer 绑定字段必须保持非可选且对外只读。
        self.assertIn("vertex: BufferHandle", shared)
        # Uniform 字段必须与顶点字段一样保持非可选。
        self.assertIn("uniform: BufferHandle", shared)
        # 绑定值必须提供一次接收两个身份的完整构造器。
        self.assertIn("pub(crate) const fn new(vertex: BufferHandle, uniform: BufferHandle) -> Self", shared)
        # 顶点资源只能通过只读投影取得。
        self.assertIn("pub(crate) const fn vertex(self) -> BufferHandle", shared)
        # Uniform 资源只能通过只读投影取得。
        self.assertIn("pub(crate) const fn uniform(self) -> BufferHandle", shared)
        # DrawPacket 必须私有保存不可拆的 pipeline 身份。
        self.assertIn("pipeline: PipelineBinding", shared)
        # DrawPacket 必须私有保存完整 Buffer 角色。
        self.assertIn("buffers: DrawBufferBindings", shared)
        # DrawPacket 必须私有保存互斥绘制范围。
        self.assertIn("range: DrawRange", shared)
        # DrawPacket 必须由完整三参数构造器一次冻结全部事实。
        self.assertIn("pub(crate) const fn new(\n        // 接收不可拆分的 pipeline 身份与共享语义。", shared)
        # pipeline 身份只能通过只读投影消费。
        self.assertIn("pub(crate) const fn pipeline(self) -> PipelineBinding", shared)
        # Buffer 角色只能通过只读投影消费。
        self.assertIn("pub(crate) const fn buffers(self) -> DrawBufferBindings", shared)
        # 绘制范围只能通过只读投影消费。
        self.assertIn("pub(crate) const fn range(self) -> DrawRange", shared)
        # 三项 packet 字段不得重新暴露 crate 内写权限。
        for field in ("pipeline: PipelineBinding", "buffers: DrawBufferBindings", "range: DrawRange"):
            # 每项公开字段写法都必须从 DrawPacket 消失。
            self.assertNotIn(f"pub(crate) {field}", shared)
        # 半成品 triangles 构造器和可选 uniform 必须永久退出契约。
        self.assertNotIn("fn triangles(", shared)
        self.assertNotIn("uniform_buffer: Option<BufferHandle>", shared)
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
        # packet 值域门禁必须委托给唯一范围。
        self.assertIn("self.range.is_valid()", shared)
        # FramePlan 必须继续通过 packet 门禁拒绝空或溢出 draw。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 锁定 FramePlan 到共享门禁的调用关系。
        self.assertIn("packet.has_valid_range()", frame_plan)

    # 所有既有生产端必须通过类型化构造器建立非索引范围。
    def test_all_current_producers_construct_typed_ranges(self) -> None:
        # 合并全部生产 DrawPacket 的源码。
        producers = "\n".join(path.read_text(encoding="utf-8") for path in PRODUCERS)
        # 当前十九个 packet 必须全部显式构造顶点范围。
        self.assertEqual(producers.count("DrawPacket::new("), 19)
        # 当前十九个 packet 必须显式构造完整 typed buffer bindings。
        self.assertEqual(producers.count("DrawBufferBindings::new("), 19)
        # 生产端不得退回结构字面量或可选 uniform 绑定。
        self.assertNotIn("DrawPacket {", producers)
        self.assertNotIn("uniform_buffer: Some", producers)
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
        self.assertIn("let range = packet.range();", opengl)
        # D3D11 也必须取得同一个共享范围。
        self.assertIn("let range = packet.range();", d3d11)
        # OpenGL 必须从范围取得索引绑定与两类计数。
        self.assertIn("range.index_binding()", opengl)
        # OpenGL 必须从范围取得 checked 有符号索引数量。
        self.assertIn(".index_count_i32()", opengl)
        # OpenGL 必须从范围取得 checked 有符号顶点数量。
        self.assertIn(".vertex_count_i32()", opengl)
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
        # 两个 Adapter 必须通过 packet 的只读 pipeline 与 buffer 投影消费绑定。
        self.assertIn("packet.pipeline()", opengl + d3d11)
        # 两个 Adapter 必须原子取得顶点与 Uniform 绑定。
        self.assertIn("packet.buffers()", opengl + d3d11)
        # Adapter 不得恢复旧的直接字段读取模式。
        for marker in ("packet.pipeline.contract", "packet.pipeline.kind", "packet.pipeline;", "packet.buffers;", "packet.range;", "packet.range.index_binding"):
            # 只拒绝明确旧模式，保留合法 accessor 调用。
            self.assertNotIn(marker, opengl + d3d11)

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
