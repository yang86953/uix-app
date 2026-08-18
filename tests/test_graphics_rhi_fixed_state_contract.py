# -*- coding: utf-8 -*-
# 验证二维光栅与深度模板状态只由共享 RHI 拥有，两个 Adapter 只能机械翻译。
"""Keep fixed raster and depth-stencil state identical across native adapters."""

# 启用延迟注解解析，保持测试与其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 pipeline 契约。
SHARED = ROOT / "src/native/present/rhi/pipeline.rs"
# 定位 OpenGL Adapter 固定状态翻译。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 原生状态对象创建。
D3D11_PIPELINE = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/pipeline.rs"
# 定位 D3D11 固定状态统一绑定。
D3D11_MODULE = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/mod.rs"
# 定位 D3D11 draw 分派边界。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs"
# 定位不得继续私自绑定 rasterizer 的 shader helper 目录。
D3D11_HELPERS = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline"


# 集中验证共享固定状态所有权和两个原生翻译入口。
class GraphicsRhiFixedStateContractTests(unittest.TestCase):
    # 共享 PipelineContract 必须完整拥有二维固定状态。
    def test_pipeline_contract_has_one_fixed_state_owner(self) -> None:
        # 读取共享 pipeline 契约源码。
        shared = SHARED.read_text(encoding="utf-8")
        # 共享层必须定义封闭面剔除语义。
        self.assertIn("pub(crate) enum PipelineCullMode", shared)
        # 共享层必须定义封闭正面绕序语义。
        self.assertIn("pub(crate) enum PipelineFrontFace", shared)
        # 共享层必须定义封闭深度裁剪语义。
        self.assertIn("pub(crate) enum PipelineDepthClip", shared)
        # 共享层必须定义二维光栅状态值对象。
        self.assertIn("pub(crate) struct PipelineRasterState", shared)
        # 共享层必须定义关闭深度与模板的值对象。
        self.assertIn("pub(crate) struct PipelineDepthStencilState", shared)
        # 完整 pipeline 契约必须拥有光栅状态。
        self.assertIn("pub(crate) raster: PipelineRasterState", shared)
        # 完整 pipeline 契约必须拥有深度模板状态。
        self.assertIn("pub(crate) depth_stencil: PipelineDepthStencilState", shared)
        # 每个现有 pipeline 都必须显式选择共享二维状态。
        self.assertEqual(shared.count("raster: PIPELINE_RASTER_2D"), 11)
        # 每个现有 pipeline 都必须显式关闭深度与模板。
        self.assertEqual(shared.count("depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED"), 11)

    # OpenGL 必须在每次 draw 前恢复共享固定状态。
    def test_opengl_explicitly_restores_fixed_state(self) -> None:
        # 读取 OpenGL draw 状态翻译源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # OpenGL 固定状态必须来自当前 pipeline 契约。
        self.assertIn("apply_pipeline_fixed_state(gl, contract.raster, contract.depth_stencil)", opengl)
        # OpenGL 必须穷尽翻译共享正面绕序。
        self.assertIn("fn gl_front_face(front_face: PipelineFrontFace)", opengl)
        # OpenGL 必须显式恢复正面绕序。
        self.assertIn("gl.front_face(gl_front_face(raster.front_face))", opengl)
        # OpenGL 必须显式关闭面剔除。
        self.assertIn("PipelineCullMode::None => gl.disable(glow::CULL_FACE)", opengl)
        # OpenGL 必须显式关闭深度测试。
        self.assertIn("gl.disable(glow::DEPTH_TEST)", opengl)
        # OpenGL 必须显式关闭深度写入。
        self.assertIn("gl.depth_mask(false)", opengl)
        # OpenGL 必须显式关闭模板测试。
        self.assertIn("gl.disable(glow::STENCIL_TEST)", opengl)
        # OpenGL 必须显式关闭模板写入。
        self.assertIn("gl.stencil_mask(0)", opengl)

    # D3D11 必须从共享状态创建并在统一边界绑定原生对象。
    def test_d3d11_maps_and_binds_shared_fixed_state_once(self) -> None:
        # 读取 D3D11 原生状态对象创建源码。
        pipeline = D3D11_PIPELINE.read_text(encoding="utf-8")
        # 读取 D3D11 pipeline 统一状态绑定源码。
        module = D3D11_MODULE.read_text(encoding="utf-8")
        # 读取 D3D11 draw 分派源码。
        draw = D3D11_DRAW.read_text(encoding="utf-8")
        # D3D11 剔除模式必须通过穷尽函数翻译。
        self.assertIn("fn d3d11_cull_mode(cull_mode: PipelineCullMode)", pipeline)
        # D3D11 正面绕序必须通过穷尽函数翻译。
        self.assertIn("fn d3d11_front_counter_clockwise(front_face: PipelineFrontFace)", pipeline)
        # D3D11 深度裁剪必须来自共享状态。
        self.assertIn("DepthClipEnable: d3d11_depth_clip_enabled(state.depth_clip)", pipeline)
        # D3D11 rasterizer 不得继续私自写死剔除模式。
        self.assertNotIn("CullMode: D3D11_CULL_NONE", pipeline)
        # D3D11 必须显式创建 depth-stencil 对象。
        self.assertIn("CreateDepthStencilState(&desc, Some(&mut native))", pipeline)
        # D3D11 深度测试必须来自共享状态。
        self.assertIn("DepthEnable: d3d11_depth_enabled(state.depth)", pipeline)
        # D3D11 深度写入必须来自共享状态。
        self.assertIn("DepthWriteMask: d3d11_depth_write_mask(state.depth)", pipeline)
        # D3D11 模板测试必须来自共享状态。
        self.assertIn("StencilEnable: d3d11_stencil_enabled(state.stencil)", pipeline)
        # D3D11 统一绑定入口必须同时绑定 rasterizer 与 depth-stencil。
        self.assertIn("context.RSSetState(&self.rasterizer)", module)
        # D3D11 统一绑定入口必须覆盖默认深度开启状态。
        self.assertIn("context.OMSetDepthStencilState(&self.depth_stencil, 0)", module)
        # draw 分派必须把同一共享契约交给统一绑定入口。
        self.assertIn("contract.raster,", draw)
        # draw 分派必须同时交付共享深度模板状态。
        self.assertIn("contract.depth_stencil,", draw)
        # 收集所有薄 RHI shader helper 源码。
        helpers = "\n".join(path.read_text(encoding="utf-8") for path in sorted(D3D11_HELPERS.glob("rhi_*.rs")))
        # shader helper 不得再次分散绑定 rasterizer。
        self.assertNotIn("RSSetState", helpers)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
