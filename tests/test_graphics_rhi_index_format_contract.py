# -*- coding: utf-8 -*-
# 验证 FramePlan 索引格式绑定在两个 Adapter 中保持同一解释。
"""Keep typed index buffer formats identical across native graphics adapters."""

# 启用延迟注解解析，保持测试与其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 DrawPacket 索引绑定契约。
SHARED = ROOT / "src/native/present/rhi/draw_packet.rs"
# 定位 FramePlan 命令闭集。
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan.rs"
# 定位 FramePlan 拥有的类型化上传值对象。
FRAME_UPLOAD = ROOT / "src/draw/backend/frame_plan_upload.rs"
# 定位 indexed Draw 的 pass-local 契约验证。
FRAME_VALIDATION = ROOT / "src/draw/backend/frame_plan_validation.rs"
# 定位 FramePlan 只读预检与 Device 执行边界。
FRAME_EXECUTION = ROOT / "src/draw/backend/frame_plan_execution.rs"
# 定位 OpenGL draw 格式翻译。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 draw 分派边界。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs"
# 定位 D3D11 统一索引格式映射。
D3D11_MODULE = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/mod.rs"
# 定位必须复用统一索引绑定入口的 D3D11 shader helper。
D3D11_HELPERS = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline"


# 集中验证共享索引格式绑定与两个 Adapter 的机械映射。
class GraphicsRhiIndexFormatContractTests(unittest.TestCase):
    # DrawPacket 必须把索引资源与元素格式绑定为一个事实。
    def test_draw_packet_owns_typed_index_binding(self) -> None:
        # 读取共享 DrawPacket 契约。
        shared = SHARED.read_text(encoding="utf-8")
        # 共享层必须定义封闭索引格式。
        self.assertIn("pub(crate) enum IndexFormat", shared)
        # 当前既有 ABI 必须显式命名 Uint32。
        self.assertIn("Uint32", shared)
        # 索引绑定必须同时保存资源与格式。
        self.assertIn("pub(crate) struct IndexBufferBinding", shared)
        # 索引范围必须原子保存类型化绑定。
        self.assertIn("binding: IndexBufferBinding", shared)
        # DrawPacket 不得继续暴露可与范围矛盾的独立索引字段。
        self.assertNotIn("pub(crate) index_buffer:", shared)
        # 共享格式必须拥有唯一步长算法。
        self.assertIn("pub(crate) const fn stride_bytes", shared)
        # 共享格式必须拥有 checked 字节偏移算法。
        self.assertIn("pub(crate) const fn byte_offset", shared)
        # 偏移溢出必须显式失败。
        self.assertIn("first_index.checked_mul(self.stride_bytes())", shared)

    # FramePlan 必须拥有 indexed Draw 的格式、内容和可访问范围。
    def test_frame_plan_owns_typed_index_content(self) -> None:
        # 读取 FramePlan 命令闭集。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 读取类型化索引载荷。
        upload = FRAME_UPLOAD.read_text(encoding="utf-8")
        # 读取 indexed Draw 共享验证。
        validation = FRAME_VALIDATION.read_text(encoding="utf-8")
        # 读取上传只读预检和唯一执行边界。
        execution = FRAME_EXECUTION.read_text(encoding="utf-8")
        # 截取索引命令的唯一 Device 执行分支。
        index_execution = execution.split("FramePlanCommand::UploadIndex { buffer, data } =>", maxsplit=1)[1].split(
            # 以后续 Uniform 分支作为索引分支的稳定右边界。
            "FramePlanCommand::UploadUniform",
            # 只截取第一个分支间隔。
            maxsplit=1,
        )[0]
        # 命令闭集必须显式交付索引 Buffer 和类型化内容。
        self.assertIn("UploadIndex {", frame_plan)
        # FramePlan 必须用封闭值对象拥有索引序列。
        self.assertIn("pub(crate) enum FrameIndexPayload", upload)
        # 当前共享 ABI 必须将 Uint32 格式与 u32 值绑定。
        self.assertIn("Uint32(Arc<[u32]>)", upload)
        # 上层计划不得退回无语义的裸字节所有权。
        self.assertNotIn("Arc<[u8]>", upload)
        # 值对象必须提供选中范围的最大索引查询。
        self.assertIn("fn max_index_in_range", upload)
        # 索引载荷只能在 Device 边界编码为字节。
        self.assertIn("let bytes = data.encode_ne_bytes();", index_execution)
        # 索引字节必须通过共享上传值对象交付 Device。
        self.assertIn("RhiBufferUpload::new(*buffer, &bytes)", index_execution)
        # indexed Draw 必须查找当前 pass 中同一 Buffer 的最近上传。
        self.assertIn("if *buffer == index_binding.buffer()", validation)
        # DrawRange 格式必须与类型化内容格式一致。
        self.assertIn("index_data.format() != index_binding.format()", validation)
        # DrawRange 只能选择已交付的索引值。
        self.assertIn(".max_index_in_range(packet.range.first_index(), packet.range.index_count())", validation)
        # 选中的最大索引必须小于已交付顶点数量。
        self.assertIn("selected_max >= uploaded_vertex_count", validation)
        # 真实索引 Buffer 角色与容量必须在 Surface acquire 前预检。
        self.assertIn("RhiBufferUploadPreflight::index(*buffer, data.size_bytes())", execution)

    # OpenGL 必须只从共享格式派生步长、原生类型和偏移。
    def test_opengl_maps_shared_index_format(self) -> None:
        # 读取 OpenGL draw 格式翻译源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 资源解析必须从类型化绑定取得句柄。
        self.assertIn("self.buffer(index_binding.buffer())?", opengl)
        # 资源步长必须与共享格式一致。
        self.assertIn("index.desc.stride_bytes() != index_format.stride_bytes()", opengl)
        # 原生元素类型必须通过封闭格式翻译入口取得。
        self.assertIn("gl_index_type(index_format)", opengl)
        # OpenGL 必须穷尽映射当前 Uint32 格式。
        self.assertIn("IndexFormat::Uint32 => glow::UNSIGNED_INT", opengl)
        # 首索引偏移必须调用共享 checked 算法。
        self.assertIn(".byte_offset(range.first_index())", opengl)
        # Adapter 不得继续使用饱和乘法掩盖偏移溢出。
        self.assertNotIn("saturating_mul", opengl)

    # D3D11 必须一次映射格式并让全部 shader helper 复用绑定入口。
    def test_d3d11_maps_shared_index_format_once(self) -> None:
        # 读取 D3D11 draw 分派源码。
        draw = D3D11_DRAW.read_text(encoding="utf-8")
        # 读取 D3D11 统一索引格式映射源码。
        module = D3D11_MODULE.read_text(encoding="utf-8")
        # D3D11 资源解析必须从类型化绑定取得句柄。
        self.assertIn("self.rhi_device.buffer(binding.buffer())?", draw)
        # D3D11 资源步长必须与共享格式一致。
        self.assertIn("index.desc.stride_bytes() != format.stride_bytes()", draw)
        # 分派层必须构造一次已经完成原生格式映射的绑定。
        self.assertIn("D3d11IndexBinding::new(index.native.clone(), format)", draw)
        # D3D11 必须穷尽映射当前 Uint32 格式。
        self.assertIn("IndexFormat::Uint32 => DXGI_FORMAT_R32_UINT", module)
        # 收集所有薄 RHI shader helper 源码。
        helpers = "\n".join(path.read_text(encoding="utf-8") for path in sorted(D3D11_HELPERS.glob("rhi_*.rs")))
        # 七个实际编码 helper 必须复用同一个索引绑定入口。
        self.assertEqual(helpers.count("bind_rhi_index_buffer(context, index)"), 7)
        # shader helper 不得继续各自写死 DXGI 索引格式。
        self.assertNotIn("DXGI_FORMAT_R32_UINT", helpers)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
