# -*- coding: utf-8 -*-
# 说明本文件锁定 Gradient 两端 shader 的完整视觉公式同义性。
"""Keep OpenGL and D3D11 Gradient shader formulas equivalent."""

# 引入标准单元测试框架。
import unittest
# 引入路径解析工具。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 OpenGL Gradient shader 常量文件。
OPENGL_SHADERS = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs"
# 定位 D3D11 Gradient shader 常量文件。
D3D11_SHADERS = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/mod.rs"


# 从 Rust 常量声明中提取同一 raw string 的完整 payload。
def shader_payload(source: str, declaration: str) -> str:
    # 定位指定常量声明。
    declaration_index = source.index(declaration)
    # 定位该声明自己的 raw string 起始标记。
    raw_start = source.index('r#"', declaration_index) + len('r#"')
    # 定位该 raw string 自己的闭合标记。
    raw_end = source.index('"#;', raw_start)
    # 返回不包含其它 shader 的 payload。
    return source[raw_start:raw_end]


# 断言两端各自包含同一语义的对应公式。
def assert_formula_pairs(
    case: unittest.TestCase,
    opengl: str,
    d3d11: str,
    pairs: tuple[tuple[str, str], ...],
) -> None:
    # 逐组检查 GLSL/HLSL 的成对公式。
    for opengl_formula, d3d11_formula in pairs:
        # OpenGL 必须保留左侧公式。
        case.assertIn(opengl_formula, opengl)
        # D3D11 必须保留右侧等价公式。
        case.assertIn(d3d11_formula, d3d11)


