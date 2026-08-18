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
