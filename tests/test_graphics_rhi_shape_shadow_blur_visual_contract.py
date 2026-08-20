# -*- coding: utf-8 -*-
# 说明本文件只锁定 Shape、Shadow 与 Blur 的双 Adapter shader 视觉公式。
"""Keep Shape, Shadow, and Blur shader formulas equivalent across adapters."""

# 引入标准单元测试框架。
import unittest
# 引入路径解析工具。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 OpenGL shader 常量所在文件。
OPENGL_SHADERS = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs"
# 定位 D3D11 shader 常量所在文件。
D3D11_SHADERS = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/mod.rs"


# 从 Rust raw string 自身边界提取唯一 shader 常量内容。
def shader_constant(source: str, declaration: str) -> str:
    # 定位当前 shader 常量声明的起点。
    declaration_index = source.index(declaration)
    # 跳过 Rust raw string 的开始标记，只保留 shader payload。
    payload_start = source.index('r#"', declaration_index) + len('r#"')
    # 使用同一 raw string 的闭合标记冻结 payload 终点。
    payload_end = source.index('"#;', payload_start)
    # 返回不包含相邻注释或其它 shader 的精确内容。
    return source[payload_start:payload_end]


# 断言同一语义在两个语言实现中各有对应公式。
def assert_formula_pairs(
    case: unittest.TestCase,
    opengl: str,
    d3d11: str,
    pairs: tuple[tuple[str, str], ...],
) -> None:
    # 逐组检查 GLSL/HLSL 的语义对应表达式。
    for opengl_formula, d3d11_formula in pairs:
        # OpenGL 必须保留该语义公式。
        case.assertIn(opengl_formula, opengl)
        # D3D11 必须保留对应的等价公式。
        case.assertIn(d3d11_formula, d3d11)


