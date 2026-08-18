# -*- coding: utf-8 -*-
# 验证 TexturedQuad 两端共享单位颜色域并保持普通/加法采样语义一致。
"""Keep textured tint validation and shader arithmetic identical across adapters."""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享顶点上传契约。
UPLOAD = ROOT / "src/draw/backend/frame_plan_upload.rs"
# 定位共享 pipeline 契约。
PIPELINE = ROOT / "src/native/present/rhi/pipeline.rs"
# 定位 OpenGL ES shader。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs"
# 定位 OpenGL ES pipeline 到 shader 的机械选择边界。
OPENGL_PIPELINE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_pipeline.rs"
# 定位 D3D11 shader 常量。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/mod.rs"
# 定位 D3D11 pipeline 到 draw helper 的机械选择边界。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs"


# 截取两个声明之间的唯一源码范围。
def source_range(source: str, start_marker: str, end_marker: str) -> str:
    # 定位当前声明起点。
    start = source.index(start_marker)
    # 定位下一个声明起点。
    end = source.index(end_marker, start)
    # 返回不混入其它 pipeline 的局部源码。
    return source[start:end]


# 集中验证共享单位域和两端 textured shader 等价性。
class GraphicsRhiTexturedTintContractTests(unittest.TestCase):
    # FramePlan 必须在共享边界验证 PositionUvColor 的颜色索引 4..8。
    def test_shared_vertex_contract_owns_unit_color_domain(self) -> None:
        # 读取共享顶点上传实现。
        upload = UPLOAD.read_text(encoding="utf-8")
        # 采样顶点分支必须存在独立单位域门禁。
        sampled = source_range(upload, "pub(crate) fn is_valid(&self) -> bool", "// 返回编码后的确定字节数量")
        # 颜色范围知识必须位于共享 FramePlan，而不是 Adapter。
        self.assertIn("vertex[4..8]", sampled)
        self.assertIn("*color >= 0.0 && *color <= 1.0", sampled)
        # 位置 float2 分支不得被颜色范围规则改变。
        self.assertIn("Self::PositionF32x2(_) => true", sampled)
        # 通用有限值门禁必须先于颜色域检查。
        self.assertIn("!values.iter().all(|value| value.is_finite())", sampled)
        # 通用结构错误必须先返回，再按封闭布局检查语义字段。
        self.assertLess(sampled.index("return false;"), sampled.index("match self"))

    # OpenGL 和 D3D11 必须对单位域 tint 执行同一采样乘法。
    def test_textured_shader_arithmetic_is_identical(self) -> None:
        # 读取 OpenGL ES shader 源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 读取 D3D11 shader 源码。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 截取 OpenGL textured 顶点和片元阶段。
        gl_vertex = source_range(opengl, "pub(super) const TEXTURED_VERTEX", "pub(super) const TEXTURED_FRAGMENT")
        gl_fragment = source_range(opengl, "pub(super) const TEXTURED_FRAGMENT", "pub(super) const COVERAGE_FRAGMENT")
        # 截取 D3D11 负责提供 color 的 glyph 顶点阶段。
        d3d11_vertex = source_range(d3d11, "const GLYPH_HLSL: &str = r#\"", "const RHI_TEXTURED_PS_HLSL: &str = r#\"")
        # 截取 D3D11 通用 textured 片元阶段。
        d3d11_fragment = source_range(d3d11, "const RHI_TEXTURED_PS_HLSL: &str = r#\"", "const GRADIENT_HLSL: &str = r#\"")
        # 两端顶点阶段都必须传递 position、UV 和 color。
        self.assertIn("v_color = a_color", gl_vertex)
        self.assertIn("o.color = input.color", d3d11_vertex)
        # 两端片元阶段必须执行 sample RGB 与 tint RGB 的乘法。
        self.assertIn("sample_color.rgb * v_color.rgb", gl_fragment)
        self.assertIn("sample.rgb * tint.rgb", d3d11_fragment)
        # 两端片元阶段必须执行 sample alpha 与 tint alpha 的乘法。
        self.assertIn("sample_color.a * v_color.a", gl_fragment)
        self.assertIn("sample.a * tint.a", d3d11_fragment)
        # 单位域已由共享 FramePlan 验证，Adapter 不得再私自 clamp/saturate。
        self.assertNotIn("clamp", gl_fragment)
        self.assertNotIn("saturate", gl_fragment)
        self.assertNotIn("clamp", d3d11_fragment)
        self.assertNotIn("saturate", d3d11_fragment)
        # D3D11 必须直接消费已验证的单位域颜色。
        self.assertIn("float4 tint = input.color", d3d11_fragment)

    # 普通与 Additive textured 必须只在共享 blend 语义上分叉并复用同一 shader。
    def test_textured_variants_share_sampling_and_split_only_blend(self) -> None:
        # 读取共享 pipeline 契约源码。
        pipeline = PIPELINE.read_text(encoding="utf-8")
        # 读取 OpenGL pipeline 的 shader 选择源码。
        opengl_pipeline = OPENGL_PIPELINE.read_text(encoding="utf-8")
        # 读取 D3D11 的 draw 分派源码。
        d3d11_draw = D3D11_DRAW.read_text(encoding="utf-8")
        # 截取普通 TexturedQuad 契约。
        regular = source_range(pipeline, "Self::TexturedQuad => ui_2d_pipeline_contract(", "Self::GradientRect => ui_2d_pipeline_contract(")
        # 截取 Additive TexturedQuad 契约。
        additive = source_range(pipeline, "Self::TexturedQuadAdditive => ui_2d_pipeline_contract(", "Self::BlurPass => ui_2d_pipeline_contract(")
        # 截取 OpenGL 把两个共享身份映射到同一 shader 的分支。
        opengl_variants = source_range(opengl_pipeline, "PipelineKind::TexturedQuad | PipelineKind::TexturedQuadAdditive => {", "PipelineKind::GradientRect => {")
        # 截取 D3D11 把两个共享身份映射到同一 draw helper 的分支。
        d3d11_variants = source_range(d3d11_draw, "PipelineKind::TexturedQuad | PipelineKind::TexturedQuadAdditive => {", "PipelineKind::GlyphCoverageQuad => {")
        # 两个变体必须使用相同顶点布局。
        self.assertIn("PipelineVertexLayout::PositionUvColorF32", regular)
        self.assertIn("PipelineVertexLayout::PositionUvColorF32", additive)
        # 两个变体必须使用相同 sampled uniform 和纹理语义。
        self.assertIn("PipelineUniformLayout::Sampled", regular)
        self.assertIn("PipelineUniformLayout::Sampled", additive)
        self.assertIn("PipelineSampling::PremultipliedColor", regular)
        self.assertIn("PipelineSampling::PremultipliedColor", additive)
        # 唯一差异必须是普通 SrcOver 与 Additive blend。
        self.assertIn("PipelineBlend::PremultipliedAlpha", regular)
        self.assertIn("PipelineBlend::Additive", additive)
        # OpenGL 两个变体必须选择完全相同的 textured shader 对。
        self.assertIn("rhi_shaders::TEXTURED_VERTEX, rhi_shaders::TEXTURED_FRAGMENT", opengl_variants)
        # D3D11 两个变体必须进入同一个 sampled draw helper。
        self.assertIn("self.pipeline.draw_rhi_textured_quad(", d3d11_variants)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行当前文件定义的测试。
    unittest.main()
