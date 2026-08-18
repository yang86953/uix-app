# -*- coding: utf-8 -*-
# 验证两个图形 Adapter 只机械映射共享类型化顶点属性序列。
"""Keep vertex attribute ABI facts in one shared RHI component."""

# 启用延迟注解解析，保持契约脚本风格一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享顶点布局 Component。
SHARED = ROOT / "src/native/present/rhi/vertex_layout.rs"
# 定位共享 pipeline 组合契约。
PIPELINE = ROOT / "src/native/present/rhi/pipeline.rs"
# 定位 OpenGL 顶点属性映射。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 OpenGL 独立布局 Adapter Component。
OPENGL_LAYOUT = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_vertex_layout.rs"
# 定位 D3D11 输入布局映射。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/pipeline.rs"


# 集中验证共享属性事实与两个 Adapter 的机械消费关系。
class GraphicsRhiVertexLayoutContractTests(unittest.TestCase):
    # 共享 Component 必须完整拥有顶点属性 ABI。
    def test_shared_component_owns_complete_attribute_sequence(self) -> None:
        # 读取共享顶点布局源码。
        shared = SHARED.read_text(encoding="utf-8")
        # 属性语义必须使用封闭枚举。
        self.assertIn("enum PipelineVertexSemantic", shared)
        # 属性格式必须使用封闭枚举。
        self.assertIn("enum PipelineVertexFormat", shared)
        # 位置、语义、格式和偏移必须绑定为一个值对象。
        self.assertIn("struct PipelineVertexAttribute", shared)
        # position/uv/color 必须由同一静态序列定义。
        self.assertIn("const POSITION_UV_COLOR_F32_ATTRIBUTES", shared)
        # 两个 Adapter 必须取得同一个只读属性切片。
        self.assertIn("fn attributes(self) -> &'static [PipelineVertexAttribute]", shared)
        # 布局必须统一验证槽位、偏移和 stride。
        self.assertIn("pub(crate) const fn is_valid(self) -> bool", shared)
        # OpenGL stride 必须通过共享 checked 投影。
        self.assertIn("pub(crate) const fn stride_bytes_i32(self) -> Option<i32>", shared)

    # OpenGL 必须从 pipeline 契约一次配置全部属性。
    def test_opengl_iterates_shared_layout_without_private_tables(self) -> None:
        # 读取 OpenGL draw 分派源码。
        opengl_draw = OPENGL.read_text(encoding="utf-8")
        # 读取 OpenGL 顶点布局映射源码。
        opengl_layout = OPENGL_LAYOUT.read_text(encoding="utf-8")
        # draw 必须直接消费当前 pipeline 的共享布局。
        self.assertIn("configure_vertex_attributes(gl, contract.vertex)?", opengl_draw)
        # 配置器必须遍历共享属性序列。
        self.assertIn("for attribute in layout.attributes()", opengl_layout)
        # stride 必须使用共享 checked 投影。
        self.assertIn(".stride_bytes_i32()", opengl_layout)
        # 属性偏移必须使用共享 checked 投影。
        self.assertIn(".offset_bytes_i32()", opengl_layout)
        # 旧 float2 私有布局 helper 必须消失。
        self.assertNotIn("configure_float2_attributes", opengl_draw + opengl_layout)
        # 旧 float8 私有布局 helper 必须消失。
        self.assertNotIn("configure_float8_attributes", opengl_draw + opengl_layout)
        # Adapter 不得再直接把共享 stride 截断为 i32。
        self.assertNotIn("stride as i32", opengl_draw + opengl_layout)

    # D3D11 必须从同一属性切片创建原生输入布局。
    def test_d3d11_maps_the_same_shared_attribute_sequence(self) -> None:
        # 读取 D3D11 pipeline Adapter 源码。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 原生描述符必须遍历共享属性序列。
        self.assertIn(".attributes()", d3d11)
        # D3D11 必须执行与 OpenGL 相同的共享有效性门禁。
        self.assertIn("if !layout.is_valid()", d3d11)
        # position 输入布局必须由共享枚举选择。
        self.assertIn("d3d11_vertex_elements(PipelineVertexLayout::PositionF32x2)", d3d11)
        # 采样输入布局必须由共享枚举选择。
        self.assertIn(
            # 锁定完整共享布局选择表达式。
            "d3d11_vertex_elements(PipelineVertexLayout::PositionUvColorF32)",
            # 在 D3D11 Adapter 中查找。
            d3d11,
        )
        # D3D11 字节偏移必须直接来自共享属性。
        self.assertIn("AlignedByteOffset: attribute.offset_bytes()", d3d11)
        # 旧基础输入描述符数组必须消失。
        self.assertNotIn("let input_elems = [D3D11_INPUT_ELEMENT_DESC", d3d11)
        # 旧采样输入描述符数组必须消失。
        self.assertNotIn("let glyph_elems = [", d3d11)

    # pipeline 只组合独立顶点布局 Component，不再拥有重复定义。
    def test_pipeline_composes_the_vertex_layout_component(self) -> None:
        # 读取共享 pipeline 源码。
        pipeline = PIPELINE.read_text(encoding="utf-8")
        # pipeline 必须从独立 Component 引入布局。
        self.assertIn("use super::vertex_layout::PipelineVertexLayout;", pipeline)
        # pipeline 文件不得重新定义同名布局枚举。
        self.assertNotIn("enum PipelineVertexLayout", pipeline)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
