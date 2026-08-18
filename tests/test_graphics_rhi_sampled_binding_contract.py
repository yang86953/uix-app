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
# 定位 OpenGL raster owner 的只读预检桥接路径。
OPENGL_RASTER = ROOT / "src/native/presentation/graphics/opengl/raster/rhi.rs"
# 定位 OpenGL host 的生命周期门禁与只读转发路径。
OPENGL_HOST = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 OpenGL draw 的完整绑定读取路径。
OPENGL_DRAW = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 Device 的生命周期门禁与只读预检入口。
D3D11_DEVICE = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs"
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
    # sampled binding 资源必须由共享 Device 在 activate 前预检。
    def test_sampled_binding_is_preflighted_before_activation(self) -> None:
        # 读取薄 RHI Device 契约。
        rhi = RHI.read_text(encoding="utf-8")
        # 读取 FramePlan 执行器顺序。
        execution = EXECUTION.read_text(encoding="utf-8")
        # 读取 OpenGL 真实资源预检实现。
        opengl_device = OPENGL_DEVICE.read_text(encoding="utf-8")
        # 读取 OpenGL raster owner 的只读桥接实现。
        opengl_raster = OPENGL_RASTER.read_text(encoding="utf-8")
        # 读取 OpenGL host 的生命周期门禁实现。
        opengl_host = OPENGL_HOST.read_text(encoding="utf-8")
        # 读取 D3D11 Device 的生命周期门禁实现。
        d3d11_device = D3D11_DEVICE.read_text(encoding="utf-8")
        # 读取 D3D11 真实资源预检实现。
        d3d11_resources = D3D11_RESOURCES.read_text(encoding="utf-8")
        # GraphicsDevice 必须公开只读 sampled 资源预检入口。
        self.assertIn(
            "fn preflight_sampled_binding(&self, _binding: SampledTextureBinding)",
            rhi,
        )
        # 执行器必须定义 sampled binding 预检扫描。
        self.assertIn("fn validate_sampled_bindings", execution)
        # 扫描必须调用共享 Device 入口。
        self.assertIn("self.device.preflight_sampled_binding(*binding)?", execution)
        # sampled 预检必须位于 Draw 预检之后。
        self.assertLess(
            execution.index("self.validate_draw_resources(steps)?"),
            execution.index("self.validate_sampled_bindings(steps)"),
        )
        # 所有资源预检必须位于 activate 之前。
        self.assertLess(
            execution.index("self.validate_sampled_bindings(steps)"),
            execution.index("self.device.activate()?"),
        )
        # OpenGL Device 必须从真实资源表解析纹理与 sampler。
        self.assertIn("let texture = self.texture(binding.texture())?;", opengl_device)
        self.assertIn("let sampler = self.sampler(binding.sampler())?;", opengl_device)
        # OpenGL Device 必须复用共享采样语义校验。
        self.assertIn(
            "binding.validate_resources(format, sampler_desc)",
            opengl_device,
        )
        # OpenGL raster owner 必须只读委托同一预检实现。
        self.assertIn("rhi_preflight_sampled_binding", opengl_raster)
        self.assertIn("self.rhi.preflight_sampled_binding(binding)", opengl_raster)
        # OpenGL host 必须先门禁，再通过只读 pipeline 借用转发。
        opengl_host_preflight = opengl_host[
            opengl_host.index("fn preflight_sampled_binding(") :
            opengl_host.index("fn update_texture(")
        ]
        self.assertLess(
            opengl_host_preflight.index("self.rhi_ensure_active()?"),
            opengl_host_preflight.index("self.rhi_pipeline()"),
        )
        # 只读预检不得恢复或触碰 native context。
        self.assertNotIn("rhi_make_current", opengl_host_preflight)
        # D3D11 Device 必须先门禁，再委托只读资源 helper。
        d3d11_device_preflight = d3d11_device[
            d3d11_device.index("fn preflight_sampled_binding(") :
            d3d11_device.index("fn create_pipeline(")
        ]
        self.assertLess(
            d3d11_device_preflight.index("self.ensure_active()?"),
            d3d11_device_preflight.index("self.rhi_validate_sampled_binding(binding)"),
        )
        # D3D11 helper 必须从真实资源表解析纹理与 sampler。
        self.assertIn(
            "let texture = self.rhi_device.texture(binding.texture())?;",
            d3d11_resources,
        )
        self.assertIn(
            "let sampler = self.rhi_device.sampler(binding.sampler())?;",
            d3d11_resources,
        )
        # D3D11 helper 必须复用共享采样语义校验。
        self.assertIn(
            "binding.validate_resources(format, sampler_desc)",
            d3d11_resources,
        )

    # 共享 pass 状态只能保存一个完整采样绑定。
    def test_pass_state_owns_one_atomic_binding(self) -> None:
        # 读取共享 pass 状态源码。
        pass_state = PASS_STATE.read_text(encoding="utf-8")
        # 读取 FramePlan 的共享反馈环验证源码。
        validation = VALIDATION.read_text(encoding="utf-8")
        # 共享层必须定义采样绑定值对象。
        self.assertIn("pub(crate) struct SampledTextureBinding", pass_state)
        # 值对象必须拥有纹理身份。
        self.assertIn("texture: TextureHandle", pass_state)
        # 值对象必须同时拥有 sampler 身份。
        self.assertIn("sampler: SamplerHandle", pass_state)
        # 值对象必须保存由 pipeline 派生的采样语义。
        self.assertIn("sampling: PipelineSampling", pass_state)
        # 构造入口必须从完整 pipeline 派生采样语义。
        self.assertIn("pub(crate) const fn for_pipeline(", pass_state)
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
        # Adapter 状态机必须继续保留同一纹理反馈环防线。
        self.assertIn("active.target.texture() == Some(binding.texture())", pass_state)
        # FramePlan helper 必须接收目标与完整采样绑定。
        self.assertIn("validate_sampled_binding_target(", validation)
        # helper 只能对离屏 Texture target 执行身份比较。
        self.assertIn("if let RenderTargetRef::Texture(target_texture) = target", validation)
        self.assertIn("target_texture == binding.texture()", validation)
        # 反馈环必须返回稳定参数错误。
        self.assertIn('"FramePlan texture feedback loop is invalid"', validation)

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
        # 读取 FramePlan 的完整命令顺序验证入口。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 当前九个生产点必须全部构造原子绑定。
        self.assertEqual(producers.count("SampledTextureBinding::for_pipeline("), 9)
        # 当前九个生产点必须全部使用类型化 FramePlan 命令。
        self.assertEqual(producers.count("FramePlanCommand::BindSampledTexture("), 9)
        # 生产端不得继续声明零号槽位。
        self.assertNotIn("slot: 0", producers)
        # 验证器必须读取最近一次完整绑定。
        validation = VALIDATION.read_text(encoding="utf-8")
        # 最近绑定必须按 painter order 逆向查找。
        self.assertIn("preceding.iter().rev().find_map", validation)
        # 固定 t0/s0 语义必须作为完整绑定值读取。
        self.assertIn("FramePlanCommand::BindSampledTexture(binding) => Some(*binding)", validation)
        # 绑定语义必须与当前 Draw pipeline 契约匹配。
        self.assertIn("binding.matches_pipeline(packet.pipeline)", validation)
        # 每条绑定命令都必须在 FramePlan 顺序遍历中立即检查反馈环。
        self.assertIn("validation::validate_sampled_binding_target(pass.target, *binding)?", frame_plan)
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
        self.assertIn("binding.validate_resources(format, sampler_desc)", opengl_device)
        self.assertIn("self.pass.bind_sampled_texture(binding)?;", opengl_device)
        # D3D11 必须把同一个类型交给共享状态机。
        self.assertIn("binding.validate_resources(format, sampler_desc)", d3d11_resources)
        self.assertIn("self.rhi_device.pass.bind_sampled_texture(binding)?;", d3d11_resources)
        # OpenGL draw 必须一次取得完整绑定。
        self.assertIn("sampled_binding_for(pipeline)", opengl_draw)
        # D3D11 四类 sampled pipeline 都必须一次取得完整绑定。
        self.assertEqual(d3d11_draw.count(".sampled_binding_for(packet.pipeline)"), 4)
        # 两个 draw Adapter 不得重复解释 sampling.accepts。
        self.assertNotIn("contract.sampling.accepts", opengl_draw + d3d11_draw)
        # 两个 Adapter 都不得读取拆分纹理状态。
        self.assertNotIn(".bound_texture()", opengl_draw + d3d11_draw)
        # 两个 Adapter 都不得读取拆分 sampler 状态。
        self.assertNotIn(".bound_sampler()", opengl_draw + d3d11_draw)

    # 两端执行期绑定必须复用预检，并在共享 pass 写入前完成。
    def test_adapters_reuse_preflight_before_pass_write(self) -> None:
        # 读取 OpenGL Adapter 的绑定实现。
        opengl = OPENGL_DEVICE.read_text(encoding="utf-8")
        # 读取 D3D11 的绑定实现。
        d3d11 = D3D11_RESOURCES.read_text(encoding="utf-8")
        # 保留每个 Adapter 的预检调用与共享 pass 写入标记。
        adapter_markers = (
            # OpenGL 执行期防御标记。
            (
                opengl,
                "self.preflight_sampled_binding(binding)?;",
                "self.pass.bind_sampled_texture(binding)?;",
            ),
            # D3D11 执行期防御标记。
            (
                d3d11,
                "self.rhi_validate_sampled_binding(binding)?;",
                "self.rhi_device.pass.bind_sampled_texture(binding)?;",
            ),
        )
        # 分别检查两个 Adapter 的绑定函数。
        for source, preflight, pass_write in adapter_markers:
            # 只截取绑定函数，避免其它资源路径偶然满足断言。
            body = source[source.index("bind_sampled_texture("):]
            # 执行期绑定必须保留同一资源预检防线。
            self.assertIn(preflight, body)
            # pass 写入必须存在于绑定函数中。
            self.assertIn(pass_write, body)
            # 真实资源预检必须早于共享 pass 写入。
            self.assertLess(body.index(preflight), body.index(pass_write))


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
