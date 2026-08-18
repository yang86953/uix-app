# -*- coding: utf-8 -*-
# 验证 FramePlan 到两个 Adapter 只传递固定 t0/s0 的原子采样绑定。
"""Keep sampled texture bindings atomic and slot-free across adapters."""

# 启用延迟注解解析，保持测试与其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享采样绑定与 pass 状态。
PASS_STATE = ROOT / "src/native/present/rhi/pass_state.rs"
# 定位薄 RHI Device 契约。
RHI = ROOT / "src/native/present/rhi.rs"
# 定位类型化 FramePlan 命令。
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan.rs"
# 定位 FramePlan 资源前置验证。
VALIDATION = ROOT / "src/draw/backend/frame_plan_validation.rs"
# 定位 FramePlan 到 Device 的唯一执行边界。
EXECUTION = ROOT / "src/draw/backend/frame_plan_execution.rs"
# 定位 OpenGL Adapter 的共享状态写入与读取路径。
OPENGL_DEVICE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"
# 定位 OpenGL draw 的完整绑定读取路径。
OPENGL_DRAW = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 Adapter 的共享状态写入路径。
D3D11_RESOURCES = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_resources.rs"
# 定位 D3D11 draw 的完整绑定读取路径。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs"
# 列出全部生产采样命令的 Drawing 源文件。
PRODUCERS = (
    # 通用颜色采样路径。
    ROOT / "src/draw/backend/rhi_renderer.rs",
    # blur 双 pass 路径。
    ROOT / "src/draw/backend/rhi_renderer_blur.rs",
    # coverage 路径。
    ROOT / "src/draw/backend/rhi_renderer_coverage.rs",
    # mixed painter-order 路径。
    ROOT / "src/draw/backend/rhi_renderer_mixed.rs",
    # 离屏 sampled quad 路径。
    ROOT / "src/draw/backend/rhi_renderer_sampled.rs",
)


# 集中验证共享值对象、FramePlan、生产端和两个 Adapter 的同一契约。
class GraphicsRhiSampledBindingContractTests(unittest.TestCase):
    # 共享 pass 状态只能保存一个完整采样绑定。
    def test_pass_state_owns_one_atomic_binding(self) -> None:
        # 读取共享 pass 状态源码。
        pass_state = PASS_STATE.read_text(encoding="utf-8")
        # 共享层必须定义采样绑定值对象。
        self.assertIn("pub(crate) struct SampledTextureBinding", pass_state)
        # 值对象必须拥有纹理身份。
        self.assertIn("texture: TextureHandle", pass_state)
        # 值对象必须同时拥有 sampler 身份。
        self.assertIn("sampler: SamplerHandle", pass_state)
        # 活动 pass 只能保存一个可选完整绑定。
        self.assertIn("sampled_binding: Option<SampledTextureBinding>", pass_state)
        # 状态不得继续拆分纹理身份。
        self.assertNotIn("bound_texture: Option", pass_state)
        # 状态不得继续拆分 sampler 身份。
        self.assertNotIn("bound_sampler: Option", pass_state)
        # 共享写入口只接收完整值对象。
        self.assertIn("binding: SampledTextureBinding", pass_state)
        # 任一纹理销毁都必须清除整个绑定。
        self.assertIn("binding.texture() == texture", pass_state)
        # 任一 sampler 销毁也必须清除整个绑定。
        self.assertIn("binding.sampler() == sampler", pass_state)

    # FramePlan 与 Device 接口不得再暴露裸槽位或拆分资源。
    def test_frame_plan_and_device_are_slot_free(self) -> None:
        # 读取 FramePlan 命令定义。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 读取薄 RHI Device trait。
        rhi = RHI.read_text(encoding="utf-8")
        # 读取 FramePlan 的唯一执行映射。
        execution = EXECUTION.read_text(encoding="utf-8")
        # FramePlan 必须以单一值承载完整采样绑定。
        self.assertIn("BindSampledTexture(SampledTextureBinding)", frame_plan)
        # 旧的带字段绑定命令必须消失。
        self.assertNotIn("BindTexture {", frame_plan)
        # FramePlan 不得再出现裸采样槽位。
        self.assertNotIn("slot: u32", frame_plan)
        # Device trait 必须接收同一个完整值对象。
        self.assertIn("fn bind_sampled_texture(", rhi)
        # Device trait 不得保留旧的拆分入口。
        self.assertNotIn("fn bind_texture(", rhi)
        # 执行器必须机械转发同一个绑定值。
        self.assertIn("self.device.bind_sampled_texture(*binding)", execution)

    # 所有生产端与验证器必须只识别类型化绑定命令。
    def test_all_producers_use_the_typed_binding(self) -> None:
        # 合并全部生产采样命令的源码。
        producers = "\n".join(path.read_text(encoding="utf-8") for path in PRODUCERS)
        # 当前九个生产点必须全部构造原子绑定。
        self.assertEqual(producers.count("SampledTextureBinding::new("), 9)
        # 当前九个生产点必须全部使用类型化 FramePlan 命令。
        self.assertEqual(producers.count("FramePlanCommand::BindSampledTexture("), 9)
        # 生产端不得继续声明零号槽位。
        self.assertNotIn("slot: 0", producers)
        # 验证器只需判断完整绑定是否已经生效。
        validation = VALIDATION.read_text(encoding="utf-8")
        # 固定 t0/s0 语义必须由类型表达而不是数值判断。
        self.assertIn("FramePlanCommand::BindSampledTexture(_)", validation)
        # 验证器不得继续匹配裸槽位。
        self.assertNotIn("slot: 0", validation)

    # OpenGL 与 D3D11 必须写入并读取同一个原子状态事实。
    def test_both_adapters_share_the_atomic_binding(self) -> None:
        # 读取 OpenGL 状态写入路径。
        opengl_device = OPENGL_DEVICE.read_text(encoding="utf-8")
        # 读取 OpenGL draw 路径。
        opengl_draw = OPENGL_DRAW.read_text(encoding="utf-8")
        # 读取 D3D11 状态写入路径。
        d3d11_resources = D3D11_RESOURCES.read_text(encoding="utf-8")
        # 读取 D3D11 draw 路径。
        d3d11_draw = D3D11_DRAW.read_text(encoding="utf-8")
        # OpenGL 必须把完整绑定交给共享状态机。
        self.assertIn("self.pass.bind_sampled_texture(binding)?;", opengl_device)
        # D3D11 必须把同一个类型交给共享状态机。
        self.assertIn("self.rhi_device.pass.bind_sampled_texture(binding)?;", d3d11_resources)
        # OpenGL draw 必须一次取得完整绑定。
        self.assertIn(".sampled_binding()", opengl_draw)
        # D3D11 四类 sampled pipeline 都必须一次取得完整绑定。
        self.assertEqual(d3d11_draw.count(".sampled_binding()"), 4)
        # 两个 Adapter 都不得读取拆分纹理状态。
        self.assertNotIn(".bound_texture()", opengl_draw + d3d11_draw)
        # 两个 Adapter 都不得读取拆分 sampler 状态。
        self.assertNotIn(".bound_sampler()", opengl_draw + d3d11_draw)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
