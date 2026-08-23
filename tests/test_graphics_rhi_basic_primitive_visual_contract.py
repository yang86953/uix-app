# -*- coding: utf-8 -*-
# 说明本文件锁定 SolidMesh 与 R8 GlyphCoverageQuad 的双 Adapter 视觉契约。
"""Keep basic solid and glyph coverage primitives visually equivalent."""

# 引入标准单元测试框架。
import unittest
# 引入有限值检查，模拟共享值域拒绝非有限输入。
import math
# 引入路径解析工具。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 pipeline 契约。
PIPELINE = ROOT / "src/platform/presentation/rhi/pipeline.rs"
# 定位共享基础图元 ABI。
PRIMITIVE = ROOT / "src/platform/presentation/rhi/primitive.rs"
# 定位 Drawing 到共享 uniform 的唯一 lowering。
UNIFORM = ROOT / "src/draw/backend/rhi_renderer_uniform.rs"
# 定位 coverage 资源与顶点 lowering。
COVERAGE_RENDERER = ROOT / "src/draw/backend/rhi_renderer_coverage.rs"
# 定位 OpenGL ES shader 与 draw 消费边界。
OPENGL_SHADERS = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs"
# 定位 OpenGL ES 固定状态与 pipeline 分支。
OPENGL_DRAW = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 shader 常量与 pipeline 分支。
D3D11_PIPELINE = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/mod.rs"
# 定位 D3D11 shader helper 与 draw 分支。
D3D11_TEXTURED = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/rhi_textured.rs"
# 定位 D3D11 资源/契约消费边界。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_draw.rs"


# 截取两个稳定声明之间的源码，避免其它 pipeline 偶然满足断言。
def source_range(source: str, declaration: str, next_declaration: str) -> str:
    # 找到当前声明的起点。
    start = source.index(declaration)
    # 找到下一个声明的起点。
    end = source.index(next_declaration, start)
    # 返回当前声明拥有的源码范围。
    return source[start:end]


# 截取 D3D11 的指定 HLSL 常量。
def hlsl_range(source: str, declaration: str, next_declaration: str) -> str:
    # 找到 HLSL 常量声明起点。
    start = source.index(declaration)
    # 找到下一个 HLSL 常量声明起点。
    end = source.index(next_declaration, start)
    # 返回单一 shader 常量范围。
    return source[start:end]


# 对 shader 使用的八位量化公式提供独立的小型参考实现。
def quantize_coverage(rgba: tuple[float, float, float, float], coverage: float) -> tuple[int, int, int, int]:
    # 共享值域要求 coverage 必须是有限单位值。
    if not math.isfinite(coverage) or not 0.0 <= coverage <= 1.0:
        # 适配器不再替共享契约修正越界 coverage。
        raise ValueError("coverage is outside the shared unit domain")
    # 共享值域要求每个颜色通道必须是有限单位值。
    if any(not math.isfinite(channel) or not 0.0 <= channel <= 1.0 for channel in rgba):
        # 适配器不再替共享契约修正越界颜色。
        raise ValueError("rgba is outside the shared unit domain")
    # 直接按受保护的 coverage 值量化字节。
    coverage_byte = int(coverage * 255.0 + 0.5)
    # 直接按受保护的颜色值量化字节。
    color = [int(channel * 255.0 + 0.5) for channel in rgba]
    # 先用量化后的 alpha 对颜色做 premultiply。
    premul = [int(channel * color[3] / 255.0) for channel in color[:3]]
    # 最后把 coverage 同时应用到 premultiplied RGB 与 alpha。
    return tuple([int(channel * coverage_byte / 255.0) for channel in premul] + [int(color[3] * coverage_byte / 255.0)])


