# -*- coding: utf-8 -*-
# 验证颜色写掩码只由共享 RHI 拥有，两个 Adapter 只能机械翻译。
"""Keep color write masks identical across the OpenGL and D3D11 adapters."""

# 启用延迟注解解析，保持测试与其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 pipeline 状态。
SHARED = ROOT / "src/platform/presentation/rhi/pipeline.rs"
# 定位 OpenGL Adapter 输出状态翻译。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 Adapter 输出状态翻译。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/pipeline.rs"


# 集中验证共享写掩码所有权和两个原生翻译入口。
class GraphicsRhiColorWriteMaskContractTests(unittest.TestCase):
    # 两个 Adapter 必须读取同一份类型化颜色写掩码。
    def test_color_write_mask_has_one_shared_owner(self) -> None:
        # 读取共享 pipeline 状态源码。
        shared = SHARED.read_text(encoding="utf-8")
        # 读取 OpenGL 输出状态翻译源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 读取 D3D11 输出状态翻译源码。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 共享层必须定义封闭的颜色写掩码类型。
        self.assertIn("pub(crate) enum PipelineColorWriteMask", shared)
        # 完整 pipeline 状态必须拥有该写掩码。
        self.assertIn("pub(crate) write_mask: PipelineColorWriteMask", shared)
        # 每个现有 blend 语义都必须声明完整 RGBA 写入。
        self.assertEqual(shared.count("write_mask: PipelineColorWriteMask::All"), 4)
        # OpenGL 必须通过穷尽函数翻译共享写掩码。
        self.assertIn("fn gl_color_write_mask(mask: PipelineColorWriteMask)", opengl)
        # OpenGL 每次应用 pipeline 状态都必须显式恢复 color mask。
        self.assertIn("gl.color_mask(write_red, write_green, write_blue, write_alpha)", opengl)
        # OpenGL 写掩码必须来自共享状态。
        self.assertIn("gl_color_write_mask(state.write_mask)", opengl)
        # D3D11 必须通过穷尽函数翻译共享写掩码。
        self.assertIn("fn d3d11_color_write_mask(mask: PipelineColorWriteMask)", d3d11)
        # D3D11 render target 写掩码必须来自共享状态。
        self.assertIn("RenderTargetWriteMask: d3d11_color_write_mask(state.write_mask)", d3d11)
        # D3D11 描述不得继续在构造点私自写死全部通道。
        self.assertNotIn("RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL", d3d11)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
