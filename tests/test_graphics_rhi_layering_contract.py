# -*- coding: utf-8 -*-
# 说明本文件只守卫 Drawing、薄 RHI 与原生 Adapter 的依赖方向。
"""Lock the platform-independent graphics layering contract."""
# 引入单元测试框架。
import unittest
# 引入带标识符边界的依赖匹配工具。
import re
# 引入路径遍历工具。
from pathlib import Path
# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位不得依赖原生图形 API 的 Drawing System。
DRAW_ROOT = ROOT / "src/draw"
# 定位只允许实现薄 RHI 的原生图形 Adapter。
ADAPTER_ROOTS = (
    # 收集 Windows 默认 D3D11 Adapter。
    ROOT / "src/native/presentation/graphics/d3d11",
    # 收集 Windows 与 Linux 共享的 OpenGL ES Adapter。
    ROOT / "src/native/presentation/graphics/opengl",
)
# 读取指定目录下全部 Rust 源文件并保留文件身份。
def rust_sources(root: Path) -> list[tuple[Path, str]]:
    # 按路径排序，保证失败输出稳定。
    return [(path, path.read_text(encoding="utf-8")) for path in sorted(root.rglob("*.rs"))]
# 固定跨平台图形实现的单向依赖规则。
class GraphicsRhiLayeringContractTests(unittest.TestCase):
    # Drawing System 只能依赖薄 RHI，不能直接绑定任一原生图形 API。
    def test_drawing_system_has_no_native_graphics_api_dependency(self) -> None:
        # 列出会把平台实现细节泄漏进 Drawing System 的命名空间和枚举前缀。
        forbidden = (r"(?<!\w)::windows", r"(?<!\w)windows::Win32", r"(?<!\w)glow::", r"(?<!\w)ash::", r"(?<!\w)metal::", r"(?<!\w)wgpu::", r"(?<!\w)D3D11_", r"(?<!\w)DXGI_", r"(?<!\w)vk::")
        # 逐文件检查依赖边界。
        for path, source in rust_sources(DRAW_ROOT):
            # 为失败报告保留相对仓库路径。
            relative = path.relative_to(ROOT)
            # 逐个拒绝原生 API 标记。
            for marker in forbidden:
                # 使用子测试精确指出泄漏的文件和 API。
                with self.subTest(path=str(relative), marker=marker):
                    # Drawing 只能看到 API 无关的 RHI 值和 trait。
                    self.assertIsNone(re.search(marker, source))

    # 原生 Adapter 只能机械实现 RHI，不能反向取得 UI 或 Drawing 语义。
    def test_native_adapters_do_not_import_high_level_ui_semantics(self) -> None:
        # 列出不应进入底层 Adapter 的高层模块路径。
        forbidden = ("crate::draw::", "crate::ui::", "crate::widgets::", "crate::components::")
        # 逐个检查当前生产 GPU Adapter 家族。
        for adapter_root in ADAPTER_ROOTS:
            # 逐文件检查反向依赖。
            for path, source in rust_sources(adapter_root):
                # 为失败报告保留相对仓库路径。
                relative = path.relative_to(ROOT)
                # 逐个拒绝高层语义导入。
                for marker in forbidden:
                    # 使用子测试精确指出发生反向依赖的位置。
                    with self.subTest(path=str(relative), marker=marker):
                        # Adapter 不得知道组件、场景或 Canvas2D 类型。
                        self.assertNotIn(marker, source)

    # 管线身份与 ABI 必须由共享 RHI 契约拥有，Adapter 只做翻译。
    def test_pipeline_contract_is_shared_and_typed(self) -> None:
        # 读取共享 pipeline 契约。
        pipeline = (ROOT / "src/native/presentation/rhi/pipeline.rs").read_text(encoding="utf-8")
        # 读取两套当前生产 Adapter 的 draw 翻译层。
        adapters = rust_sources(ADAPTER_ROOTS[0]) + rust_sources(ADAPTER_ROOTS[1])
        # 共享层必须使用封闭类型表达 pipeline 身份。
        self.assertIn("pub(crate) enum PipelineKind", pipeline)
        # 共享层必须集中声明顶点、uniform、采样与混合事实。
        self.assertIn("pub(crate) struct PipelineContract", pipeline)
        # 旧的无类型数字 key 不得在任一生产 Adapter 复活。
        for path, source in adapters:
            # 使用子测试报告具体回归文件。
            with self.subTest(path=str(path.relative_to(ROOT))):
                # Adapter 必须匹配 PipelineKind，而不是约定数字。
                self.assertNotIn("pipeline_keys", source)

    # Surface 帧事务必须原子借用同一原生 context，禁止跨 Adapter 拼接 device 与 surface。
    def test_surface_execution_requires_one_context_owner(self) -> None:
        # 读取 FramePlan 的公共执行入口。
        plan = (ROOT / "src/draw/backend/frame_plan.rs").read_text(encoding="utf-8")
        # 读取唯一 surface 事务实现。
        execution = (ROOT / "src/draw/backend/frame_plan_execution.rs").read_text(encoding="utf-8")
        # 读取只按计划作用域选择最终 present 或离屏 submit 的执行组件。
        renderer_execution = (ROOT / "src/draw/backend/rhi_renderer_execution.rs").read_text(encoding="utf-8")
        # FramePlan 必须用封闭类型原子保存 Surface 或 Offscreen 作用域。
        self.assertIn("enum FramePlanScope", plan)
        # 旧的可选 SurfaceToken 不得继续允许模糊的第三种状态。
        self.assertNotIn("surface: Option<SurfaceToken>", plan)
        # 离屏作用域不能携带或伪造 SurfaceToken。
        self.assertIn("FramePlanScope::Offscreen", plan)
        # 完整 surface 帧只能接收同时拥有 Device 与 Surface 的组合 context。
        self.assertIn("pub(crate) fn execute_on_context(", plan)
        # 禁止恢复可把不同原生 Adapter 对象任意拼接的分离执行入口。
        self.assertNotIn("device: &mut dyn GraphicsDevice,\n        surface: &mut dyn GraphicsSurface", plan)
        # acquire、submit、代际复核与 present 必须由同一事务函数拥有。
        surface_transaction = execution[
            # 从完整 surface 执行入口开始。
            execution.index("pub(crate) fn execute_on_context_with_before_present(") :
            # 到 offscreen 执行入口结束。
            execution.index("pub(crate) fn execute_offscreen_on_device")
        ]
        # 事务必须从组合 context 获取 surface image。
        self.assertIn("let frame = context.surface().acquire()?;", surface_transaction)
        # 事务必须通过同一 context 提交 device 工作。
        self.assertIn("let submission = context.device().submit()?;", surface_transaction)
        # 事务必须通过同一 context 完成不可拆的最终 present。
        self.assertIn(".present(RhiPresentTransaction::new(", surface_transaction)
        # 禁止恢复丢弃 acquired frame 却延后 present 的 surface segment 入口。
        self.assertNotIn("execute_surface_segment_on_context", execution)
        # Renderer 门面必须原子保存私有封闭角色与自身生命周期。
        self.assertIn("pub(crate) struct RhiRendererFrame", renderer_execution)
        # 截取 Renderer 帧角色定义，核对 Offscreen 不持有 Surface 能力。
        frame_contract = renderer_execution[renderer_execution.index("enum RhiRendererFrameRole") : renderer_execution.index("enum RhiRendererFrameExecutionState")]
        # Offscreen 变体必须显式拥有 Device 与纹理目标。
        offscreen_contract = frame_contract[frame_contract.index("Offscreen {") :]
        # Device-only 变体不能携带 present damage。
        self.assertNotIn("PresentDamage", offscreen_contract)
        # Device-only 变体不能携带组合 context。
        self.assertNotIn("GraphicsContextRhi", offscreen_contract)
        # Renderer 帧必须从自身变体构造匹配的 Surface/Offscreen 计划。
        self.assertIn("FramePlan::new(context.surface_ref().token(), damage.clone())", renderer_execution)
        self.assertIn("RhiRendererFrameRole::Offscreen { .. } => FramePlan::offscreen()", renderer_execution)
        # Surface 执行边界必须显式拒绝 Offscreen 计划。
        self.assertIn("if !plan.targets_surface()", renderer_execution)
        # 截取唯一命令执行器，验证所有 surface/offscreen 命令共享同一原生执行边界。
        executor = execution[
            # 从统一执行方法开始。
            execution.index("pub(super) fn execute(mut self, steps: &[FramePlanStep])") :
            # 到 pass 执行方法结束。
            execution.index("fn execute_pass", execution.index("pub(super) fn execute(mut self, steps: &[FramePlanStep])"))
        ]
        # Device 激活必须由计划执行器自身调用，不能依赖上层碰巧留下正确 context。
        self.assertIn("self.device.activate()?;", executor)
        # Device 健康 preflight 必须由计划执行器自身调用，不能依赖上层调用顺序。
        self.assertIn("self.device.maintain()?;", executor)
        # owner-context 激活必须先于健康检查。
        self.assertLess(executor.index("self.device.activate()?;"), executor.index("self.device.maintain()?;"))
        # 健康 preflight 必须发生在第一条 FramePlan 命令之前。
        self.assertLess(executor.index("self.device.maintain()?;"), executor.index("for step in steps"))
    # 原生 context 激活与设备健康检查必须保持两个正交生命周期契约。
    def test_device_activation_and_health_are_orthogonal(self) -> None:
        # 读取薄 RHI Device 契约。
        rhi = (ROOT / "src/native/presentation/rhi/mod.rs").read_text(encoding="utf-8")
        # 读取唯一生产 GPU recipe owner。
        owner = (ROOT / "src/native/presentation/contracts/gpu_recipe_owner.rs").read_text(encoding="utf-8")
        # 读取 Drawing GPU Module 私有的 FramePlan 启动探针。
        probe = (ROOT / "src/draw/backend/gpu/device_probe.rs").read_text(encoding="utf-8")
        # 读取 OpenGL Device Adapter 门面。
        opengl = (ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs").read_text(encoding="utf-8")
        # Device 必须分别声明原生 context 激活和健康维护。
        self.assertIn("fn activate(&mut self) -> Result<()>", rhi)
        # 健康维护不得被删除或重新混入 Surface。
        self.assertIn("fn maintain(&mut self) -> Result<()>", rhi)
        # Drawing probe 只接收 owner 已激活的 Device，并在创建资源前检查健康。
        self.assertIn("GraphicsDevice::maintain(device)?;", probe)
        # 健康检查必须先于探针资源事务。
        self.assertLess(probe.index("GraphicsDevice::maintain(device)?;"), probe.index("ProbeScope::new()"))
        # 组合 owner 借用必须激活当前原生 context。
        context_borrow = owner[owner.index("pub(crate) fn rhi_context(") : owner.index("pub(crate) fn resize_surface(")]
        # 激活责任必须由唯一 owner 承担，不泄漏给 Drawing 调用方。
        self.assertIn("GraphicsDevice::activate(context.device())?;", context_borrow)
        # 窄 Device 借用必须复用同一个组合激活边界。
        device_borrow = owner[owner.index("pub(crate) fn rhi_device(") : owner.index("pub(crate) fn rhi_surface(")]
        # 禁止 Device 借用绕过组合 owner 直接暴露未激活 context。
        self.assertIn("self.rhi_context()?.device()", device_borrow)
        # 截取 OpenGL 激活方法。
        opengl_activate = opengl[opengl.index("fn activate(&mut self)") : opengl.index("fn maintain(&mut self)")]
        # OpenGL 只在 activate 中恢复 thread-current context。
        self.assertIn("self.rhi_make_current()", opengl_activate)
        # 截取 OpenGL 健康维护方法。
        opengl_maintain = opengl[opengl.index("fn maintain(&mut self)") : opengl.index("fn inject_device_lost_for_test")]
        # 健康维护只消费 typed device 状态。
        self.assertIn("rhi_maintain()", opengl_maintain)
        # DeviceLost 不得阻止 checked teardown 所需的 context 激活。
        self.assertNotIn("rhi_make_current", opengl_maintain)
    # Device submit 与 Surface present 必须共享同一提交身份状态机。
    def test_submission_sequence_is_shared_by_both_adapters(self) -> None:
        # 读取薄 RHI 组合入口。
        rhi = (ROOT / "src/native/presentation/rhi/mod.rs").read_text(encoding="utf-8")
        # 读取 API 无关提交序列契约。
        submission = (ROOT / "src/native/presentation/rhi/submission.rs").read_text(encoding="utf-8")
        # 读取 OpenGL Device 提交实现。
        opengl_device = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 OpenGL Surface 呈现实现。
        opengl_surface = (ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs").read_text(encoding="utf-8")
        # 读取 D3D11 Device 状态。
        d3d11_device = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 D3D11 Device 提交与校验实现。
        d3d11_submit = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_submit.rs").read_text(encoding="utf-8")
        # 读取 D3D11 Surface 呈现实现。
        d3d11_surface = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi.rs").read_text(encoding="utf-8")
        # 薄 RHI 必须装配独立的共享提交状态机。
        self.assertIn("mod submission;", rhi)
        # 共享类型必须同时拥有下一身份和最近提交两个事实。
        self.assertIn("pub(crate) struct RhiSubmissionSequence", submission)
        # 下一身份必须只存在于共享状态机。
        self.assertIn("next_raw: Option<u64>", submission)
        # 最近提交必须只存在于共享状态机。
        self.assertIn("last: Option<SubmissionHandle>", submission)
        # 两个 Adapter 都必须把签发委托给共享状态机。
        self.assertIn("submission_sequence.issue()", opengl_device)
        # D3D11 不得再只返回一个未被 Surface 追踪的计数值。
        self.assertIn("submission_sequence.issue()", d3d11_submit)
        # OpenGL Surface 必须在原生交换前验证完整事务。
        self.assertIn("rhi_validate_present(", opengl_surface)
        # D3D11 Surface 必须在 DXGI Present 前执行相同门禁。
        self.assertIn("validate_present_impl(", d3d11_surface)
        # OpenGL Device 必须把完整事务交给共享门禁。
        self.assertIn("transaction.validate(", opengl_device)
        # D3D11 Device 必须调用同一共享门禁。
        self.assertIn("transaction.validate(", d3d11_submit)
        # Adapter 不得继续维护历史私有字段。
        for source in (opengl_device, d3d11_device, d3d11_submit):
            # 禁止私有下一提交计数器重新出现。
            self.assertNotIn("next_submission", source)
            # 禁止私有最近提交缓存重新出现。
            self.assertNotIn("last_submission", source)
        # D3D11 present 参数不得再用下划线伪装为可忽略值。
        self.assertNotIn("_submission: crate::native::present::rhi::SubmissionHandle", d3d11_surface)
    # Render pass 生命周期、几何门禁和资源冲突必须由共享 RHI 状态机拥有。
    def test_render_pass_state_is_shared_by_both_adapters(self) -> None:
        # 读取薄 RHI 组合入口。
        rhi = (ROOT / "src/native/presentation/rhi/mod.rs").read_text(encoding="utf-8")
        # 读取 API 无关的 pass 状态机。
        pass_state = (ROOT / "src/native/presentation/rhi/pass_state.rs").read_text(encoding="utf-8")
        # 读取 OpenGL Device Adapter 状态和命令实现。
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 D3D11 Device Adapter 状态和 pass 实现。
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 D3D11 资源绑定实现。
        d3d11_resources = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_resources.rs").read_text(encoding="utf-8")
        # 读取 D3D11 pass 收尾实现。
        d3d11_submit = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_submit.rs").read_text(encoding="utf-8")
        # 薄 RHI 必须装配独立共享状态机。
        self.assertIn("mod pass_state;", rhi)
        # 共享状态机必须拥有唯一活动 pass 事实。
        self.assertIn("pub(crate) struct RhiPassState", pass_state)
        # 活动目标与 extent 必须由一个原子值共同生灭。
        self.assertIn("active: Option<ActiveRhiPass>", pass_state)
        # pass 状态不得保存可被后续 Draw 继承的 scissor 历史。
        self.assertNotIn("scissor: Option<RhiScissor>", pass_state)
        # 目标反馈环必须只接收当前 packet 的纹理身份并在 API 无关层拒绝。
        self.assertIn("if target.texture() == Some(texture)", pass_state)
        # OpenGL 必须把 pass 开始委托给共享状态机。
        self.assertIn("self.pass.begin(target, extent, load)?;", opengl)
        # D3D11 必须把同一转换委托给共享状态机。
        self.assertIn("self.rhi_device.pass.begin(target, extent, load)?;", d3d11)
        self.assertIn("packet.sampling()", opengl)
        self.assertIn("packet.sampling()", d3d11_resources + d3d11)
        self.assertNotIn("sampled_binding_for", opengl + d3d11_resources + d3d11)
        self.assertNotIn("bind_sampled_texture", opengl + d3d11_resources + d3d11)
        # 两个 Adapter 不得保留会再次漂移的平行状态字段。
        for adapter, source in (("opengl", opengl), ("d3d11", d3d11)):
            # 逐个拒绝旧的重复事实来源。
            for field in ("pass_open:", "active_extent:", "active_target_handle:", "bound_texture:", "bound_sampler:"):
                # 失败时同时报告 Adapter 和字段。
                with self.subTest(adapter=adapter, field=field):
                    # 所有 API 无关 pass 状态只能存在于 RhiPassState。
                    self.assertNotIn(field, source)
        # D3D11 pass 收尾必须显式解除采样输入，禁止驱动隐式解决冲突。
        self.assertIn("PSSetShaderResources(0, Some(&[None]))", d3d11_submit)
        # D3D11 pass 收尾必须显式解除原生输出目标。
        self.assertIn("OMSetRenderTargets(None, None)", d3d11_submit)
        # D3D11 最终必须通过共享状态机原子清理逻辑事实。
        self.assertIn("self.rhi_device.pass.end()?;", d3d11_submit)
    # Device 与 Surface 的能力事实必须由各自角色拥有，组合 context 只负责同源借用。
    def test_device_and_surface_capabilities_are_orthogonal(self) -> None:
        # 读取两个角色的共享能力值。
        capabilities = (ROOT / "src/native/presentation/rhi/capabilities.rs").read_text(encoding="utf-8")
        # 读取两个薄 RHI trait。
        rhi = (ROOT / "src/native/presentation/rhi/mod.rs").read_text(encoding="utf-8")
        # 读取 D3D11 Device Adapter。
        d3d11_device = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 D3D11 Surface Adapter。
        d3d11_surface = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi.rs").read_text(encoding="utf-8")
        # 读取 OpenGL Device profile。
        opengl_device = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi.rs").read_text(encoding="utf-8")
        # 读取 OpenGL Surface host。
        opengl_surface = (ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs").read_text(encoding="utf-8")
        # 读取两个 Adapter 把 Surface 事实冻结为 registry recipe 的创建边界。
        adapter_factories = "\n".join((ROOT / path).read_text(encoding="utf-8") for path in ("src/native/presentation/graphics/d3d11/adapter/mod.rs", "src/native/presentation/graphics/opengl/adapter/mod.rs"))
        # 读取 GPU owner 对静态 recipe 与实际 Surface 的一致性门禁。
        gpu_owner = (ROOT / "src/native/presentation/contracts/gpu_recipe_owner.rs").read_text(encoding="utf-8")
        # 读取 Drawing Renderer 私有的能力投影。
        raster_capabilities = (ROOT / "src/draw/backend/gpu/capabilities.rs").read_text(encoding="utf-8")
        # Device 与 Surface 必须拥有不同的类型身份。
        self.assertIn("pub(crate) struct GraphicsDeviceCapabilities", capabilities)
        self.assertIn("pub(crate) struct GraphicsSurfaceCapabilities", capabilities)
        # Device trait 只能返回 Device 能力。
        self.assertIn("fn device_capabilities(&self) -> GraphicsDeviceCapabilities", rhi)
        # Surface trait 只能返回 Surface 能力。
        self.assertIn("fn surface_capabilities(&self) -> GraphicsSurfaceCapabilities", rhi)
        # Device 能力定义不得包含 Surface 回读事实。
        device_section = capabilities[:capabilities.index("pub(crate) struct GraphicsSurfaceCapabilities")]
        # Device 角色不能读取 Surface 回读状态。
        self.assertNotIn("pub(crate) readback:", device_section)
        # 截取 Surface 能力定义，检查它只保留直接实现的可选原语。
        surface_section = capabilities[capabilities.index("pub(crate) struct GraphicsSurfaceCapabilities"):]
        # Surface 必须拥有最终呈现边界的类型化像素保留证明。
        self.assertIn("pub(crate) present_coherency: PresentCoherency", surface_section)
        # 局部呈现必须只由类型化 PresentCoherency 表达。
        self.assertNotIn("partial_present", surface_section)
        # 遮挡退出探测必须由实际 Present 结果驱动，不能保留未消费布尔值。
        self.assertNotIn("occlusion", surface_section)
        # D3D11 Device capability 不得再读取 swapchain。
        d3d11_device_capabilities = d3d11_device[
            # 从 Device capability 方法开始。
            d3d11_device.index("fn device_capabilities(") :
            # 到下一项 Device 操作结束。
            d3d11_device.index("fn maintain(", d3d11_device.index("fn device_capabilities("))
        ]
        # Device profile 必须完全独立于 Surface owner。
        self.assertNotIn("swap_chain", d3d11_device_capabilities)
        # D3D11 Surface 必须从实际 swapchain 报告 coherency 与同步回读原语。
        self.assertIn("GraphicsSurfaceCapabilities::with_readback(", d3d11_surface)
        # 构造参数必须来自该 Surface 的真实 swapchain 契约。
        self.assertIn("self.present_coherency()", d3d11_surface)
        # OpenGL Device profile 不得声明 Surface 回读。
        self.assertNotIn("surface_readback", opengl_device)
        # OpenGL Surface host 必须独立报告 FullOnly 与同步回读能力。
        self.assertIn("GraphicsSurfaceCapabilities::with_readback(", opengl_surface)
        # 未实现局部交换证明的 OpenGL Surface 必须显式保持 FullOnly。
        self.assertIn("PresentCoherency::FullOnly", opengl_surface)
        # Candidate recipe 只能从实际 Surface capability 复制呈现一致性。
        self.assertIn("GraphicsSurface::surface_capabilities(&ctx)", adapter_factories)
        # GPU owner 必须拒绝 registry recipe 与实际 Surface 事实漂移。
        self.assertIn("caps.present_coherency != surface_capabilities.present_coherency", gpu_owner)
        # 薄 RHI 不得声明由 Renderer 组合实现的 retained framebuffer 策略。
        self.assertNotIn("retained_framebuffer", capabilities)
        # Drawing 必须从 render-to-texture、采样和复制原语推导 retained color target。
        self.assertIn("retained_color_target: capabilities.render_to_texture", raster_capabilities)
        # 采样事实必须参与同一个派生表达式。
        self.assertIn("&& capabilities.sampled_textures", raster_capabilities)
        # 复制事实必须参与同一个派生表达式。
        self.assertIn("&& capabilities.texture_copy", raster_capabilities)
    # Gradient 的仿射字段与 alpha 语义必须由共享契约拥有，Adapter 只能机械映射。
    def test_gradient_uniform_and_alpha_semantics_are_shared(self) -> None:
        # 读取共享 Gradient 值对象。
        gradient = (ROOT / "src/native/presentation/rhi/gradient.rs").read_text(encoding="utf-8")
        # 读取 Drawing 到 FramePlan 的 Gradient lowering。
        renderer = (ROOT / "src/draw/backend/rhi_renderer_gradient.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 的共享字段映射。
        opengl_draw = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        # 读取 OpenGL Gradient shader。
        opengl_shader = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs").read_text(encoding="utf-8")
        # 读取 D3D11 Gradient shader。
        d3d11_shader = (ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/mod.rs").read_text(encoding="utf-8")
        # 共享层必须拥有固定值对象而非仅声明字节数。
        self.assertIn("pub(crate) struct RhiGradientRasterParams", gradient)
        # Renderer 必须使用共享构造器派生仿射边和线性长度。
        self.assertIn("RhiGradientRasterParams::new(", renderer)
        # 截取 OpenGL Gradient 分支，排除其它 pipeline 的合法字段布局。
        opengl_gradient = opengl_draw[
            # 从类型化 Gradient 分派开始。
            opengl_draw.index("PipelineKind::GradientRect =>") :
            # 到下一个 pipeline 分派结束。
            opengl_draw.index("PipelineKind::GlyphCoverageQuad =>")
        ]
        # OpenGL 必须读取共享字段常量，不得保留数字偏移。
        for offset in (
            # viewport 字段身份。
            "GRADIENT_VIEWPORT_FLOAT_OFFSET",
            # origin 与 X 边字段身份。
            "GRADIENT_ORIGIN_EDGE_X_FLOAT_OFFSET",
            # Y 边字段身份。
            "GRADIENT_EDGE_Y_FLOAT_OFFSET",
            # 起始颜色字段身份。
            "GRADIENT_COLOR_A_FLOAT_OFFSET",
            # 结束颜色字段身份。
            "GRADIENT_COLOR_B_FLOAT_OFFSET",
            # 模式与方向或半径字段身份。
            "GRADIENT_PARAMS_FLOAT_OFFSET",
        ):
            # 每个共享字段都必须进入 Adapter 映射。
            self.assertIn(offset, opengl_gradient)
        # GradientRect 统一使用 straight-alpha，OpenGL 径向分支不得提前 premultiply。
        self.assertIn("fragColor = color;", opengl_shader)
        # 明确拒绝曾导致 Linux alpha 被乘两次的旧表达式。
        self.assertNotIn("fragColor = vec4(color.rgb * color.a, color.a);", opengl_shader)
        # D3D11 线性与径向分支同样直接返回 straight-alpha 插值结果。
        self.assertGreaterEqual(d3d11_shader.count("return lerp(u_color_a, u_color_b, t);"), 2)
    # Shape 的分析覆盖率必须写入唯一输出变量，禁止 Adapter 产生作用域分叉。
    def test_shape_shader_mask_has_one_owner_in_each_adapter(self) -> None:
        # 读取 OpenGL 的固定 Shape shader。
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs").read_text(encoding="utf-8")
        # 读取 D3D11 的固定 Shape shader。
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/mod.rs").read_text(encoding="utf-8")
        # 截取 OpenGL Shape 片元阶段，排除其它 pipeline 的局部变量。
        opengl_shape = opengl[
            # 从 Shape 片元常量开始。
            opengl.index("pub(super) const SHAPE_FRAGMENT") :
            # 到后续 Sector 片元常量结束。
            opengl.index("pub(super) const SECTOR_FRAGMENT")
        ]
        # 截取 D3D11 Rect HLSL，排除其它 shader 的局部变量。
        d3d11_shape = d3d11[
            # 从 Shape HLSL 常量开始。
            d3d11.index("const RECT_HLSL") :
            # 到后续 Glyph HLSL 常量结束。
            d3d11.index("const GLYPH_HLSL")
        ]
        # OpenGL 只能声明一个由所有 Shape 分支共同赋值的覆盖率变量。
        self.assertEqual(opengl_shape.count("float mask;"), 1)
        # D3D11 同样只能声明一个 mask，禁止描边分支遮蔽最终输出所读变量。
        self.assertEqual(d3d11_shape.count("float mask;"), 1)
    # Shadow 的仿射字段、资源生命周期和 Adapter 映射必须由独立共享契约拥有。
    def test_shadow_uniform_and_resources_are_shared_and_typed(self) -> None:
        # 读取共享 Shadow 值对象。
        shadow = (ROOT / "src/native/presentation/rhi/shadow.rs").read_text(encoding="utf-8")
        # 读取 Shadow 到 FramePlan 的唯一 command lowering。
        renderer = (ROOT / "src/draw/backend/rhi_renderer_shadow.rs").read_text(encoding="utf-8")
        # 读取混合 painter-order 路径，确认其没有复制 ABI 公式。
        mixed = (ROOT / "src/draw/backend/rhi_renderer_mixed.rs").read_text(encoding="utf-8")
        # 读取共享 pipeline 大小契约。
        pipeline = (ROOT / "src/native/presentation/rhi/pipeline.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 的共享字段映射。
        opengl_draw = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        # 共享层必须拥有固定值对象而不是仅声明六个匿名 float4。
        self.assertIn("pub(crate) struct RhiShadowRasterParams", shadow)
        # Renderer 必须使用共享构造器派生两条仿射边和环境标记。
        self.assertIn("RhiShadowRasterParams::new(", renderer)
        # 混合路径必须复用唯一 Shadow command lowering，禁止复制 24-float 数组。
        self.assertIn("RhiRenderer::append_shadow_commands(", mixed)
        # Shadow 不得因当前字节数相同而借用 Shape 的 pipeline 或常量资源生命周期。
        self.assertNotIn("self.ensure_shape_resources(context)", renderer)
        # PipelineContract 必须引用 Shadow 自己的固定 ABI 大小。
        self.assertIn("Self::Shadow => SHADOW_UNIFORM_BYTES", pipeline)
        # 截取 OpenGL Shadow 分支，排除其它 pipeline 的合法字段布局。
        opengl_shadow = opengl_draw[
            # 从类型化 Shadow 分派开始。
            opengl_draw.index("PipelineKind::BoxShadow =>") :
            # 到下一个 pipeline 分派结束。
            opengl_draw.index("PipelineKind::BlurPass =>")
        ]
        # OpenGL 必须读取共享字段常量，不得保留数字偏移。
        for offset in (
            # viewport 字段身份。
            "SHADOW_VIEWPORT_FLOAT_OFFSET",
            # origin 与 X 边字段身份。
            "SHADOW_ORIGIN_EDGE_X_FLOAT_OFFSET",
            # straight-alpha 颜色字段身份。
            "SHADOW_COLOR_FLOAT_OFFSET",
            # 四角半径字段身份。
            "SHADOW_RADIUS_FLOAT_OFFSET",
            # Y 边与两轴 blur 字段身份。
            "SHADOW_EDGE_Y_BLUR_FLOAT_OFFSET",
            # 本体尺寸与环境曲线标记字段身份。
            "SHADOW_BODY_SIZE_AMBIENT_FLOAT_OFFSET",
        ):
            # 每个共享字段都必须进入 Adapter 映射。
            self.assertIn(offset, opengl_shadow)
        # 禁止恢复由 Adapter 解释的匿名 Shadow float4 偏移。
        for numeric_offset in (4, 8, 12, 16, 20):
            # 每个旧数字偏移都必须从 Shadow 分支消失。
            self.assertNotIn(f"read_vec4(&uniform, {numeric_offset})", opengl_shadow)
    # Blur 的尺寸、区域、方向、tap 与权重必须由一个共享类型化 ABI 拥有。
    def test_blur_uniform_is_shared_typed_and_complete(self) -> None:
        # 读取共享 Blur 值对象。
        blur = (ROOT / "src/native/presentation/rhi/blur.rs").read_text(encoding="utf-8")
        # 读取 Drawing 的高斯核构造与 FramePlan lowering。
        renderer = (ROOT / "src/draw/backend/rhi_renderer_blur.rs").read_text(encoding="utf-8")
        # 读取共享 pipeline 大小契约。
        pipeline = (ROOT / "src/native/presentation/rhi/pipeline.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 的共享字段映射。
        opengl_draw = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        # 读取 OpenGL Blur shader 字段声明。
        opengl_shader = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs").read_text(encoding="utf-8")
        # 读取 D3D11 Blur cbuffer 字段声明。
        d3d11_shader = (ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/mod.rs").read_text(encoding="utf-8")
        # 共享层必须拥有完整 304 字节值对象而非匿名数组大小。
        self.assertIn("pub(crate) struct RhiBlurRasterParams", blur)
        # Drawing 只负责产生归一化核，字段排列必须交给共享构造器。
        self.assertIn("RhiBlurRasterParams::new(", renderer)
        # 禁止 Drawing 恢复手工维护的 76-float ABI 数组。
        self.assertNotIn("let mut values = [0.0f32; 76]", renderer)
        # PipelineContract 必须引用 Blur 自己的固定 ABI 大小。
        self.assertIn("Self::Blur => BLUR_UNIFORM_BYTES", pipeline)
        # 截取 OpenGL Blur 分支，排除其它 pipeline 的合法字段布局。
        opengl_blur = opengl_draw[
            # 从类型化 Blur 分派开始。
            opengl_draw.index("PipelineKind::BlurPass =>") :
            # 到共享固定状态开始编码的边界结束。
            opengl_draw.index("// SAFETY: contract 只包含固定 GL 采样覆盖映射", opengl_draw.index("PipelineKind::BlurPass =>"))
        ]
        # OpenGL 必须读取共享字段常量，不得保留数字偏移与独立权重数量。
        for field in (
            # 目标与 source 尺寸字段身份。
            "BLUR_SIZES_FLOAT_OFFSET",
            # 采样区域字段身份。
            "BLUR_REGION_FLOAT_OFFSET",
            # 方向与 tap 半径字段身份。
            "BLUR_DIRECTION_TAPS_FLOAT_OFFSET",
            # 权重起始字段身份。
            "BLUR_WEIGHTS_FLOAT_OFFSET",
            # 完整权重数量。
            "BLUR_WEIGHT_COUNT",
        ):
            # 每个共享字段都必须进入 Adapter 映射。
            self.assertIn(field, opengl_blur)
        # 禁止恢复三个匿名 header float4 偏移。
        for numeric_offset in (0, 4, 8):
            # 每个旧数字偏移都必须从 Blur 分支消失。
            self.assertNotIn(f"read_vec4(&uniform, {numeric_offset})", opengl_blur)
        # 禁止 Adapter 重新声明权重从第十二个 float 开始。
        self.assertNotIn("read_f32(&uniform, 12 + index)", opengl_blur)
        # 两套 shader 都必须保留完整十六个 float4 权重数组。
        self.assertIn("uniform vec4 u_weights[16];", opengl_shader)
        # D3D11 cbuffer 必须与同一共享总容量匹配。
        self.assertIn("float4 u_weights[16];", d3d11_shader)
    # MSDF 的 viewport、atlas extent 与距离范围必须由共享类型化 ABI 拥有。
    def test_msdf_uniform_is_shared_typed_and_api_neutral(self) -> None:
        # 读取共享 MSDF 值对象。
        msdf = (ROOT / "src/native/presentation/rhi/msdf.rs").read_text(encoding="utf-8")
        # 读取 Drawing 的 MSDF atlas 与 FramePlan lowering。
        renderer = (ROOT / "src/draw/backend/rhi_renderer_msdf.rs").read_text(encoding="utf-8")
        # 读取共享 pipeline 大小契约。
        pipeline = (ROOT / "src/native/presentation/rhi/pipeline.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 的共享字段映射。
        opengl_draw = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        # 读取 OpenGL MSDF 片元输出公式。
        opengl_shader = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs").read_text(encoding="utf-8")
        # 读取 D3D11 MSDF cbuffer 字段声明。
        d3d11_shader = (ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/msdf_shader.rs").read_text(encoding="utf-8")
        # 共享层必须拥有完整三十二字节值对象。
        self.assertIn("pub(crate) struct RhiMsdfRasterParams", msdf)
        # Drawing 必须把实际 atlas extent 作为类型化 RHI 尺寸传入共享构造器。
        self.assertIn("RhiMsdfRasterParams::new(viewport, texture_extent, quad.range)", renderer)
        # 禁止 Drawing 恢复手工维护的八 float 常量数组。
        self.assertNotIn("Self::encode_f32s(&[", renderer[renderer.index("fn msdf_uniform(") :])
        # PipelineContract 必须引用 MSDF 自己的固定 ABI 大小。
        self.assertIn("Self::Msdf => MSDF_UNIFORM_BYTES", pipeline)
        # 截取 OpenGL MSDF 分支，排除其它 pipeline 的合法字段布局。
        opengl_msdf = opengl_draw[
            # 从类型化 MSDF 分派开始。
            opengl_draw.index("PipelineKind::MsdfGlyphQuad =>") :
            # 到下一个 pipeline 分派结束。
            opengl_draw.index("PipelineKind::ShapeRect | PipelineKind::ShapeRectAdditive =>")
        ]
        # OpenGL 必须只按共享命名字段映射三个语义段。
        for field in (
            # viewport 字段身份。
            "MSDF_VIEWPORT_FLOAT_OFFSET",
            # atlas extent 字段身份。
            "MSDF_TEXTURE_SIZE_FLOAT_OFFSET",
            # 距离范围字段身份。
            "MSDF_RANGE_FLOAT_OFFSET",
        ):
            # 每个共享字段都必须进入 Adapter 映射。
            self.assertIn(field, opengl_msdf)
        # 禁止 Adapter 重新声明匿名 float 偏移。
        for numeric_offset in range(5):
            # 每个旧数字偏移都必须从 MSDF 分支消失。
            self.assertNotIn(f"read_f32(&uniform, {numeric_offset})", opengl_msdf)
        # D3D11 cbuffer 必须保持同一 viewport、texture extent 与 range 顺序。
        self.assertIn("float2 u_viewport;", d3d11_shader)
        # sampled atlas extent 必须紧随 viewport。
        self.assertIn("float2 u_tex_size;", d3d11_shader)
        # 距离范围必须占用第二个 float4 的首槽。
        self.assertIn("float u_range;", d3d11_shader)
        # 两套 shader 必须先把解析覆盖率冻结到同一个八位字节域。
        self.assertIn("float coverage_byte = floor(coverage * 255.0 + 0.5);", opengl_shader)
        # OpenGL alpha 必须只把字节颜色按字节覆盖率缩放一次。
        self.assertIn("float alpha = floor(color.a * coverage_byte / 255.0);", opengl_shader)
        # OpenGL 预乘颜色必须复用同一个量化覆盖率。
        self.assertIn("vec3 rgb = floor(premul * coverage_byte / 255.0);", opengl_shader)
        # 禁止恢复会把 MSDF 输出推到饱和的重复 255 倍缩放。
        self.assertNotIn("color.a * coverage * 255.0", opengl_shader)
        # D3D11 必须保持同一覆盖率量化与 alpha 缩放顺序。
        self.assertIn("float alpha = floor(color.a * coverage_byte / 255.0);", d3d11_shader)
    # 基础 Mesh、sampled/coverage 与 Sector 常量也必须由共享类型化 ABI 拥有。
    def test_basic_primitive_uniforms_are_shared_and_typed(self) -> None:
        # 读取基础图元共享值对象。
        primitive = (ROOT / "src/native/presentation/rhi/primitive.rs").read_text(encoding="utf-8")
        # 读取 Drawing 到共享 ABI 的集中映射。
        uniform = (ROOT / "src/draw/backend/rhi_renderer_uniform.rs").read_text(encoding="utf-8")
        # 读取多个 painter-order 执行入口。
        renderer = (ROOT / "src/draw/backend/rhi_renderer.rs").read_text(encoding="utf-8")
        # 读取混合 painter-order 执行入口。
        mixed = (ROOT / "src/draw/backend/rhi_renderer_mixed.rs").read_text(encoding="utf-8")
        # 读取共享 pipeline 大小契约。
        pipeline = (ROOT / "src/native/presentation/rhi/pipeline.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 的共享字段映射。
        opengl_draw = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        # 三类常量必须各自拥有封闭值对象。
        for value_type in ("RhiMeshRasterParams", "RhiSampledRasterParams", "RhiSectorRasterParams"):
            # 每个值对象都必须在 API 无关的 RHI 层声明。
            self.assertIn(f"pub(crate) struct {value_type}", primitive)
            # Drawing 的集中映射必须通过共享构造器编码。
            self.assertIn(f"{value_type}::new(", uniform)
        # Pipeline 大小必须引用值对象拥有的命名常量。
        for layout, size in (("Mesh", "MESH_UNIFORM_BYTES"), ("Sampled", "SAMPLED_UNIFORM_BYTES"), ("Sector", "SECTOR_UNIFORM_BYTES")):
            # 禁止 pipeline 重新用 float 数量推导 ABI。
            self.assertIn(f"Self::{layout} => {size}", pipeline)
        # 独立和混合 solid 路径必须复用同一个 Mesh 映射。
        self.assertIn("FrameUniformPayload::Mesh(Self::mesh_uniform(viewport, mesh.rgba))", renderer)
        # 混合路径必须复用集中 Mesh 映射。
        self.assertIn("FrameUniformPayload::Mesh(RhiRenderer::mesh_uniform(", mixed)
        # 混合路径的上传纹理、Picture 与 coverage 必须复用同一 sampled 映射。
        self.assertEqual(mixed.count("FrameUniformPayload::Sampled(RhiRenderer::sampled_uniform(viewport))"), 3)
        # Sector 必须通过共享构造器接收语义字段，不得手工拼十六个 float。
        self.assertIn("FrameUniformPayload::Sector(RhiRenderer::sector_uniform(", mixed)
        # 截取 OpenGL 基础分支并确认只按共享字段名解码。
        for field in (
            # Mesh viewport 字段身份。
            "MESH_VIEWPORT_FLOAT_OFFSET",
            # Mesh color 字段身份。
            "MESH_COLOR_FLOAT_OFFSET",
            # sampled/coverage viewport 字段身份。
            "SAMPLED_VIEWPORT_FLOAT_OFFSET",
            # Sector viewport 字段身份。
            "SECTOR_VIEWPORT_FLOAT_OFFSET",
            # Sector rect 字段身份。
            "SECTOR_RECT_FLOAT_OFFSET",
            # Sector color 字段身份。
            "SECTOR_COLOR_FLOAT_OFFSET",
            # Sector angles 字段身份。
            "SECTOR_ANGLES_FLOAT_OFFSET",
        ):
            # OpenGL Adapter 必须消费共享字段身份。
            self.assertIn(field, opengl_draw)
    # FramePlan 上传必须保持类型化，直到唯一 Device 执行边界才编码为字节。
    def test_frame_plan_uploads_stay_typed_until_device_execution(self) -> None:
        # 读取 FramePlan 的命令闭集。
        frame_plan = (ROOT / "src/draw/backend/frame_plan.rs").read_text(encoding="utf-8")
        # 读取 FramePlan 拥有的类型化上传载荷。
        upload = (ROOT / "src/draw/backend/frame_plan_upload.rs").read_text(encoding="utf-8")
        # 读取 FramePlan 到 Device 的唯一执行边界。
        execution = (ROOT / "src/draw/backend/frame_plan_execution.rs").read_text(encoding="utf-8")
        # 读取全部 Drawing 后端源码以排除旧裸字节命令。
        drawing = rust_sources(DRAW_ROOT)
        # 顶点上传必须由封闭布局枚举拥有。
        self.assertIn("pub(crate) enum FrameVertexPayload", upload)
        # Uniform 上传必须由封闭语义枚举拥有。
        self.assertIn("pub(crate) enum FrameUniformPayload", upload)
        # 每一种当前 raster 语义都必须拥有独立变体。
        for variant in ("Mesh", "Sampled", "Gradient", "Shape", "Shadow", "Blur", "Msdf", "Sector"):
            # 变体必须包装对应的共享 RHI 值对象。
            self.assertIn(f"{variant}(Rhi{variant}RasterParams)", upload)
        # FramePlan 必须区分顶点与 Uniform 上传。
        self.assertIn("UploadVertex {", frame_plan)
        # Uniform 上传必须是独立命令，不能混成无语义字节。
        self.assertIn("UploadUniform {", frame_plan)
        # FramePlan 自身不得再保存裸字节切片。
        self.assertNotIn("Arc<[u8]>", frame_plan)
        # 旧的无类型上传命令必须完全退出 Drawing。
        self.assertNotIn("FramePlanCommand::UpdateBuffer", drawing)
        # 顶点只允许在唯一执行器中编码。
        self.assertIn("data.encode_ne_bytes()", execution)
        # 编码后的短生命期字节只能进入不可拆的 Device 上传值对象。
        self.assertIn("RhiBufferUpload::new(*buffer, &bytes)", execution)

    # Pipeline 原生句柄与共享语义必须不可拆，并在 FramePlan 前置核对上传布局。
    def test_pipeline_binding_closes_frame_plan_layout_contract(self) -> None:
        # 读取共享 pipeline 身份与 ABI 契约。
        pipeline = (ROOT / "src/native/presentation/rhi/pipeline.rs").read_text(encoding="utf-8")
        # 读取只接受绑定 pipeline 的 draw packet。
        draw_packet = (ROOT / "src/native/presentation/rhi/draw_packet.rs").read_text(encoding="utf-8")
        # 读取 Device 创建与销毁的公共边界。
        rhi = (ROOT / "src/native/presentation/rhi/mod.rs").read_text(encoding="utf-8")
        # 读取 FramePlan 的前序上传布局验证组件。
        validation = (ROOT / "src/draw/backend/frame_plan_validation.rs").read_text(encoding="utf-8")
        # 读取 FramePlan 验证入口。
        frame_plan = (ROOT / "src/draw/backend/frame_plan.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 最终资源表核对。
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs").read_text(encoding="utf-8") + (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        # 读取 D3D11 最终资源表核对。
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs").read_text(encoding="utf-8") + (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_draw.rs").read_text(encoding="utf-8")
        # PipelineBinding 必须原子保存类型本身、opaque handle 与共享 kind。
        self.assertTrue(all(marker in pipeline for marker in ("pub(crate) struct PipelineBinding", "handle: PipelineHandle", "kind: PipelineKind")))
        # DrawPacket 必须私有保存完整绑定并只提供只读投影。
        self.assertTrue(all(marker in draw_packet for marker in ("pipeline: PipelineBinding", "buffers: DrawBufferBindings", "pub(crate) const fn pipeline(self) -> PipelineBinding", "pub(crate) const fn buffers(self) -> DrawBufferBindings")))
        # DrawPacket 不得重新暴露 crate 内字段写权限。
        self.assertTrue(all(marker not in draw_packet for marker in ("pub(crate) pipeline: PipelineBinding", "pub(crate) buffers: DrawBufferBindings")))
        # Device 创建边界必须直接返回绑定身份。
        self.assertIn("fn create_pipeline(&mut self, _desc: PipelineDesc) -> Result<PipelineBinding>", rhi)
        # FramePlan 必须在结构验证阶段调用布局核对。
        self.assertIn("validation::validate_draw_uploads(pass, command_index, *packet)?", frame_plan)
        # 布局验证必须读取绑定身份的唯一共享契约。
        self.assertIn("let contract = packet.pipeline().contract()", validation)
        # 顶点上传必须与 pipeline 顶点布局比较。
        self.assertIn("data.layout() != contract.vertex", validation)
        # Uniform 上传必须与 pipeline 常量布局比较。
        self.assertIn("uniform.layout() != contract.uniform", validation)
        # OpenGL 必须持有共享表并用完整 binding 解析 draw 身份。
        self.assertTrue(all(marker in opengl for marker in ("RhiPipelineResourceTable", "self.pipeline(packet.pipeline())?")))
        # D3D11 必须持有同一共享表并用完整 binding 解析 draw 身份。
        self.assertTrue(all(marker in d3d11 for marker in ("RhiPipelineResourceTable", "self.rhi_device.pipeline(packet.pipeline())?")))
        # 两个 Adapter 必须原子取得 DrawPacket 的 Buffer 角色。
        self.assertIn("packet.buffers()", opengl + d3d11)
        # 两个 Adapter 都不得恢复任何 DrawPacket 私有字段直读模式。
        self.assertTrue(all(marker not in opengl + d3d11 for marker in ("packet.pipeline.contract", "packet.pipeline.kind", "packet.pipeline;", "packet.buffers;", "packet.range;", "packet.range.index_binding")))
        # 两个 Adapter 都不得重新拥有带平台名称的 kind 错配错误。
        self.assertTrue(all("pipeline binding kind is stale" not in adapter for adapter in (opengl, d3d11)))

    # 颜色纹理在进入 shader 前必须由 Adapter 归一化为同一逻辑 RGBA 语义。
    def test_color_texture_channels_are_normalized_at_adapter_boundary(self) -> None:
        # 读取 OpenGL 的 CPU texture 上传边界。
        upload = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_upload.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 的颜色纹理采样 shader。
        shader = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs").read_text(encoding="utf-8")
        # BGRA CPU 载荷必须在原生 Adapter 内转换，不能让 shader 猜测来源布局。
        self.assertIn(
            "normalize_upload_payload(texture.desc.format(), validated.data())", upload
        )
        # 颜色采样必须像 D3D11 一样直接消费逻辑 RGBA 值。
        self.assertIn("vec4 sample_color = texture(u_tex, v_uv);", shader)
        # 禁止重新引入只对上传图片成立、却会破坏 render target 的 shader 通道交换。
        self.assertNotIn("texture(u_tex, v_uv).bgra", shader)

    # 纹理复制与移动必须共享同一格式、范围、资源关系和左上原点契约。
    def test_texture_transfer_contract_is_shared_and_top_left(self) -> None:
        # 读取 RHI 组合入口。
        rhi = (ROOT / "src/native/presentation/rhi/mod.rs").read_text(encoding="utf-8")
        # 读取共享纹理传输契约。
        transfer = (ROOT / "src/native/presentation/rhi/transfer.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 普通复制实现。
        opengl_copy = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 重叠安全移动实现。
        opengl_move = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_copy.rs").read_text(encoding="utf-8")
        # 读取 D3D11 的复制与移动实现。
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs").read_text(encoding="utf-8")
        # RHI 必须显式装配唯一传输契约。
        self.assertIn("mod transfer;", rhi)
        # 共享类型化区域必须拥有唯一 checked_add 边界算法。
        self.assertIn(".checked_add(self.extent.width)", transfer)
        # 普通 copy 必须拒绝同资源并引导到 move 语义。
        self.assertIn("RHI texture copy requires different resources", transfer)
        # R8 必须在两个 Adapter 都没有共同 render-target copy 基线时被统一拒绝。
        self.assertIn("RHI texture transfer requires a renderable color format", transfer)
        # OpenGL 普通复制必须调用共享门禁。
        self.assertIn("copy.validate_transfer(source_desc, destination_desc)?;", opengl_copy)
        # OpenGL move 必须调用同一共享门禁。
        self.assertIn("movement.validate_transfer(", opengl_move)
        # D3D11 copy 与 move 都必须调用共享门禁。
        self.assertEqual(d3d11.count("validate_transfer("), 2)
        # Adapter 不得恢复会隐藏整数溢出的饱和边界算法。
        self.assertNotIn("saturating_add", opengl_copy + d3d11)
        # OpenGL 离屏 copy 必须消费共享类型化原生投影。
        self.assertEqual(opengl_copy.count("native_origin_and_size_i32()"), 2)
        # OpenGL Adapter 不得再直接强转拆散的 copy 字段。
        self.assertNotIn("copy.source_", opengl_copy)
        # OpenGL 不得再把目标离屏 Y 当作 window surface 行序翻转。
        self.assertNotIn("destination.extent.height as i32 -", opengl_copy)
        # OpenGL 不得再把源离屏 Y 当作 window surface 行序翻转。
        self.assertNotIn("source.extent.height as i32 -", opengl_copy)

    # 颜色编码与混合值域必须由共享 RHI 冻结，原生 Adapter 不得启用隐式 sRGB 转换。
    def test_color_transfer_and_blend_domain_are_shared(self) -> None:
        # 读取共享颜色契约。
        color = (ROOT / "src/native/presentation/rhi/color.rs").read_text(encoding="utf-8")
        # 读取 Device 与 Surface 已拆分的 capability 值。
        capabilities = (ROOT / "src/native/presentation/rhi/capabilities.rs").read_text(encoding="utf-8")
        # 读取 OpenGL texture 格式映射。
        opengl_texture = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_resources.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 混合状态映射。
        opengl_draw = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        # 读取 Linux EGL surface 配置。
        egl = (ROOT / "src/native/presentation/graphics/opengl/adapter/egl.rs").read_text(encoding="utf-8")
        # 读取 Windows WGL surface 配置。
        wgl = (ROOT / "src/native/presentation/graphics/opengl/adapter/wgl.rs").read_text(encoding="utf-8")
        # 读取 D3D11 texture 格式映射。
        d3d11_texture = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 D3D11 swapchain 格式映射。
        d3d11_surface = (ROOT / "src/native/presentation/graphics/d3d11/adapter/swapchain.rs").read_text(encoding="utf-8")
        # 共享层必须命名 RGB 编码，不能把 Unorm 当作完整颜色语义。
        self.assertIn("pub(crate) enum RhiRgbEncoding", color)
        # 共享层必须命名混合运算值域。
        self.assertIn("pub(crate) enum RhiBlendDomain", color)
        # Texture 与 Surface 必须投影同一唯一颜色契约。
        self.assertIn("pub(crate) const UIX_COLOR_CONTRACT", color)
        # capability profile 必须把颜色契约交给每个 Adapter。
        self.assertIn("color_contract: UIX_COLOR_CONTRACT", capabilities)
        # OpenGL 颜色纹理必须使用普通 RGBA8，避免采样和写入时发生隐藏传递函数转换。
        self.assertIn("glow::RGBA8 as i32", opengl_texture)
        # OpenGL 混合必须直接机械翻译共享因子。
        self.assertIn("let state = blend.state();", opengl_draw)
        # OpenGL Adapter 家族不得私自启用 framebuffer sRGB 转换。
        self.assertNotIn("FRAMEBUFFER_SRGB", opengl_texture + opengl_draw + egl + wgl)
        # EGL/WGL surface 不得私自声明另一套 sRGB colorspace。
        self.assertNotIn("GL_COLORSPACE", egl + wgl)
        # D3D11 颜色纹理必须使用非 sRGB UNORM view。
        self.assertIn("DXGI_FORMAT_B8G8R8A8_UNORM", d3d11_texture)
        # D3D11 swapchain 必须使用同一非 sRGB UNORM surface。
        self.assertIn("DXGI_FORMAT_B8G8R8A8_UNORM", d3d11_surface)
        # D3D11 Adapter 不得私自使用带隐式转换的 sRGB view。
        self.assertNotIn("_SRGB", d3d11_texture + d3d11_surface)

    # Viewport 与 scissor 的目标范围必须由共享 RHI 值契约统一判定。
    def test_target_geometry_validation_is_shared_across_adapters(self) -> None:
        # 读取独立共享 RHI 几何 Component。
        geometry = (ROOT / "src/native/presentation/rhi/geometry.rs").read_text(encoding="utf-8")
        # 读取唯一拥有目标范围门禁的共享 pass 状态机。
        pass_state = (ROOT / "src/native/presentation/rhi/pass_state.rs").read_text(encoding="utf-8")
        # 读取 DrawPacket 独占的动态栅格值对象。
        draw_raster = (ROOT / "src/native/presentation/rhi/draw_raster.rs").read_text(encoding="utf-8")
        # 读取 OpenGL ES 的 pass 状态翻译。
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 D3D11 的 pass 状态翻译。
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_state.rs").read_text(encoding="utf-8")
        # Viewport 与 scissor 必须各自拥有一个共享目标边界方法。
        self.assertEqual(geometry.count("fits_within(self, extent: RhiExtent) -> bool"), 2)
        # 物理 viewport 必须在共享层冻结为整像素，禁止 Adapter 各自量化。
        self.assertIn("value.fract() != 0.0", geometry)
        # Draw 栅格值对象必须统一组合 viewport 与 scissor 的目标边界。
        self.assertIn("self.viewport.fits_within(extent)", draw_raster)
        self.assertIn("scissor.fits_within(extent)", draw_raster)
        # pass 状态机必须一次验证当前 packet 的完整栅格事实。
        self.assertIn("raster.fits_within(extent)", pass_state)
        # OpenGL ES 必须委托共享 Draw 栅格门禁。
        self.assertIn("self.pass.validate_draw_raster(raster)?;", opengl)
        # OpenGL ES 不得在 Adapter 内私自 round 成与 D3D11 不同的 viewport。
        self.assertNotIn("viewport.width.round()", opengl)
        # D3D11 必须委托完全相同的共享 Draw 栅格门禁。
        self.assertIn("self.rhi_device.pass.validate_draw_raster(raster)?;", d3d11)
        # OpenGL ES 每次 Draw 都必须投影 packet 自有 scissor。
        self.assertIn("self.apply_scissor(gl, raster.scissor())?;", opengl)
        # D3D11 每次 Draw 也必须投影同一个 packet scissor。
        self.assertIn("let scissor = raster.scissor();", d3d11)
        # 两套 Adapter 都不得恢复平行的目标范围算法。
        self.assertNotIn("fits_within(extent)", opengl + d3d11)

    # 局部清屏必须复用共享目标边界，Adapter 只保留原生清理编码差异。
    def test_clear_rect_reuses_shared_target_geometry_contract(self) -> None:
        # 读取唯一拥有局部清理门禁的共享 pass 状态机。
        pass_state = (ROOT / "src/native/presentation/rhi/pass_state.rs").read_text(encoding="utf-8")
        # 读取 OpenGL ES 的局部清屏翻译。
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_clear.rs").read_text(encoding="utf-8")
        # 读取 D3D11 的局部清屏翻译。
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_clear.rs").read_text(encoding="utf-8")
        # 共享状态机必须拥有唯一目标边界判定。
        self.assertIn("scissor.fits_within(extent)", pass_state)
        # OpenGL ES 必须把颜色和矩形一起委托给共享门禁。
        self.assertIn("self.pass.validate_clear(color, scissor)?;", opengl)
        # D3D11 ClearView 必须使用完全相同的共享门禁。
        self.assertIn("self.rhi_device.pass.validate_clear(color, scissor)?;", d3d11)
        # OpenGL ES 不得自行用 checked_add 定义边界语义。
        self.assertNotIn("checked_add", opengl)
        # D3D11 不得自行用 checked_add 定义另一套边界语义。
        self.assertNotIn("checked_add", d3d11)
        # OpenGL ES 局部清理只编码本命令矩形，不恢复历史裁剪。
        self.assertIn("self.apply_scissor(gl, Some(scissor))?;", opengl)
        self.assertNotIn("previous", opengl)
        # D3D11 ClearView 接收独立矩形，不得修改 raster scissor 状态。
        self.assertNotIn("rhi_set_scissor", d3d11)

    # 清屏颜色必须在共享 RHI 边界成为预乘值，Adapter 只能读取通道。
    def test_clear_color_premultiplication_has_one_shared_owner(self) -> None:
        # 读取共享 RHI 颜色值对象。
        rhi = (ROOT / "src/native/presentation/rhi/mod.rs").read_text(encoding="utf-8")
        # 读取 FrameEncoder 到 FramePlan 的清屏 lowering。
        lowering = (ROOT / "src/draw/backend/gpu/backend/rhi_frame.rs").read_text(encoding="utf-8")
        # 读取 OpenGL ES 的 pass 与局部清理 Adapter。
        opengl = (
            # 合并完整 pass 清理实现。
            (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs").read_text(encoding="utf-8")
            # 合并局部清理实现。
            + (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_clear.rs").read_text(encoding="utf-8")
        )
        # 读取 D3D11 的 pass 与局部清理 Adapter。
        d3d11 = (
            # 合并完整 pass 清理实现。
            (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs").read_text(encoding="utf-8")
            # 合并局部清理实现。
            + (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_clear.rs").read_text(encoding="utf-8")
        )
        # 颜色通道必须保持私有，禁止任一 Adapter 绕过值对象构造。
        self.assertIn("pub(crate) struct RhiColor([f32; 4]);", rhi)
        # 共享值对象必须拥有 straight 到 premultiplied 的唯一转换。
        self.assertIn("pub(crate) fn from_straight_rgba", rhi)
        # 共享校验必须明确冻结 RGB 不得大于 alpha。
        self.assertIn("&& red <= alpha", rhi)
        # FrameEncoder 清屏必须调用共享转换，而不是直接包装 straight 颜色。
        self.assertIn("RhiColor::from_straight_rgba(color_rgba(color))", lowering)
        # Drawing System 中不得恢复 RhiColor tuple 直接构造。
        for path, source in rust_sources(DRAW_ROOT):
            # 使用子测试报告具体越界构造文件。
            with self.subTest(path=str(path.relative_to(ROOT))):
                # 所有生产和测试调用都必须经过具名语义构造器。
                self.assertNotIn("RhiColor(", source)
        # OpenGL ES 只能读取共享预乘通道。
        self.assertIn("color.components()", opengl)
        # D3D11 必须读取同一共享预乘通道。
        self.assertIn("color.components()", d3d11)
        # 两套 Adapter 都不得重新乘 alpha。
        self.assertNotIn("* alpha", opengl + d3d11)

    # UI 背景图层必须在进入 Drawing 原语前冻结组件背景盒裁剪。
    def test_radial_background_is_clipped_by_ui_box_semantics(self) -> None:
        # 读取 UI System 私有的背景图层映射。
        background = (ROOT / "src/ui/style_paint/background.rs").read_text(encoding="utf-8")
        # 截取径向背景分支，避免图片路径中的裁剪掩盖回归。
        radial = background[
            # 从径向分支开始。
            background.index("ResolvedBackground::Radial(inner, outer) =>") :
            # 到 match 分支结束后的函数边界。
            background.index("    }\n}\n\n/// 生成与背景盒相交", background.index("ResolvedBackground::Radial(inner, outer) =>"))
        ]
        # 径向原语提交前必须压入当前背景盒。
        self.assertLess(radial.index("ctx.push_clip(rect);"), radial.index("ctx.fill_radial_gradient("))
        # 原语提交后必须恢复裁剪栈。
        self.assertLess(radial.index("ctx.fill_radial_gradient("), radial.index("ctx.pop_clip();"))


# 允许直接运行本文件进行最小架构验证。
if __name__ == "__main__":
    # 交给标准 unittest runner 执行。
    unittest.main()
