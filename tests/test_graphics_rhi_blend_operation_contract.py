# -*- coding: utf-8 -*-
# 验证混合运算只由共享 RHI 拥有，两个 Adapter 只能机械翻译。
"""Keep blend operations identical across the OpenGL and D3D11 adapters."""

# 启用延迟注解解析，保持测试与仓库其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 pipeline 契约。
SHARED = ROOT / "src/native/present/rhi/pipeline.rs"
# 定位 OpenGL Adapter 翻译。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 Adapter 翻译。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/pipeline.rs"


# 集中验证共享运算所有权和两个原生翻译入口。
class GraphicsRhiBlendOperationContractTests(unittest.TestCase):
    # 两个 Adapter 必须读取同一份 RGB 与 alpha 运算。
    def test_blend_operations_have_one_shared_owner(self) -> None:
        # 读取共享 pipeline 契约源码。
        shared = SHARED.read_text(encoding="utf-8")
        # 读取 OpenGL blend state 翻译源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 读取 D3D11 blend state 翻译源码。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 共享层必须定义封闭的混合运算类型。
        self.assertIn("pub(crate) enum PipelineBlendOperation", shared)
        # 共享状态必须分别拥有 RGB 运算。
        self.assertIn("pub(crate) color_operation: PipelineBlendOperation", shared)
        # 共享状态必须分别拥有 alpha 运算。
        self.assertIn("pub(crate) alpha_operation: PipelineBlendOperation", shared)
        # OpenGL 必须通过穷尽函数翻译共享运算。
        self.assertIn("fn gl_blend_operation(operation: PipelineBlendOperation)", opengl)
        # OpenGL 必须显式恢复 RGB 与 alpha equation。
        self.assertIn("gl.blend_equation_separate(", opengl)
        # OpenGL 的 RGB equation 必须来自共享状态。
        self.assertIn("gl_blend_operation(state.color_operation)", opengl)
        # OpenGL 的 alpha equation 必须来自共享状态。
        self.assertIn("gl_blend_operation(state.alpha_operation)", opengl)
        # D3D11 必须通过穷尽函数翻译共享运算。
        self.assertIn("fn d3d11_blend_operation(operation: PipelineBlendOperation)", d3d11)
        # D3D11 的 RGB operation 必须来自共享状态。
        self.assertIn("BlendOp: d3d11_blend_operation(state.color_operation)", d3d11)
        # D3D11 的 alpha operation 必须来自共享状态。
        self.assertIn("BlendOpAlpha: d3d11_blend_operation(state.alpha_operation)", d3d11)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
