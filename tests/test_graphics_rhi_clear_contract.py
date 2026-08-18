# -*- coding: utf-8 -*-
# 验证颜色清理输出状态在共享 RHI 和两个 Adapter 中保持一致。
"""Keep color clear output state identical across native graphics adapters."""

# 启用延迟注解解析，保持测试与其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享颜色与清理输出契约。
SHARED = ROOT / "src/native/present/rhi/color.rs"
# 定位 OpenGL 整目标清理入口。
OPENGL_FULL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"
# 定位 OpenGL 局部清理与状态翻译。
OPENGL_RECT = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_clear.rs"
# 定位 D3D11 整目标清理入口。
D3D11_FULL = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs"
# 定位 D3D11 局部清理与契约验收。
D3D11_RECT = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_clear.rs"


# 集中验证整目标与局部颜色清理的共享输出状态。
class GraphicsRhiClearContractTests(unittest.TestCase):
    # 共享颜色模块必须拥有唯一类型化清理输出契约。
    def test_shared_contract_owns_clear_output_state(self) -> None:
        # 读取共享颜色契约源码。
        shared = SHARED.read_text(encoding="utf-8")
        # 共享层必须定义封闭颜色清理契约。
        self.assertIn("pub(crate) struct RhiColorClearContract", shared)
        # 契约必须拥有颜色写掩码。
        self.assertIn("pub(crate) write_mask: PipelineColorWriteMask", shared)
        # 契约必须拥有颜色抖动状态。
        self.assertIn("pub(crate) dither: PipelineDitherState", shared)
        # 唯一清理常量必须完整覆盖 RGBA。
        self.assertIn("write_mask: PipelineColorWriteMask::All", shared)
        # 唯一清理常量必须关闭颜色抖动。
        self.assertIn("dither: PipelineDitherState::Disabled", shared)

    # OpenGL 的整目标与局部清理必须逐项恢复共享状态。
    def test_opengl_maps_contract_for_full_and_rect_clear(self) -> None:
        # 读取 OpenGL 整目标清理源码。
        full = OPENGL_FULL.read_text(encoding="utf-8")
        # 读取 OpenGL 局部清理与状态翻译源码。
        rect = OPENGL_RECT.read_text(encoding="utf-8")
        # 整目标清理必须消费共享清理常量。
        self.assertIn("clear::apply_color_clear_contract(gl, UIX_COLOR_CLEAR_CONTRACT)", full)
        # 整目标清理必须覆盖陈旧 scissor。
        self.assertIn("gl.disable(glow::SCISSOR_TEST)", full)
        # 局部清理必须消费同一共享清理常量。
        self.assertIn("apply_color_clear_contract(gl, UIX_COLOR_CLEAR_CONTRACT)", rect)
        # OpenGL 必须穷尽映射全通道写掩码。
        self.assertIn("PipelineColorWriteMask::All => gl.color_mask(true, true, true, true)", rect)
        # OpenGL 必须穷尽映射禁用抖动状态。
        self.assertIn("PipelineDitherState::Disabled => gl.disable(glow::DITHER)", rect)
        # 局部清理必须只编码自身显式 scissor。
        self.assertIn("self.apply_scissor(gl, Some(scissor))?", rect)
        # 清理不得保存或恢复前一条 Draw 的裁剪历史。
        self.assertNotIn("previous", rect)

    # D3D11 的两个原生清理入口必须验收同一共享状态。
    def test_d3d11_accepts_contract_for_full_and_rect_clear(self) -> None:
        # 读取 D3D11 整目标清理源码。
        full = D3D11_FULL.read_text(encoding="utf-8")
        # 读取 D3D11 局部清理与验收源码。
        rect = D3D11_RECT.read_text(encoding="utf-8")
        # 整目标清理必须先验收共享清理常量。
        self.assertIn("validate_color_clear_contract(UIX_COLOR_CLEAR_CONTRACT)?", full)
        # 契约拒绝必须发生在共享 pass 建立之前，避免留下半提交状态。
        self.assertLess(
            full.index("validate_color_clear_contract(UIX_COLOR_CLEAR_CONTRACT)?"),
            full.index("self.rhi_device.pass.begin"),
        )
        # 整目标必须继续使用 D3D11 的完整 view 清理原语。
        self.assertIn("ClearRenderTargetView", full)
        # 局部清理必须验收同一共享清理常量。
        self.assertIn("validate_color_clear_contract(UIX_COLOR_CLEAR_CONTRACT)?", rect)
        # D3D11 只能接受共享层当前唯一清理状态。
        self.assertIn("if !contract.is_uix_contract()", rect)
        # 局部清理必须继续使用显式矩形 ClearView。
        self.assertIn("context.ClearView", rect)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