# 集中验证三组固定 shader 的跨 Adapter 视觉同义性。
class GraphicsRhiShapeShadowBlurVisualContractTests(unittest.TestCase):
    # Shape 的 SDF、描边 coverage 和 premultiply 必须成对一致。
    def test_shape_sdf_coverage_and_output_are_equivalent(self) -> None:
        # 读取 OpenGL shader 源码。
        opengl_source = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 D3D11 shader 源码。
        d3d11_source = D3D11_SHADERS.read_text(encoding="utf-8")
        # 截取 OpenGL Shape fragment 常量。
        opengl = shader_constant(opengl_source, "pub(super) const SHAPE_FRAGMENT")
        # 截取 D3D11 Shape Rect 常量。
        d3d11 = shader_constant(d3d11_source, "const RECT_HLSL")
        # 锁定 per-corner rounded-rect SDF 的中心、半尺寸和距离公式。
        assert_formula_pairs(self, opengl, d3d11, (
            ("vec2 half_size = size * 0.5;", "float2 half_size = size * 0.5;"),
            ("vec2 q = local - half_size;", "float2 q = local - half_size;"),
            ("vec2 d = abs(q) - half_size + corner_radius;", "float2 d = abs(q) - half_size + cr;"),
            ("return outside + inside - corner_radius;", "return outside + inside - cr;"),
        ))
        # 锁定四角半径按象限选择，而不是由 Adapter 重新解释。
        self.assertIn("corner_radius = (q.y < 0.0) ? radius.x : radius.w;", opengl)
        # 锁定 OpenGL 右上和右下象限半径选择。
        self.assertIn("corner_radius = (q.y < 0.0) ? radius.y : radius.z;", opengl)
        # 锁定 D3D11 的同一四角半径象限选择。
        self.assertIn("cr = (q.y < 0.0) ? radius.x : radius.w;", d3d11)
        # 锁定 D3D11 右上和右下象限半径选择。
        self.assertIn("cr = (q.y < 0.0) ? radius.y : radius.z;", d3d11)
        # 锁定 OpenGL 描边 outer/inner 的同心局部偏移。
        self.assertIn("vec2 outer_local = v_local - vec2(u_stroke.z);", opengl)
        # 锁定 OpenGL 描边 inner 的同心局部偏移。
        self.assertIn("vec2 inner_local = v_local - vec2(u_stroke.w);", opengl)
        # 锁定 D3D11 描边 outer/inner 的同心局部偏移。
        self.assertIn("float2 outer_local = input.local - float2(u_stroke.z, u_stroke.z);", d3d11)
        # 锁定 D3D11 描边 inner 的同心局部偏移。
        self.assertIn("float2 inner_local = input.local - float2(u_stroke.w, u_stroke.w);", d3d11)
        # 锁定 outer/inner 描边尺寸和半径的同心 lowering。
        assert_formula_pairs(self, opengl, d3d11, (
            ("vec2 outer_size = v_rect_size + 2.0 * half_stroke;", "float2 outer_size = input.rect_size + 2.0 * h;"),
            ("vec4 outer_radius = u_radius + half_stroke;", "float4 outer_rad = u_radius + h;"),
            ("vec2 inner_size = max(v_rect_size - 2.0 * half_stroke, 0.0);", "float2 inner_size = max(input.rect_size - 2.0 * h, 0.0);"),
            ("vec4 inner_radius = max(u_radius - half_stroke, 0.0);", "float4 inner_rad = max(u_radius - h, 0.0);"),
        ))
        # 锁定描边双 coverage 的乘积。
        assert_formula_pairs(self, opengl, d3d11, (
            ("mask = clamp(0.5 - outer_sd, 0.0, 1.0) * clamp(0.5 + inner_sd, 0.0, 1.0);", "mask = saturate(0.5 - outer_sd) * saturate(0.5 + inner_sd);"),
            ("mask = clamp(0.5 - outer_sd, 0.0, 1.0);", "mask = saturate(0.5 - outer_sd);"),
            ("? clamp(0.5 - rounded_rect_sdf(v_local, v_rect_size, u_radius), 0.0, 1.0)", "? saturate(0.5 - rounded_rect_sdf(input.local, input.rect_size, u_radius))"),
        ))
        # 锁定两端 outer/inner coverage 的 SDF 来源调用。
        self.assertIn("float outer_sd = rounded_rect_sdf(outer_local, outer_size, outer_radius);", opengl)
        # 锁定 OpenGL inner coverage 使用独立同心 SDF。
        self.assertIn("float inner_sd = rounded_rect_sdf(inner_local, inner_size, inner_radius);", opengl)
        # 锁定 D3D11 outer coverage 使用共享 SDF。
        self.assertIn("float outer_sd = rounded_rect_sdf(outer_local, outer_size, outer_rad);", d3d11)
        # 锁定 D3D11 inner coverage 使用独立同心 SDF。
        self.assertIn("float inner_sd = rounded_rect_sdf(inner_local, inner_size, inner_rad);", d3d11)
        # 形状外部必须丢弃，避免产生零 alpha 的额外混合片元。
        self.assertIn("if (mask <= 0.0)\n        discard;", opengl)
        # D3D11 必须保留同一 mask discard 门禁。
        self.assertIn("if (mask <= 0.0)\n        discard;", d3d11)
        # 锁定 straight-alpha 八位量化、premultiply 与 coverage 输出顺序。
        assert_formula_pairs(self, opengl, d3d11, (
            ("vec4 color = floor(clamp(u_color, 0.0, 1.0) * 255.0 + 0.5);", "float4 color = floor(saturate(u_color) * 255.0 + 0.5);"),
            ("vec3 premul = floor(color.rgb * color.a / 255.0);", "float3 premul = floor(color.rgb * color.a / 255.0);"),
            ("fragColor = vec4(premul * mask, color.a * mask) / 255.0;", "return float4(premul * mask, color.a * mask) / 255.0;"),
        ))

    # Shadow 的 SDF、普通/ambient 曲线和 straight-alpha 输出必须成对一致。
    def test_shadow_sdf_curves_and_output_are_equivalent(self) -> None:
        # 读取 OpenGL shader 源码。
        opengl_source = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 D3D11 shader 源码。
        d3d11_source = D3D11_SHADERS.read_text(encoding="utf-8")
        # 截取 OpenGL Shadow fragment 常量。
        opengl = shader_constant(opengl_source, "pub(super) const SHADOW_FRAGMENT")
        # 截取 D3D11 Shadow 常量。
        d3d11 = shader_constant(d3d11_source, "const SHADOW_HLSL")
        # 锁定 Shadow rounded-rect SDF 的核心公式。
        assert_formula_pairs(self, opengl, d3d11, (
            ("vec2 half_size = size * 0.5;", "float2 half_size = size * 0.5;"),
            ("vec2 d = abs(q) - half_size + corner_radius;", "float2 d = abs(q) - half_size + cr;"),
            ("return outside + inside - corner_radius;", "return outside + inside - cr;"),
        ))
        # 锁定 OpenGL Shadow 四角半径的左侧象限选择。
        self.assertIn("corner_radius = (q.y < 0.0) ? radius.x : radius.w;", opengl)
        # 锁定 OpenGL Shadow 四角半径的右侧象限选择。
        self.assertIn("corner_radius = (q.y < 0.0) ? radius.y : radius.z;", opengl)
        # 锁定 D3D11 Shadow 四角半径的左侧象限选择。
        self.assertIn("cr = (q.y < 0.0) ? radius.x : radius.w;", d3d11)
        # 锁定 D3D11 Shadow 四角半径的右侧象限选择。
        self.assertIn("cr = (q.y < 0.0) ? radius.y : radius.z;", d3d11)
        # 锁定 OpenGL Shadow blur 半径和形状局部坐标。
        self.assertIn("float blur_radius = max(blur.x, blur.y);", opengl)
        # 锁定 OpenGL Shadow 主 SDF 输入。
        self.assertIn("vec2 shape_local = v_local - blur - vec2(1.0);", opengl)
        # 锁定 OpenGL Shadow 主 SDF 调用。
        self.assertIn("float signed_distance = rounded_rect_sdf(shape_local, v_rect_size, u_radius);", opengl)
        # 锁定 D3D11 Shadow blur 半径和形状局部坐标。
        self.assertIn("float blur_radius = max(blur.x, blur.y);", d3d11)
        # 锁定 D3D11 Shadow 主 SDF 输入。
        self.assertIn("float2 shape_local = input.local - blur - 1.0;", d3d11)
        # 锁定 D3D11 Shadow 主 SDF 调用。
        self.assertIn("float sd = rounded_rect_sdf(shape_local, input.rect_size, u_radius);", d3d11)
        # 锁定普通 smooth coverage 曲线。
        assert_formula_pairs(self, opengl, d3d11, (
            ("float t = clamp((blur - signed_distance) / (2.0 * blur), 0.0, 1.0);", "float t = saturate((blur - sd) / (2.0 * blur));"),
            ("return t * t * (3.0 - 2.0 * t);", "return t * t * (3.0 - 2.0 * t);"),
        ))
        # 锁定 OpenGL 普通和 ambient coverage 的实际调用。
        self.assertIn("? shadow_coverage_ambient(signed_distance, blur_radius)", opengl)
        self.assertIn(": shadow_coverage(signed_distance, blur_radius);", opengl)
        # 锁定 D3D11 普通和 ambient coverage 的实际调用。
        self.assertIn("coverage = shadow_coverage_ambient(sd, blur_radius);", d3d11)
        self.assertIn("coverage = shadow_coverage(sd, blur_radius);", d3d11)
        # 锁定 ambient 曲线的 half blur、四次方和系数。
        assert_formula_pairs(self, opengl, d3d11, (
            ("float half_blur = blur * 0.5;", "float halfb = blur * 0.5;"),
            ("float t = clamp((half_blur - signed_distance) / (blur + half_blur), 0.0, 1.0);", "float t = saturate((halfb - sd) / (blur + halfb));"),
            ("return squared * squared * (5.0 - 4.0 * t);", "return t2 * t2 * (5.0 - 4.0 * t);"),
        ))
        # 锁定 ambient 分支和小 blur fallback。
        assert_formula_pairs(self, opengl, d3d11, (
            ("if (blur_radius > 0.5)", "if (blur_radius > 0.5)"),
            ("coverage = u_size.z > 0.5", "if (u_size.z > 0.5)"),
            ("coverage = clamp(0.5 - signed_distance, 0.0, 1.0);", "coverage = saturate(0.5 - sd);"),
        ))
        # 阴影外部必须丢弃。
        self.assertIn("if (coverage <= 0.0)\n        discard;", opengl)
        # D3D11 必须保留同一 discard 条件。
        self.assertIn("if (coverage <= 0.0)\n        discard;", d3d11)
        # 锁定 straight-alpha 输出不提前 premultiply RGB。
        assert_formula_pairs(self, opengl, d3d11, (
            ("fragColor = vec4(u_color.rgb, u_color.a * coverage);", "return float4(u_color.rgb, u_color.a * coverage);"),
        ))

    # Blur 的区域 UV、tap 索引和加权采样必须成对一致。
    def test_blur_region_sampling_and_accumulation_are_equivalent(self) -> None:
        # 读取 OpenGL shader 源码。
        opengl_source = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 D3D11 shader 源码。
        d3d11_source = D3D11_SHADERS.read_text(encoding="utf-8")
        # 截取 OpenGL Blur 顶点常量。
        opengl_vertex = shader_constant(opengl_source, "pub(super) const BLUR_VERTEX")
        # 截取 OpenGL Blur fragment 常量。
        opengl_fragment = shader_constant(opengl_source, "pub(super) const BLUR_FRAGMENT")
        # 截取 D3D11 Blur 常量。
        d3d11 = shader_constant(d3d11_source, "const BLUR_HLSL")
        # 锁定 top-left 区域到 source texture 的 UV 映射。
        assert_formula_pairs(self, opengl_vertex, d3d11, (
            ("vec2 unit = vec2(a_pos.x * 0.5 + 0.5, 0.5 - a_pos.y * 0.5);", "float2 unit = float2(input.pos.x * 0.5 + 0.5, 0.5 - input.pos.y * 0.5);"),
            ("v_uv = (u_region.xy + unit * u_region.zw) / u_sizes.zw;", "o.uv = (u_region.xy + unit * u_region.zw) / u_sizes.zw;"),
        ))
        # 锁定方向步长和整数 tap radius。
        assert_formula_pairs(self, opengl_fragment, d3d11, (
            ("vec2 step_size = u_dir_taps.xy / u_sizes.zw;", "float2 step = u_dir_taps.xy / u_sizes.zw;"),
            ("int radius = int(u_dir_taps.z);", "int radius = (int)u_dir_taps.z;"),
        ))
        # 锁定 OpenGL 顶点的 Y 投影翻转。
        self.assertIn("gl_Position = vec4(a_pos.x, -a_pos.y, 0.0, 1.0);", opengl_vertex)
        # 锁定 D3D11 顶点直接写入 SV_POSITION 的 API 对应差异。
        self.assertIn("o.pos = float4(input.pos, 0.0, 1.0);", d3d11)
        # 锁定 OpenGL 的完整 64 次循环声明与 accumulator 初始化。
        self.assertIn("vec4 color = vec4(0.0);", opengl_fragment)
        self.assertIn("for (int index = 0; index < 64; ++index)", opengl_fragment)
        # 锁定 D3D11 的完整 64 次循环声明与 accumulator 初始化。
        self.assertIn("float4 color = 0;", d3d11)
        self.assertIn("for (int i = 0; i < 64; ++i)", d3d11)
        # 锁定 64 tap 上限与四槽数组索引。
        assert_formula_pairs(self, opengl_fragment, d3d11, (
            ("index < 64", "i < 64"),
            ("u_weights[index / 4][index % 4]", "u_weights[i / 4][i % 4]"),
        ))
        # 锁定零权重终止，防止未使用槽位改变结果。
        self.assertIn("if (weight <= 0.0)\n            break;", opengl_fragment)
        # D3D11 必须保留对应的零权重终止。
        self.assertIn("if (w <= 0.0) break;", d3d11)
        # 锁定以 radius 为中心的 offset 和 texture/Sample 加权累加。
        assert_formula_pairs(self, opengl_fragment, d3d11, (
            ("vec2 offset = step_size * float(index - radius);", "float2 off = step * (float)(i - radius);"),
            ("color += weight * texture(u_tex, v_uv + offset);", "color += w * u_tex.Sample(u_samp, input.uv + off);"),
            ("fragColor = color;", "return color;"),
        ))


# 支持直接运行本文件中的精确契约测试。
if __name__ == "__main__":
    # 运行当前文件中的全部测试。
    unittest.main()