# 集中验证 Gradient 的顶点、线性和径向视觉契约。
class GraphicsRhiGradientVisualContractTests(unittest.TestCase):
    # 顶点阶段必须使用同一 affine quad 与目标坐标语义。
    def test_gradient_vertex_projection_and_local_uv_are_equivalent(self) -> None:
        # 读取 OpenGL shader 源码。
        opengl_source = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 D3D11 shader 源码。
        d3d11_source = D3D11_SHADERS.read_text(encoding="utf-8")
        # 提取 OpenGL Gradient vertex payload。
        opengl = shader_payload(opengl_source, "pub(super) const GRADIENT_VERTEX")
        # 提取 D3D11 Gradient vertex/pixel payload。
        d3d11 = shader_payload(d3d11_source, "const GRADIENT_HLSL")
        # 锁定 origin、edge_x、edge_y 与 affine position 公式。
        assert_formula_pairs(self, opengl, d3d11, (
            ("vec2 origin = u_quad_origin_edge_x.xy;", "float2 origin = u_origin_edge_x.xy;"),
            ("vec2 edge_x = u_quad_origin_edge_x.zw;", "float2 edge_x = u_origin_edge_x.zw;"),
            ("vec2 edge_y = u_quad_edge_y.xy;", "float2 edge_y = u_edge_y.xy;"),
            ("vec2 pos = origin + a_pos.x * edge_x + a_pos.y * edge_y;", "float2 pos = origin + input.pos.x * edge_x + input.pos.y * edge_y;"),
            ("vec2 ndc = (pos / u_viewport) * 2.0 - 1.0;", "float2 ndc = (pos / u_viewport) * 2.0 - 1.0;"),
        ))
        # OpenGL 的目标 Y 方向必须来自共享 target_y_sign。
        self.assertIn("ndc.y *= u_target_y_sign;", opengl)
        # D3D11 使用固定的等价 -Y 投影。
        self.assertIn("ndc.y = -ndc.y;", d3d11)
        # OpenGL 必须把 NDC 写入 gl_Position。
        self.assertIn("gl_Position = vec4(ndc, 0.0, 1.0);", opengl)
        # D3D11 必须把 NDC 写入 SV_POSITION 输出。
        self.assertIn("o.pos = float4(ndc, 0.0, 1.0);", d3d11)
        # OpenGL local UV 必须直接传递单位 quad 坐标。
        self.assertIn("v_gradient_uv = a_pos;", opengl)
        # D3D11 local UV 必须直接传递单位 quad 坐标。
        self.assertIn("o.local = input.pos;", d3d11)

    # 线性模式必须锁定四个方向与 straight-alpha 插值输出。
    def test_gradient_linear_directions_and_alpha_output_are_equivalent(self) -> None:
        # 读取 OpenGL shader 源码。
        opengl_source = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 D3D11 shader 源码。
        d3d11_source = D3D11_SHADERS.read_text(encoding="utf-8")
        # 提取 OpenGL Gradient fragment payload。
        opengl = shader_payload(opengl_source, "pub(super) const GRADIENT_FRAGMENT")
        # 提取 D3D11 Gradient payload。
        d3d11 = shader_payload(d3d11_source, "const GRADIENT_HLSL")
        # 截取 OpenGL linear 分支，排除 radial 分支的同名输出。
        opengl_linear = opengl[:opengl.index("vec2 center = vec2(0.5);")]
        # 截取 D3D11 linear 分支，排除 radial 分支的同名输出。
        d3d11_linear = d3d11[:d3d11.index("float2 center = float2(0.5, 0.5);")]
        # 锁定 linear mode 门禁与方向参数读取。
        assert_formula_pairs(self, opengl_linear, d3d11_linear, (
            ("if (mode < 0.5)", "if (mode < 0.5)"),
            ("float direction = u_params.y;", "float dir = u_params.y;"),
        ))
        # 锁定四个方向的区间分支。
        assert_formula_pairs(self, opengl_linear, d3d11_linear, (
            ("if (direction < 0.5)", "if (dir < 0.5)"),
            ("else if (direction < 1.5)", "else if (dir < 1.5)"),
            ("else if (direction < 2.5)", "else if (dir < 2.5)"),
            ("else\n            t =", "else\n            t ="),
        ))
        # 锁定 x、y 和正对角线 t 公式。
        assert_formula_pairs(self, opengl_linear, d3d11_linear, (
            ("t = v_gradient_uv.x;", "t = local.x;"),
            ("t = v_gradient_uv.y;", "t = local.y;"),
            ("(v_gradient_uv.x * u_params.z + v_gradient_uv.y * u_params.w)", "(local.x * size.x + local.y * size.y)"),
        ))
        # 锁定反对角线 t 公式及同一 size 权重。
        assert_formula_pairs(self, opengl_linear, d3d11_linear, (
            ("(v_gradient_uv.x * u_params.z - v_gradient_uv.y * u_params.w + u_params.w)", "(local.x * size.x - local.y * size.y + size.y)"),
            ("max(u_params.z + u_params.w, 1e-6)", "max(size.x + size.y, 1e-6)"),
        ))
        # 线性两个对角方向都必须保留 OpenGL 尺寸归一化分母。
        self.assertEqual(opengl_linear.count("/ max(u_params.z + u_params.w, 1e-6);"), 2)
        # 线性两个对角方向都必须保留 D3D11 尺寸归一化分母。
        self.assertEqual(d3d11_linear.count("/ max(size.x + size.y, 1e-6);"), 2)
        # OpenGL 必须把线性 t 限制到单位区间并执行 straight-alpha mix。
        self.assertIn("mix(u_color_a, u_color_b, clamp(t, 0.0, 1.0))", opengl_linear)
        # D3D11 必须执行等价 saturate 与 straight-alpha lerp。
        self.assertIn("t = saturate(t);", d3d11_linear)
        self.assertIn("return lerp(u_color_a, u_color_b, t);", d3d11_linear)

    # 径向模式必须锁定中心、半径门禁、range 与 straight-alpha 输出。
    def test_gradient_radial_formula_and_alpha_output_are_equivalent(self) -> None:
        # 读取 OpenGL shader 源码。
        opengl_source = OPENGL_SHADERS.read_text(encoding="utf-8")
        # 读取 D3D11 shader 源码。
        d3d11_source = D3D11_SHADERS.read_text(encoding="utf-8")
        # 提取 OpenGL Gradient fragment payload。
        opengl = shader_payload(opengl_source, "pub(super) const GRADIENT_FRAGMENT")
        # 提取 D3D11 Gradient payload。
        d3d11 = shader_payload(d3d11_source, "const GRADIENT_HLSL")
        # 截取 OpenGL radial 分支，排除 linear 分支的同名输出。
        opengl_radial = opengl[opengl.index("vec2 center = vec2(0.5);") :]
        # 截取 D3D11 radial 分支，排除 linear 分支的同名输出。
        d3d11_radial = d3d11[d3d11.index("float2 center = float2(0.5, 0.5);") :]
        # 锁定 radial 分支和同一局部中心。
        assert_formula_pairs(self, opengl_radial, d3d11_radial, (
            ("vec2 center = vec2(0.5);", "float2 center = float2(0.5, 0.5);"),
            ("float distance_to_center = distance(v_gradient_uv, center);", "float dist = length(input.local - center);"),
        ))
        # 锁定 outer/inner radius 直接来自共享 params 字段。
        assert_formula_pairs(self, opengl_radial, d3d11_radial, (
            ("float outer_radius = u_params.z;", "float outer_r = u_params.z;"),
            ("float inner_radius = u_params.y;", "float inner_r = u_params.y;"),
        ))
        # 锁定外部 discard 条件。
        assert_formula_pairs(self, opengl_radial, d3d11_radial, (
            ("if (distance_to_center > outer_radius)", "if (dist > outer_r)"),
            ("discard;", "discard;"),
        ))
        # 锁定 range 下限与单位区间 t 公式。
        assert_formula_pairs(self, opengl_radial, d3d11_radial, (
            ("float range = max(outer_radius - inner_radius, 1e-6);", "float range = max(outer_r - inner_r, 1e-6);"),
            ("float t = clamp((distance_to_center - inner_radius) / range, 0.0, 1.0);", "float t = saturate((dist - inner_r) / range);"),
        ))
        # OpenGL 必须使用 radial straight-alpha mix 输出。
        self.assertIn("vec4 color = mix(u_color_a, u_color_b, t);", opengl_radial)
        self.assertIn("fragColor = color;", opengl_radial)
        # D3D11 必须使用 radial straight-alpha lerp 输出。
        self.assertIn("return lerp(u_color_a, u_color_b, t);", d3d11_radial)


# 支持直接运行本文件中的精确契约测试。
if __name__ == "__main__":
    # 运行当前文件中的全部测试。
    unittest.main()
