# -*- coding: utf-8 -*-
# 说明本文件锁定 Gradient radial outer radius 的共享值域与双 Adapter 映射。
"""Keep radial gradient radius semantics identical across graphics adapters."""

# 引入最小单元测试框架。
import unittest
# 引入路径组合能力。
from pathlib import Path


# 定位当前仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 Gradient 值对象。
GRADIENT = ROOT / "src/platform/presentation/rhi/gradient.rs"
# 定位 FramePlan Uniform 载荷门禁。
UPLOAD = ROOT / "src/draw/backend/frame_plan_upload.rs"
# 定位 FramePlan 顶层验证。
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan.rs"
# 定位 OpenGL ES shader 源。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs"
# 定位 D3D11 shader 源。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/mod.rs"


# 截取两个声明之间的唯一源码范围。
def source_range(source: str, start_marker: str, end_marker: str) -> str:
    # 定位当前契约的起点。
    start = source.index(start_marker)
    # 定位下一个声明的起点。
    end = source.index(end_marker, start)
    # 返回不混入其它契约的局部源码。
    return source[start:end]


# 验证共享 Gradient 值域与两端 shader 的机械映射。
class GraphicsRhiGradientRadiusContractTests(unittest.TestCase):
    # 共享值对象必须拥有 finite、mode 和 radial outer radius 门禁。
    def test_shared_gradient_gate_owns_radial_domain(self) -> None:
        # 读取共享 Gradient 实现。
        gradient = GRADIENT.read_text(encoding="utf-8")
        # 截取唯一共享值域验证函数。
        gate = source_range(gradient, "pub(crate) fn is_valid(&self) -> bool", "// 把共享 Gradient ABI")
        # 所有字段必须先通过有限值门禁。
        self.assertIn("self.values.iter().all(|value| value.is_finite())", gate)
        # 模式必须只接受精确的线性零和径向一。
        self.assertIn("GRADIENT_PARAMS_FLOAT_OFFSET", gate)
        self.assertIn("mode != 0.0 && mode != 1.0", gate)
        # 径向外半径必须来自共享参数偏移并严格为正。
        self.assertIn("GRADIENT_PARAMS_FLOAT_OFFSET + 2", gate)
        self.assertIn("outer_radius > 0.0", gate)

    # FrameUniformPayload 与 FramePlan 必须委托同一个共享值域门禁。
    def test_frame_plan_delegates_gradient_gate(self) -> None:
        # 读取 Uniform 载荷门禁。
        upload = UPLOAD.read_text(encoding="utf-8")
        # 截取 Uniform 载荷的共享验证函数。
        uniform_gate = source_range(upload, "pub(crate) fn is_valid(self) -> bool", "// 返回共享 PipelineContract")
        # Gradient 变体必须委托值对象验证，而不是只检查 finite。
        self.assertIn("Self::Gradient(value) => value.is_valid()", uniform_gate)
        # 读取 FramePlan 顶层验证。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 截取 UploadUniform 的局部验证范围。
        plan_gate = source_range(frame_plan, "// 验证类型化 Uniform 属于共享 FramePlan 值域。", "// 检查纹理复制区域。")
        # FramePlan 必须调用新的统一 is_valid 门禁。
        self.assertIn("if !data.is_valid()", plan_gate)
        # 失败必须保持 InvalidArgument 分类。
        self.assertIn("Errc::InvalidArgument", plan_gate)
        # 诊断必须表达共享值域违例。
        self.assertIn("outside the shared value domain", plan_gate)

    # OpenGL 与 D3D11 必须直接消费同一正 outer radius 并保持 radial 公式一致。
    def test_radial_shaders_share_direct_outer_radius_semantics(self) -> None:
        # 读取 OpenGL shader 源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 读取 D3D11 shader 源码。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 截取 OpenGL radial fragment，排除 Shape 等后续 shader。
        gl_radial = source_range(opengl, "pub(super) const GRADIENT_FRAGMENT", "// 圆角矩形")
        # 截取 D3D11 Gradient shader，排除 Mesh 等后续 shader。
        d3d11_gradient = source_range(d3d11, "const GRADIENT_HLSL", "const MESH_HLSL")
        # 两端都必须直接读取共享 outer radius。
        self.assertIn("float outer_radius = u_params.z;", gl_radial)
        self.assertNotIn("float outer_radius = max(u_params.z", gl_radial)
        self.assertIn("float outer_r = u_params.z;", d3d11_gradient)
        # 两端必须使用相同的 outside discard 条件。
        self.assertIn("if (distance_to_center > outer_radius)", gl_radial)
        self.assertIn("if (dist > outer_r)", d3d11_gradient)
        # 两端必须使用相同的正数下限 range 公式。
        self.assertIn("max(outer_radius - inner_radius, 1e-6)", gl_radial)
        self.assertIn("max(outer_r - inner_r, 1e-6)", d3d11_gradient)
        # 两端必须使用相同的 inner/outer 插值并裁剪到单位区间。
        self.assertIn("clamp((distance_to_center - inner_radius) / range, 0.0, 1.0)", gl_radial)
        self.assertIn("saturate((dist - inner_r) / range)", d3d11_gradient)


# 允许直接运行本文件完成静态契约回归。
if __name__ == "__main__":
    # 使用 unittest 默认发现与报告机制。
    unittest.main()
