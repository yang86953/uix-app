# -*- coding: utf-8 -*-
# 说明本文件锁定 Coverage/MSDF 输入值域由共享契约拥有。
"""Keep sampled glyph value-domain ownership in FramePlan and PipelineSampling."""

# 引入标准单元测试框架。
import unittest
# 引入路径解析工具。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享顶点值域门禁。
VERTEX_UPLOAD = ROOT / "src/draw/backend/frame_plan_upload.rs"
# 定位共享 FramePlan 验证。
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan.rs"
# 定位共享采样格式与过滤契约。
PIPELINE = ROOT / "src/native/presentation/rhi/pipeline.rs"
# 定位 OpenGL shader 常量。
OPENGL_SHADERS = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs"
# 定位 OpenGL draw 的共享采样门禁消费点。
OPENGL_DRAW = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 coverage shader 常量。
D3D11_PIPELINE = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/mod.rs"
# 定位 D3D11 MSDF shader 常量。
D3D11_MSDF = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/msdf_shader.rs"
# 定位 D3D11 draw 的共享采样门禁消费点。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_draw.rs"


# 截取两个稳定声明之间的源码，避免其它代码偶然满足断言。
def source_range(source: str, declaration: str, next_declaration: str) -> str:
    # 找到当前声明的起点。
    start = source.index(declaration)
    # 找到下一个声明的起点。
    end = source.index(next_declaration, start)
    # 返回当前声明拥有的源码范围。
    return source[start:end]


