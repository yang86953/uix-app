# -*- coding: utf-8 -*-
# 验证颜色抖动语义在共享 Drawing 契约和两个 Device Adapter 中保持一致。
"""Keep color dither state identical across native graphics adapters."""

# 启用延迟注解解析，保持测试与其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 pipeline 契约。
PIPELINE = ROOT / "src/native/present/rhi/pipeline.rs"
# 定位 OpenGL draw 状态翻译。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 公共状态映射。
D3D11_MODULE = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/mod.rs"
# 定位 D3D11 draw 分派边界。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs"


# 集中验证共享颜色抖动语义和两个 Device Adapter 的状态翻译。
class GraphicsRhiDitherContractTests(unittest.TestCase):
    # PipelineContract 必须拥有唯一类型化颜色抖动语义。
    def test_pipeline_contract_owns_disabled_dither_state(self) -> None:
        # 读取共享 pipeline 契约。
        pipeline = PIPELINE.read_text(encoding="utf-8")
        # 共享层必须定义封闭颜色抖动类型。
        self.assertIn("pub(crate) enum PipelineDitherState", pipeline)
        # 当前封闭集合必须只接受禁用语义。
        self.assertIn("PipelineDitherState {\n    // 禁止原生 API", pipeline)
        # 完整 pipeline 契约必须拥有颜色抖动状态。
        self.assertIn("pub(crate) dither: PipelineDitherState", pipeline)
        # 公共二维构造边界必须冻结 Disabled。
        self.assertIn("dither: PipelineDitherState::Disabled", pipeline)

    # OpenGL draw 必须覆盖 OpenGL ES 默认启用的颜色抖动状态。
    def test_opengl_explicitly_disables_dither(self) -> None:
        # 读取 OpenGL draw 状态翻译源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # OpenGL draw 必须从当前 pipeline 契约恢复颜色抖动状态。
        self.assertIn("apply_pipeline_dither(gl, contract.dither)", opengl)
        # OpenGL 必须穷尽匹配 Disabled。
        self.assertIn("PipelineDitherState::Disabled => gl.disable(glow::DITHER)", opengl)
        # 颜色抖动只能由统一固定状态入口管理一次。
        self.assertEqual(opengl.count("glow::DITHER"), 1)

    # D3D11 必须显式验收其固有输出行为与共享禁用语义相容。
    def test_d3d11_accepts_only_disabled_dither(self) -> None:
        # 读取 D3D11 公共状态映射源码。
        module = D3D11_MODULE.read_text(encoding="utf-8")
        # 读取 D3D11 draw 分派源码。
        draw = D3D11_DRAW.read_text(encoding="utf-8")
        # D3D11 必须提供封闭的共享语义验收入口。
        self.assertIn("fn d3d11_supports_dither_state(state: PipelineDitherState) -> bool", module)
        # D3D11 当前只能接受 Disabled，未来新增变体必须显式处理。
        self.assertIn("matches!(state, PipelineDitherState::Disabled)", module)
        # 固定状态绑定必须拒绝 Adapter 无法表达的共享语义。
        self.assertIn("if !d3d11_supports_dither_state(dither)", module)
        # draw 分派必须把当前 pipeline 的共享颜色抖动状态交给统一入口。
        self.assertIn("contract.dither,", draw)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
