# -*- coding: utf-8 -*-
# 说明本文件只锁定 Sector 的共享 ABI 与两个原生 shader 的视觉同义性。
"""Keep the D3D11 and OpenGL ES Sector shaders visually equivalent."""

# 引入标准单元测试框架。
import unittest
# 引入路径解析工具。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 Sector ABI 定义。
PRIMITIVE = ROOT / "src/native/present/rhi/primitive.rs"
# 定位 Drawing 到共享 ABI 的唯一 renderer lowering。
UNIFORM = ROOT / "src/draw/backend/rhi_renderer_uniform.rs"
# 定位 OpenGL ES shader 源码。
OPENGL_SHADERS = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs"
# 定位 D3D11 shader 源码。
D3D11_SHADERS = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/rhi_sector.rs"
# 定位 OpenGL draw 的共享字段消费边界。
OPENGL_DRAW = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 draw 的共享字段消费边界。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs"


# 截取两个稳定声明之间的源码，避免其它分支的相同片段满足断言。
def source_range(source: str, declaration: str, next_declaration: str) -> str:
    # 找到当前范围的声明起点。
    start = source.index(declaration)
    # 找到下一个范围的声明起点。
    end = source.index(next_declaration, start)
    # 返回当前声明拥有的精确源码范围。
    return source[start:end]


# 截取 D3D11 Sector shader 的完整 HLSL 常量范围。
def sector_hlsl(source: str) -> str:
    # 找到 Sector HLSL 常量的声明起点。
    start = source.index('pub(crate) const SECTOR_HLSL: &str = r#"')
    # 找到 shader 编译函数的声明起点。
    end = source.index('// 编译 SectorHLSL 的 VS/PS', start)
    # 返回只包含 Sector HLSL 的源码。
    return source[start:end]