# 集中验证基础图元的共享 ABI、shader 与固定输出语义。
class GraphicsRhiBasicPrimitiveVisualContractTests(unittest.TestCase):
    # SolidMesh 必须由一个 Mesh ABI lowering 被两个 Adapter 的同名分支消费。
    def test_solid_mesh_shared_abi_and_straight_alpha_contract(self) -> None:
        # 读取共享 pipeline 契约源码。
        pipeline = PIPELINE.read_text(encoding="utf-8")
        # 读取共享 Mesh ABI 源码。
        primitive = PRIMITIVE.read_text(encoding="utf-8")
        # 读取 renderer lowering 源码。
        uniform = UNIFORM.read_text(encoding="utf-8")
        # 读取 OpenGL shader 源码。
        opengl_shader = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 OpenGL draw 分支源码。
        opengl_draw = OPENGL_DRAW.read_text(encoding="utf-8")
        # 读取 D3D11 pipeline shader 源码。
        d3d11_pipeline = D3D11_PIPELINE.read_text(encoding="utf-8")
        # 读取 D3D11 draw 分支源码。
        d3d11_draw = D3D11_DRAW.read_text(encoding="utf-8")
        # 截取 SolidMesh 的 OpenGL 分支。
        opengl = source_range(opengl_draw, "PipelineKind::SolidMesh => {", "PipelineKind::TexturedQuad | PipelineKind::TexturedQuadAdditive => {")
        # 截取 SolidMesh 的 D3D11 分支。
        d3d11 = source_range(d3d11_draw, "PipelineKind::SolidMesh => {", "PipelineKind::TexturedQuad | PipelineKind::TexturedQuadAdditive => {")
        # 截取 OpenGL Solid vertex shader，避免其它 position shader 偶然满足投影断言。
        solid_vertex = source_range(opengl_shader, "pub(super) const SOLID_VERTEX", "pub(super) const SOLID_FRAGMENT")
        # 截取 D3D11 Mesh HLSL 常量。
        mesh_hlsl = hlsl_range(d3d11_pipeline, "const MESH_HLSL: &str = r#\"", "const BLUR_HLSL: &str = r#\"")
        # SolidMesh 必须固定为 32 字节 Mesh ABI。
        self.assertIn("pub(crate) const MESH_UNIFORM_BYTES: usize = 32;", primitive)
        # viewport 必须位于 Mesh ABI 的前两个 float。
        self.assertIn("pub(crate) const MESH_VIEWPORT_FLOAT_OFFSET: usize = 0;", primitive)
        # color 必须位于 Mesh ABI 的第二个 float4。
        self.assertIn("pub(crate) const MESH_COLOR_FLOAT_OFFSET: usize = 4;", primitive)
        # renderer 必须通过共享 Mesh value object 完成唯一 lowering。
        self.assertIn("RhiMeshRasterParams::new(viewport, rgba)", uniform)
        # 两个 Adapter 必须都选择 SolidMesh 分支。
        self.assertIn("PipelineKind::SolidMesh =>", opengl)
        self.assertIn("PipelineKind::SolidMesh =>", d3d11)
        # 两端必须读取相同 viewport 与颜色字段。
        self.assertIn("MESH_VIEWPORT_FLOAT_OFFSET", opengl)
        self.assertIn("MESH_COLOR_FLOAT_OFFSET", opengl)
        self.assertIn("&uniform_native,", d3d11)
        # 截取 OpenGL Solid fragment shader，避免其它 fragment 偶然满足直出断言。
        solid_fragment = source_range(opengl_shader, "pub(super) const SOLID_FRAGMENT", "pub(super) const TEXTURED_VERTEX")
        # 两端位置投影都必须使用 viewport 归一化到 NDC 的同一公式。
        self.assertIn("(a_pos / u_viewport) * 2.0 - 1.0", solid_vertex)
        self.assertIn("(input.pos / u_viewport) * 2.0 - 1.0", mesh_hlsl)
        # 两端必须保留平台坐标系所需的机械 Y 映射，而不改变 X/viewport 公式。
        self.assertIn("ndc.y *= u_target_y_sign", solid_vertex)
        self.assertIn("ndc.y = -ndc.y", mesh_hlsl)
        # Solid shader 必须直出 straight-alpha 颜色，不得自行 premultiply 或量化。
        self.assertIn("fragColor = u_color", solid_fragment)
        self.assertIn("return u_color", mesh_hlsl)
        # Solid pipeline 必须使用共享 StraightAlpha blend 语义。
        solid_contract = source_range(pipeline, "Self::SolidMesh => ui_2d_pipeline_contract(", "Self::TexturedQuad => ui_2d_pipeline_contract(")
        self.assertIn("PipelineBlend::StraightAlpha", solid_contract)
        # 两端 Solid 分支不得引入 coverage 采样或 premultiply 逻辑。
        self.assertNotIn("sampled_format", opengl)
        self.assertNotIn("premul", mesh_hlsl)
        # Solid 顶点阶段不得引入逐顶点颜色插值，颜色只能来自共享 uniform。
        self.assertNotIn("v_color", solid_vertex)
        self.assertNotIn("COLOR0", mesh_hlsl)
        # Solid 片元阶段不得私自量化或采样颜色。
        self.assertNotIn("floor(", solid_fragment)
        self.assertNotIn("texture(", solid_fragment)

    # GlyphCoverageQuad 必须锁定 R8、最近点、量化与 premultiply 顺序。
    def test_glyph_coverage_shared_sampling_and_shader_order(self) -> None:
        # 读取共享 pipeline 契约源码。
        pipeline = PIPELINE.read_text(encoding="utf-8")
        # 读取共享 sampled/coverage ABI 源码。
        primitive = PRIMITIVE.read_text(encoding="utf-8")
        # 读取 renderer coverage lowering 源码。
        coverage_renderer = COVERAGE_RENDERER.read_text(encoding="utf-8")
        # 读取 OpenGL shader 与 draw 源码。
        opengl_shader = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 OpenGL coverage 分支源码。
        opengl_draw = OPENGL_DRAW.read_text(encoding="utf-8")
        # 读取 D3D11 shader 与 draw helper 源码。
        d3d11_pipeline = D3D11_PIPELINE.read_text(encoding="utf-8")
        # 读取 D3D11 coverage helper 源码。
        d3d11_textured = D3D11_TEXTURED.read_text(encoding="utf-8")
        # 读取 D3D11 coverage 分支源码。
        d3d11_draw = D3D11_DRAW.read_text(encoding="utf-8")
        # 截取 OpenGL coverage 分支。
        opengl = source_range(opengl_draw, "PipelineKind::GlyphCoverageQuad => {", "PipelineKind::MsdfGlyphQuad => {")
        # 截取 D3D11 coverage 分支。
        d3d11 = source_range(d3d11_draw, "PipelineKind::GlyphCoverageQuad => {", "PipelineKind::MsdfGlyphQuad => {")
        # 截取 OpenGL coverage fragment shader。
        opengl_fragment = source_range(opengl_shader, "pub(super) const COVERAGE_FRAGMENT", "pub(super) const MSDF_FRAGMENT")
        # 截取 D3D11 glyph shader。
        glyph_hlsl = hlsl_range(d3d11_pipeline, "const GLYPH_HLSL: &str = r#\"", "const RHI_TEXTURED_PS_HLSL: &str = r#\"")
        # coverage 必须复用含 viewport 与可选 surface 圆角槽位的 sampled ABI。
        self.assertIn("pub(crate) const SAMPLED_UNIFORM_BYTES: usize = 32;", primitive)
        self.assertIn("pub(crate) const SAMPLED_VIEWPORT_FLOAT_OFFSET: usize = 0;", primitive)
        # coverage renderer 必须使用共享 sampled uniform 与 R8 texture 描述。
        self.assertIn("RhiRenderer::sampled_uniform(viewport)", coverage_renderer)
        self.assertIn("TextureFormat::R8Unorm", coverage_renderer)
        # coverage renderer 必须选择 nearest sampler。
        self.assertIn("SamplerDesc::nearest_clamp()", coverage_renderer)
        # 共享 PipelineSampling 必须要求 R8 与非线性过滤。
        coverage_contract = source_range(pipeline, "Self::GlyphCoverageQuad => ui_2d_pipeline_contract(", "Self::ShapeRect => ui_2d_pipeline_contract(")
        self.assertIn("PipelineSampling::Coverage", coverage_contract)
        self.assertIn("PipelineBlend::PremultipliedAlpha", coverage_contract)
        self.assertIn("Self::Coverage => matches!(format, TextureFormat::R8Unorm)", pipeline)
        self.assertIn("Self::Coverage => !sampler.uses_linear_filter()", pipeline)
        # 两端 coverage 分支必须实际消费同一个 GlyphCoverageQuad 身份。
        self.assertIn("PipelineKind::GlyphCoverageQuad =>", opengl)
        self.assertIn("PipelineKind::GlyphCoverageQuad =>", d3d11)
        self.assertIn("draw_rhi_coverage_quad(", d3d11_textured)
        # 两端 shader 必须采样单通道 coverage 值。
        self.assertIn("texture(u_tex, v_uv).r", opengl_fragment)
        self.assertIn("Texture2D<float> u_atlas", glyph_hlsl)
        self.assertIn("u_atlas.Sample(u_samp, input.uv)", glyph_hlsl)
        # 两端必须直接量化共享契约保护的 coverage 输入。
        self.assertIn("floor(texture(u_tex, v_uv).r * 255.0 + 0.5)", opengl_fragment)
        # D3D11 必须使用同一直接 coverage 量化公式。
        self.assertIn("floor(u_atlas.Sample(u_samp, input.uv) * 255.0 + 0.5)", glyph_hlsl)
        # 两端必须直接量化共享契约保护的 tint 输入。
        self.assertIn("floor(v_color * 255.0 + 0.5)", opengl_fragment)
        # D3D11 必须使用同一直接 tint 量化公式。
        self.assertIn("floor(input.color * 255.0 + 0.5)", glyph_hlsl)
        # OpenGL Adapter 不得重新引入 coverage 的私有归一化。
        self.assertNotIn("clamp(texture(u_tex, v_uv).r", opengl_fragment)
        # D3D11 Adapter 不得重新引入 coverage 的私有归一化。
        self.assertNotIn("saturate(u_atlas.Sample(u_samp, input.uv))", glyph_hlsl)
        # OpenGL Adapter 不得重新引入 tint 的私有归一化。
        self.assertNotIn("clamp(v_color", opengl_fragment)
        # D3D11 Adapter 不得重新引入 tint 的私有归一化。
        self.assertNotIn("saturate(input.color)", glyph_hlsl)
        # 两端必须先按量化 alpha premultiply，再乘 coverage。
        self.assertIn("floor(premul * coverage / 255.0)", opengl_fragment)
        self.assertIn("floor(premul * coverage / 255.0)", glyph_hlsl)
        self.assertIn("floor(color.a * coverage / 255.0)", opengl_fragment)
        self.assertIn("floor(color.a * coverage / 255.0)", glyph_hlsl)
        # OpenGL 必须先量化颜色、再预乘 RGB、最后应用 coverage。
        self.assertLess(opengl_fragment.index("vec4 color = floor("), opengl_fragment.index("vec3 premul = floor("))
        self.assertLess(opengl_fragment.index("vec3 premul = floor("), opengl_fragment.index("vec3 rgb = floor("))
        # D3D11 必须保持与 OpenGL 相同的预乘和 coverage 顺序。
        self.assertLess(glyph_hlsl.index("float4 color = floor("), glyph_hlsl.index("float3 premul = floor("))
        self.assertLess(glyph_hlsl.index("float3 premul = floor("), glyph_hlsl.index("float3 rgb = floor("))
        # 两端不得回退到 straight-alpha 颜色直接乘 coverage。
        self.assertNotIn("vec3 rgb = floor(color.rgb * coverage", opengl_fragment)
        self.assertNotIn("float3 rgb = floor(color.rgb * coverage", glyph_hlsl)

    # 共享参考函数必须锁定 coverage 的关键边界结果。
    def test_coverage_reference_bytes(self) -> None:
        # 完全透明颜色在任意 coverage 下都必须输出零 alpha。
        self.assertEqual(quantize_coverage((1.0, 0.5, 0.0, 0.0), 1.0), (0, 0, 0, 0))
        # 零 coverage 必须完全丢弃 premultiplied RGB 与 alpha。
        self.assertEqual(quantize_coverage((1.0, 0.5, 0.25, 1.0), 0.0), (0, 0, 0, 0))
        # 满 coverage 必须返回量化后的 premultiplied 颜色。
        self.assertEqual(quantize_coverage((1.0, 0.5, 0.25, 1.0), 1.0), (255, 128, 64, 255))
        # 半 coverage 与半 alpha 必须体现先 alpha premultiply 再 coverage 的顺序。
        self.assertEqual(quantize_coverage((1.0, 0.5, 0.25, 0.5), 0.5), (64, 32, 16, 64))
        # 越界 coverage 必须由共享参考边界拒绝。
        with self.assertRaises(ValueError):
            # 负 coverage 不得再由 Adapter 风格的隐式归一化修正。
            quantize_coverage((1.0, 0.5, 0.25, 0.5), -0.1)
        # 非有限 coverage 必须由共享参考边界拒绝。
        with self.assertRaises(ValueError):
            # NaN coverage 不得进入字节量化。
            quantize_coverage((1.0, 0.5, 0.25, 0.5), float("nan"))
        # 越界 tint 必须由共享参考边界拒绝。
        with self.assertRaises(ValueError):
            # 超出单位域的颜色通道不得由 Adapter 修正。
            quantize_coverage((1.1, 0.5, 0.25, 0.5), 1.0)
        # 非有限 tint 必须由共享参考边界拒绝。
        with self.assertRaises(ValueError):
            # 无穷颜色通道不得进入字节量化。
            quantize_coverage((float("inf"), 0.5, 0.25, 0.5), 1.0)


# 支持直接执行本文件定义的精确契约测试。
if __name__ == "__main__":
    # 运行当前文件中的全部测试。
    unittest.main()