# 集中验证共享 sampled 输入值域和两个 Adapter 的消费边界。
class GraphicsRhiSampledValueDomainContractTests(unittest.TestCase):
    # 两个 Adapter 必须消费共享 FramePlan 的单位 tint 与采样组合。
    def test_shared_gates_and_direct_shader_consumption(self) -> None:
        # 读取共享顶点门禁源码。
        vertex_upload = VERTEX_UPLOAD.read_text(encoding="utf-8")
        # 读取 FramePlan 验证源码。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 读取共享采样契约源码。
        pipeline = PIPELINE.read_text(encoding="utf-8")
        # 读取 OpenGL shader 源码。
        opengl_source = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 OpenGL draw 源码。
        opengl_draw_source = OPENGL_DRAW.read_text(encoding="utf-8")
        # 读取 D3D11 glyph shader 源码。
        d3d11_source = D3D11_PIPELINE.read_text(encoding="utf-8")
        # 读取 D3D11 MSDF shader 源码。
        d3d11_msdf_source = D3D11_MSDF.read_text(encoding="utf-8")
        # 读取 D3D11 draw 源码。
        d3d11_draw_source = D3D11_DRAW.read_text(encoding="utf-8")
        # 截取共享颜色值域分支。
        vertex_gate = source_range(vertex_upload, "Self::PositionUvColorF32(_) =>", "        }\n    }\n\n    // 返回编码")
        # 截取 FramePlan 顶点验证分支。
        plan_vertex_gate = source_range(frame_plan, "if let FramePlanCommand::UploadVertex", "                        // 验证类型化 Uniform")
        # 截取共享 Coverage/MSDF 采样实现。
        sampling_gate = source_range(pipeline, "impl PipelineSampling", "// 定义所有 Adapter")
        # 截取 OpenGL Coverage shader。
        gl_coverage = source_range(opengl_source, "pub(super) const COVERAGE_FRAGMENT", "pub(super) const MSDF_FRAGMENT")
        # 截取 OpenGL MSDF shader。
        gl_msdf = source_range(opengl_source, "pub(super) const MSDF_FRAGMENT", "pub(super) const SHAPE_VERTEX")
        # 截取 D3D11 Coverage shader。
        d3d_coverage = source_range(d3d11_source, "const GLYPH_HLSL: &str = r#\"", "const RHI_TEXTURED_PS_HLSL")
        # 截取 D3D11 MSDF shader。
        d3d_msdf = source_range(d3d11_msdf_source, "pub(crate) const MSDF_GLYPH_HLSL", "\n\"#;")
        # 截取 OpenGL Coverage draw 分支。
        gl_coverage_draw = source_range(opengl_draw_source, "PipelineKind::GlyphCoverageQuad => {", "PipelineKind::MsdfGlyphQuad => {")
        # 截取 OpenGL MSDF draw 分支。
        gl_msdf_draw = source_range(opengl_draw_source, "PipelineKind::MsdfGlyphQuad => {", "PipelineKind::ShapeRect | PipelineKind::ShapeRectAdditive => {")
        # 截取 D3D11 Coverage draw 分支。
        d3d_coverage_draw = source_range(d3d11_draw_source, "PipelineKind::GlyphCoverageQuad => {", "PipelineKind::MsdfGlyphQuad => {")
        # 截取 D3D11 MSDF draw 分支。
        d3d_msdf_draw = source_range(d3d11_draw_source, "PipelineKind::MsdfGlyphQuad => {", "PipelineKind::ShapeRect | PipelineKind::ShapeRectAdditive => {")
        # 共享门禁必须检查 float8 颜色字段的单位上下界。
        self.assertIn("vertex[4..8]", vertex_gate)
        # 共享门禁必须同时检查颜色字段的闭区间上下界。
        self.assertIn("*color >= 0.0 && *color <= 1.0", vertex_gate)
        # FramePlan 必须在 Adapter 前调用顶点值域验证。
        self.assertIn("if !data.is_valid()", plan_vertex_gate)
        # Coverage 必须固定 R8Unorm 与最近点过滤。
        self.assertIn("Self::Coverage => matches!(format, TextureFormat::R8Unorm)", sampling_gate)
        # Coverage 必须通过封闭枚举只接受最近点过滤。
        self.assertIn("matches!(sampler.filter(), SamplerFilter::Nearest)", sampling_gate)
        # MSDF 必须固定 Rgba8Unorm 与线性过滤。
        self.assertIn("Self::Msdf => matches!(format, TextureFormat::Rgba8Unorm)", sampling_gate)
        # MSDF 必须与颜色采样共同通过封闭枚举只接受线性过滤。
        self.assertIn("matches!(sampler.filter(), SamplerFilter::Linear)", sampling_gate)
        # OpenGL 共享 helper 必须取得与 pipeline 匹配的绑定。
        self.assertIn("packet.sampling()", opengl_draw_source)
        # OpenGL Coverage draw 必须把当前 packet 的条件采样角色交给共享 helper。
        self.assertIn("bind_sampled(gl, self, program, packet.sampling())", gl_coverage_draw)
        # OpenGL MSDF draw 必须复用同一共享 helper。
        self.assertIn("bind_sampled(gl, self, program, packet.sampling())", gl_msdf_draw)
        # D3D11 Coverage draw 必须通过共享 helper 取得当前 packet 的完整绑定。
        self.assertIn("packet_sampled_binding(self, packet)?", d3d_coverage_draw)
        # D3D11 MSDF draw 复用同一共享绑定门禁。
        self.assertIn("packet_sampled_binding(self, packet)?", d3d_msdf_draw)
        # OpenGL Coverage 必须直接量化已验证的 sample 与 tint。
        self.assertIn("floor(texture(u_tex, v_uv).r * 255.0 + 0.5)", gl_coverage)
        # OpenGL Coverage 必须直接量化已验证的 tint。
        self.assertIn("floor(v_color * 255.0 + 0.5)", gl_coverage)
        # OpenGL Coverage 不得恢复 sample clamp。
        self.assertNotIn("clamp(texture(u_tex, v_uv).r", gl_coverage)
        # OpenGL Coverage 不得恢复 tint clamp。
        self.assertNotIn("clamp(v_color", gl_coverage)
        # D3D11 Coverage 必须直接量化已验证的 sample 与 tint。
        self.assertIn("floor(u_atlas.Sample(u_samp, input.uv) * 255.0 + 0.5)", d3d_coverage)
        # D3D11 Coverage 必须直接量化已验证的 tint。
        self.assertIn("floor(input.color * 255.0 + 0.5)", d3d_coverage)
        # D3D11 Coverage 不得恢复 sample saturate。
        self.assertNotIn("saturate(u_atlas.Sample(u_samp, input.uv))", d3d_coverage)
        # D3D11 Coverage 不得恢复 tint saturate。
        self.assertNotIn("saturate(input.color)", d3d_coverage)
        # OpenGL MSDF 必须直接消费 encoded，并保留 tint 的共享值域。
        self.assertIn("vec3 encoded = texture(u_tex, v_uv).rgb;", gl_msdf)
        # OpenGL MSDF 必须直接量化已验证的 tint。
        self.assertIn("floor(v_color * 255.0 + 0.5)", gl_msdf)
        # OpenGL MSDF 不得恢复 tint clamp。
        self.assertNotIn("clamp(v_color", gl_msdf)
        # D3D11 MSDF 必须直接消费 encoded 与已验证 tint。
        self.assertIn("float3 encoded = u_atlas.Sample(u_samp, input.uv).rgb;", d3d_msdf)
        # D3D11 MSDF 必须直接量化已验证的 tint。
        self.assertIn("float4 color = floor(input.color * 255.0 + 0.5);", d3d_msdf)
        # D3D11 MSDF 不得恢复 encoded sample saturate。
        self.assertNotIn("saturate(u_atlas.Sample(u_samp, input.uv).rgb)", d3d_msdf)
        # D3D11 MSDF 不得恢复 tint saturate。
        self.assertNotIn("saturate(input.color)", d3d_msdf)

    # 解析 coverage clamp、稳定保护和量化顺序必须仍由两端保留。
    def test_msdf_algorithmic_clamp_and_quantization_remain(self) -> None:
        # 读取 OpenGL shader 源码。
        opengl_source = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 D3D11 MSDF shader 源码。
        d3d11_msdf_source = D3D11_MSDF.read_text(encoding="utf-8")
        # 截取 OpenGL MSDF shader。
        gl_msdf = source_range(opengl_source, "pub(super) const MSDF_FRAGMENT", "pub(super) const SHAPE_VERTEX")
        # 截取 D3D11 MSDF shader。
        d3d_msdf = source_range(d3d11_msdf_source, "pub(crate) const MSDF_GLYPH_HLSL", "\n\"#;")
        # 两端必须保留解析 coverage 的最终单位域保护。
        self.assertIn("clamp(0.5 - signed_distance * screen_pixel_range, 0.0, 1.0)", gl_msdf)
        # D3D11 必须保留同义的解析 coverage 单位域保护。
        self.assertIn("saturate(0.5 - signed_distance * screen_pixel_range)", d3d_msdf)
        # 两端必须保留导数、纹理尺寸和屏幕像素范围的稳定保护。
        self.assertIn("max(fwidth(v_uv), vec2(1e-6))", gl_msdf)
        # OpenGL 必须保留纹理尺寸最小值保护。
        self.assertIn("max(u_tex_size, vec2(1.0))", gl_msdf)
        # D3D11 必须保留显式导数计算。
        self.assertIn("abs(ddx(input.uv)) + abs(ddy(input.uv))", d3d_msdf)
        # D3D11 必须保留导数倒数的最小分母保护。
        self.assertIn("max(uv_derivative, float2(0.000001, 0.000001))", d3d_msdf)
        # D3D11 必须保留纹理尺寸最小值保护。
        self.assertIn("max(u_tex_size, float2(1.0, 1.0))", d3d_msdf)
        # 两端必须保留 coverage 量化、premultiply 和 coverage 应用顺序。
        self.assertIn("floor(coverage * 255.0 + 0.5)", gl_msdf)
        # D3D11 必须保留相同的 coverage 字节量化。
        self.assertIn("floor(coverage * 255.0 + 0.5)", d3d_msdf)
        # OpenGL 必须先量化颜色再执行 premultiply。
        self.assertLess(gl_msdf.index("vec4 color = floor("), gl_msdf.index("vec3 premul = floor("))
        # OpenGL 必须先 premultiply 再应用 coverage。
        self.assertLess(gl_msdf.index("vec3 premul = floor("), gl_msdf.index("vec3 rgb = floor("))
        # D3D11 必须先量化颜色再执行 premultiply。
        self.assertLess(d3d_msdf.index("float4 color = floor("), d3d_msdf.index("float3 premul = floor("))
        # D3D11 必须先 premultiply 再应用 coverage。
        self.assertLess(d3d_msdf.index("float3 premul = floor("), d3d_msdf.index("float3 rgb = floor("))
        # 两端必须使用量化 coverage 同时作用于 RGB 与 alpha。
        self.assertIn("premul * coverage_byte / 255.0", gl_msdf)
        # D3D11 RGB 必须应用量化 coverage。
        self.assertIn("premul * coverage_byte / 255.0", d3d_msdf)
        # OpenGL alpha 必须应用量化 coverage。
        self.assertIn("color.a * coverage_byte / 255.0", gl_msdf)
        # D3D11 alpha 必须应用量化 coverage。
        self.assertIn("color.a * coverage_byte / 255.0", d3d_msdf)


# 支持直接执行本文件定义的精确契约测试。
if __name__ == "__main__":
    # 运行当前文件中的全部测试。
    unittest.main()