# 集中验证共享 Sector ABI 与两套原生视觉实现。
class GraphicsRhiSectorVisualContractTests(unittest.TestCase):
    # 共享值对象必须唯一拥有 ABI，renderer 与两个 Adapter 只能消费同一字段身份。
    def test_shared_abi_has_one_lowering_and_two_typed_consumers(self) -> None:
        # 读取共享 ABI 源码。
        primitive = PRIMITIVE.read_text(encoding="utf-8")
        # 读取 renderer lowering 源码。
        uniform = UNIFORM.read_text(encoding="utf-8")
        # 读取 OpenGL draw 的完整源码。
        opengl_source = OPENGL_DRAW.read_text(encoding="utf-8")
        # 读取 D3D11 draw 的完整源码。
        d3d11_source = D3D11_DRAW.read_text(encoding="utf-8")
        # 读取 D3D11 Sector shader 源码。
        d3d11_shader_source = D3D11_SHADERS.read_text(encoding="utf-8")
        # 只截取 OpenGL Sector 分支，禁止其它 pipeline 偶然满足字段断言。
        opengl = source_range(opengl_source, "PipelineKind::Sector => {", "PipelineKind::BoxShadow => {")
        # 只截取 D3D11 Sector 分支，禁止其它 pipeline 偶然满足 helper 断言。
        d3d11 = source_range(d3d11_source, "PipelineKind::Sector => {", "PipelineKind::BoxShadow => {")
        # 只截取 D3D11 Sector 常量，核对二进制 uniform 的原生字段解释。
        d3d11_shader = sector_hlsl(d3d11_shader_source)
        # Sector 必须固定为四个 float4。
        self.assertIn("pub(crate) const SECTOR_UNIFORM_BYTES: usize = 64;", primitive)
        # viewport 必须位于第一个 float4。
        self.assertIn("pub(crate) const SECTOR_VIEWPORT_FLOAT_OFFSET: usize = 0;", primitive)
        # rect 必须位于第二个 float4。
        self.assertIn("pub(crate) const SECTOR_RECT_FLOAT_OFFSET: usize = 4;", primitive)
        # color 必须位于第三个 float4。
        self.assertIn("pub(crate) const SECTOR_COLOR_FLOAT_OFFSET: usize = 8;", primitive)
        # angles 必须位于第四个 float4。
        self.assertIn("pub(crate) const SECTOR_ANGLES_FLOAT_OFFSET: usize = 12;", primitive)
        # renderer 必须通过共享 RhiSectorRasterParams 构造 ABI。
        self.assertIn("RhiSectorRasterParams::new(viewport, rect, rgba, angles)", uniform)
        # OpenGL 必须从共享 offset 读取 viewport。
        self.assertIn("read_f32(&uniform, SECTOR_VIEWPORT_FLOAT_OFFSET)", opengl)
        # OpenGL 必须从共享 offset 读取 rect、color 与 angles。
        self.assertIn("read_vec4(&uniform, SECTOR_RECT_FLOAT_OFFSET)", opengl)
        self.assertIn("read_vec4(&uniform, SECTOR_COLOR_FLOAT_OFFSET)", opengl)
        self.assertIn("read_vec4(&uniform, SECTOR_ANGLES_FLOAT_OFFSET)", opengl)
        # D3D11 必须按同一 PipelineKind 将 uniform 原子交给 Sector helper。
        self.assertIn("PipelineKind::Sector =>", d3d11)
        # D3D11 Sector 分支必须只把共享 uniform buffer 交给原生 helper。
        self.assertIn("self.pipeline.draw_rhi_sector(", d3d11)
        # D3D11 helper 必须接收同一个完整 uniform buffer，不能另组角度载荷。
        self.assertIn("&uniform_native,", d3d11)
        # D3D11 shader 必须按共享四个 float4 的顺序解释完整二进制载荷。
        self.assertIn(
            "float2 u_viewport;\n    float2 _pad0;\n    float4 u_rect;\n    float4 u_color;\n    float4 u_angles;",
            d3d11_shader,
        )
        # D3D11 不得在 Adapter draw 分支重新解释角度字段。
        self.assertNotIn("SECTOR_ANGLES_FLOAT_OFFSET", d3d11)

    # 两套 Sector shader 必须使用同一单位矩形、圆半径、角度和抗锯齿语义。
    def test_shader_geometry_and_coverage_are_equivalent(self) -> None:
        # 读取 OpenGL shader 源码。
        opengl_source = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 D3D11 shader 源码。
        d3d11_source = D3D11_SHADERS.read_text(encoding="utf-8")
        # 截取 OpenGL Sector vertex shader。
        opengl_vertex = source_range(opengl_source, "pub(super) const SECTOR_VERTEX", "pub(super) const SECTOR_FRAGMENT")
        # 截取 OpenGL Sector fragment shader。
        opengl_fragment = source_range(opengl_source, "pub(super) const SECTOR_FRAGMENT", "pub(super) const SHADOW_VERTEX")
        # 截取 D3D11 Sector shader。
        d3d11_sector = sector_hlsl(d3d11_source)
        # 两端都必须从外接矩形左上角按单位 quad 插值。
        self.assertIn("u_rect.xy + a_pos * u_rect.zw", opengl_vertex)
        self.assertIn("u_rect.xy + input.pos * u_rect.zw", d3d11_sector)
        # 两端都必须把局部坐标归一化到以中心为原点的单位圆。
        self.assertIn("(v_local / max(v_rect_size, vec2(0.0001)) - 0.5) * 2.0", opengl_fragment)
        self.assertIn("(input.local / max(input.rect_size, float2(0.0001, 0.0001)) - 0.5) * 2.0", d3d11_sector)
        # 两端都必须以单位圆半径计算径向 coverage。
        self.assertIn("float radius = length(unit);", opengl_fragment)
        self.assertIn("float radius = length(unit);", d3d11_sector)
        # 两端必须用同一导数宽度形成半像素径向过渡。
        self.assertIn("(1.0 - radius) / radial_width + 0.5", opengl_fragment)
        # D3D11 的 saturate 必须机械对应 OpenGL 的零到一 clamp。
        self.assertIn("(1.0 - radius) / radial_width + 0.5", d3d11_sector)
        # 两端都必须允许近似整圆时跳过角向裁剪。
        self.assertIn("u_angles.y < TAU - 0.0001 && radius > 0.0001", opengl_fragment)
        self.assertIn("u_angles.y < TAU - 0.0001 && radius > 0.0001", d3d11_sector)
        # 两端都必须从相同坐标分量计算正 X 轴参考角。
        self.assertIn("atan(unit.y, unit.x)", opengl_fragment)
        # D3D11 的 atan2 参数顺序必须与 OpenGL 的双参数 atan 一致。
        self.assertIn("atan2(unit.y, unit.x)", d3d11_sector)
        # 两端都必须把负角归一化到同一个正向圆周。
        self.assertIn("if (angle < 0.0)", opengl_fragment)
        # D3D11 必须保留同一负角归一化门禁。
        self.assertIn("if (angle < 0.0)", d3d11_sector)
        # 两端都必须用相同常量完成一次圆周归一化。
        self.assertIn("angle += TAU;", opengl_fragment)
        # D3D11 必须保留同一个圆周增量。
        self.assertIn("angle += TAU;", d3d11_sector)
        # 两端都必须从正 X 轴读取起始角并按正向 sweep 计算 delta。
        self.assertIn("mod(angle - u_angles.x + TAU, TAU)", opengl_fragment)
        self.assertIn("fmod(angle - u_angles.x + TAU, TAU)", d3d11_sector)
        # 两端必须从 sweep 两侧的最短距离形成角向边缘。
        self.assertIn("min(delta, u_angles.y - delta)", opengl_fragment)
        # D3D11 必须使用同一角向边缘距离。
        self.assertIn("min(delta, u_angles.y - delta)", d3d11_sector)
        # 两端都必须对径向和角向边界使用导数宽度抗锯齿。
        self.assertIn("fwidth(radius)", opengl_fragment)
        self.assertIn("fwidth(radius)", d3d11_sector)
        self.assertIn("fwidth(angle)", opengl_fragment)
        self.assertIn("fwidth(angle)", d3d11_sector)
        # 两端都必须把 sweep 外部明确归零，不能只依赖边缘距离的符号。
        self.assertIn("if (delta > u_angles.y)", opengl_fragment)
        # D3D11 必须使用相同的 sweep 外部门禁。
        self.assertIn("if (delta > u_angles.y)", d3d11_sector)
        # 两端都必须相乘径向与角向覆盖率。
        self.assertIn("float mask = radial_mask * angular_mask;", opengl_fragment)
        # D3D11 必须保持同一 coverage 合成顺序。
        self.assertIn("float mask = radial_mask * angular_mask;", d3d11_sector)
        # 两端都必须对 mask 为零的片元执行外部丢弃。
        self.assertIn("if (mask <= 0.0)", opengl_fragment)
        self.assertIn("if (mask <= 0.0)", d3d11_sector)
        # 防止任一端退化为没有 AA 的硬圆边界。
        self.assertNotIn("radius <= 1.0 ? 1.0 : 0.0", opengl_fragment)
        self.assertNotIn("radius <= 1.0 ? 1.0 : 0.0", d3d11_sector)

    # 两端必须先量化 straight-alpha 输入，再预乘并应用 coverage mask。
    def test_shader_alpha_quantization_and_mask_order_are_equivalent(self) -> None:
        # 读取 OpenGL shader 源码。
        opengl_source = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 D3D11 shader 源码。
        d3d11_source = D3D11_SHADERS.read_text(encoding="utf-8")
        # 截取 OpenGL Sector fragment shader。
        opengl_fragment = source_range(opengl_source, "pub(super) const SECTOR_FRAGMENT", "pub(super) const SHADOW_VERTEX")
        # 截取 D3D11 Sector shader。
        d3d11_sector = sector_hlsl(d3d11_source)
        # 两端都必须将 straight-alpha 输入量化到八位域。
        self.assertIn("floor(clamp(u_color, 0.0, 1.0) * 255.0 + 0.5)", opengl_fragment)
        self.assertIn("floor(saturate(u_color) * 255.0 + 0.5)", d3d11_sector)
        # 两端都必须先按八位 alpha 预乘颜色。
        self.assertIn("floor(color.rgb * color.a / 255.0)", opengl_fragment)
        self.assertIn("floor(color.rgb * color.a / 255.0)", d3d11_sector)
        # 两端都必须在预乘颜色和 alpha 上应用同一个 mask。
        self.assertIn("vec4(premul * mask, color.a * mask) / 255.0", opengl_fragment)
        self.assertIn("float4(premul * mask, color.a * mask) / 255.0", d3d11_sector)
        # 防止任一端回退到未量化的 straight-alpha 直出。
        self.assertNotIn("fragColor = vec4(u_color.rgb, u_color.a * mask)", opengl_fragment)
        self.assertNotIn("return float4(u_color.rgb, u_color.a * mask)", d3d11_sector)


# 支持直接执行本文件定义的精确契约测试。
if __name__ == "__main__":
    # 运行当前文件中的全部测试。
    unittest.main()
