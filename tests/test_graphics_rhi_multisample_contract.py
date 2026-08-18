# -*- coding: utf-8 -*-
# 验证单样本覆盖语义在 Surface 配置和两个 Adapter 中保持一致。
"""Keep single-sample coverage state identical across native graphics adapters."""

# 启用延迟注解解析，保持测试与其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享采样覆盖值对象。
SHARED = ROOT / "src/native/present/rhi/multisample.rs"
# 定位共享 pipeline 契约。
PIPELINE = ROOT / "src/native/present/rhi/pipeline.rs"
# 定位 EGL Surface 配置。
EGL = ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs"
# 定位 OpenGL draw 状态翻译。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 原生状态对象创建。
D3D11_PIPELINE = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/pipeline.rs"
# 定位 D3D11 公共状态映射。
D3D11_MODULE = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/mod.rs"
# 定位 D3D11 draw 分派边界。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs"
# 定位必须复用共享 sample mask 的 D3D11 shader helper。
D3D11_HELPERS = ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline"


# 集中验证单样本 Surface 配置和两个 Device Adapter 的状态翻译。
class GraphicsRhiMultisampleContractTests(unittest.TestCase):
    # PipelineContract 必须拥有唯一类型化采样覆盖语义。
    def test_pipeline_contract_owns_single_sample_state(self) -> None:
        # 读取共享采样覆盖值对象。
        shared = SHARED.read_text(encoding="utf-8")
        # 读取共享 pipeline 契约。
        pipeline = PIPELINE.read_text(encoding="utf-8")
        # 共享层必须定义封闭采样覆盖类型。
        self.assertIn("pub(crate) enum PipelineMultisampleState", shared)
        # 单样本状态必须提供 alpha-to-coverage 事实。
        self.assertIn("alpha_to_coverage_enabled", shared)
        # 单样本状态必须提供 rasterizer 多样本事实。
        self.assertIn("raster_multisample_enabled", shared)
        # 单样本状态必须提供完整输出 sample mask。
        self.assertIn("pub(crate) const fn sample_mask", shared)
        # 完整 pipeline 契约必须拥有采样覆盖状态。
        self.assertIn("pub(crate) multisample: PipelineMultisampleState", pipeline)
        # 公共二维构造边界必须冻结 SingleSample。
        self.assertIn("multisample: PipelineMultisampleState::SingleSample", pipeline)

    # EGL 与 OpenGL draw 必须显式选择并恢复单样本覆盖状态。
    def test_opengl_surface_and_draw_are_explicitly_single_sample(self) -> None:
        # 读取 EGL config 选择源码。
        egl = EGL.read_text(encoding="utf-8")
        # 读取 OpenGL draw 状态翻译源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # EGL 必须明确拒绝多样本 buffer config。
        self.assertIn("egl::SAMPLE_BUFFERS,\n                0,", egl)
        # EGL 必须明确要求零个多样本样本。
        self.assertIn("egl::SAMPLES,\n                0,", egl)
        # OpenGL draw 必须从当前 pipeline 契约恢复采样覆盖状态。
        self.assertIn("apply_pipeline_multisample(gl, contract.multisample)", opengl)
        # OpenGL 必须穷尽匹配 SingleSample。
        self.assertIn("PipelineMultisampleState::SingleSample =>", opengl)
        # OpenGL 必须关闭 alpha-to-coverage。
        self.assertIn("gl.disable(glow::SAMPLE_ALPHA_TO_COVERAGE)", opengl)
        # OpenGL 必须关闭 sample coverage。
        self.assertIn("gl.disable(glow::SAMPLE_COVERAGE)", opengl)

    # D3D11 三个固定状态阶段必须消费同一共享采样覆盖语义。
    def test_d3d11_maps_one_shared_multisample_state(self) -> None:
        # 读取 D3D11 原生状态创建源码。
        pipeline = D3D11_PIPELINE.read_text(encoding="utf-8")
        # 读取 D3D11 公共状态映射源码。
        module = D3D11_MODULE.read_text(encoding="utf-8")
        # 读取 D3D11 draw 分派源码。
        draw = D3D11_DRAW.read_text(encoding="utf-8")
        # blend descriptor 的 alpha-to-coverage 必须来自共享状态。
        self.assertIn("AlphaToCoverageEnable: d3d11_alpha_to_coverage_enabled(multisample)", pipeline)
        # rasterizer 的多样本开关必须来自同一共享状态。
        self.assertIn("MultisampleEnable: d3d11_raster_multisample_enabled(multisample)", pipeline)
        # 输出 sample mask 必须通过共享状态映射。
        self.assertIn("d3d11_sample_mask(self.multisample_state)", module)
        # draw 分派必须把当前 pipeline 的共享采样覆盖状态交给统一入口。
        self.assertIn("contract.multisample,", draw)
        # 收集所有薄 RHI shader helper 源码。
        helpers = "\n".join(path.read_text(encoding="utf-8") for path in sorted(D3D11_HELPERS.glob("rhi_*.rs")))
        # 七个 helper 必须复用同一个共享 sample mask 入口。
        self.assertEqual(helpers.count("self.rhi_sample_mask()"), 7)
        # shader helper 不得继续私自写死全 sample mask。
        self.assertNotIn("0xffff_ffff", helpers)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
