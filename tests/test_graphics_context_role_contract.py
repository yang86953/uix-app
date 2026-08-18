# 声明本文件锁定组合 RHI 的 Device 与 Surface 角色边界。
"""锁定组合 RHI 上下文只暴露 Device 与 Surface 窄角色。"""

# 引入标准路径工具读取当前仓库契约源码。
from pathlib import Path
# 引入标准单元测试框架表达静态架构断言。
import unittest

# 解析仓库根目录，避免测试依赖调用方工作目录。
ROOT = Path(__file__).resolve().parents[1]


# 校验组合根、FramePlan 与 Drawing 调用方不会重新混合两类职责。
class GraphicsContextRoleContractTests(unittest.TestCase):
    # 校验组合契约本身不再继承任何角色。
    def test_context_is_a_pure_composition_contract(self) -> None:
        # 读取薄 RHI 角色定义。
        rhi = (ROOT / "src/native/present/rhi.rs").read_text(encoding="utf-8")
        # 截取组合根契约定义与 blanket 实现之间的正文。
        context_contract = rhi[
            # 从组合 trait 的唯一声明开始。
            rhi.index("pub(crate) trait GraphicsContextRhi") :
            # 在 blanket 实现前结束。
            rhi.index("impl<T> GraphicsContextRhi for T")
        ]
        # 组合根不得通过 supertrait 泄露 Device 或 Surface 原语。
        self.assertIn("pub(crate) trait GraphicsContextRhi {", context_contract)
        # 只读 Device 查询必须经过显式窄视图。
        self.assertIn("fn device_ref(&self) -> &dyn GraphicsDevice;", context_contract)
        # 可变 Device 命令必须经过显式窄视图。
        self.assertIn("fn device(&mut self) -> &mut dyn GraphicsDevice;", context_contract)
        # 只读 Surface 查询必须经过显式窄视图。
        self.assertIn("fn surface_ref(&self) -> &dyn GraphicsSurface;", context_contract)
        # 可变 Surface 操作必须经过显式窄视图。
        self.assertIn("fn surface(&mut self) -> &mut dyn GraphicsSurface;", context_contract)

    # 校验 Surface FramePlan 事务按角色阶段执行。
    def test_surface_transaction_borrows_one_role_at_a_time(self) -> None:
        # 读取唯一 FramePlan 执行组件。
        execution = (ROOT / "src/draw/backend/frame_plan_execution.rs").read_text(
            # 明确使用 UTF-8 读取中文注释与 Rust 源码。
            encoding="utf-8"
        )
        # 截取最终呈现事务，排除后续离屏入口。
        transaction = execution[
            # 从带 present 钩子的事务开始。
            execution.index("pub(crate) fn execute_on_context_with_before_present(") :
            # 在 Device-only 离屏入口前结束。
            execution.index("pub(crate) fn execute_offscreen_on_device")
        ]
        # acquire 只能经 Surface 角色发生。
        self.assertIn("let frame = context.surface().acquire()?;", transaction)
        # 命令执行器只能取得 Device 角色。
        self.assertIn("FramePlanExecutor::for_surface(context.device(), frame.target())", transaction)
        # submit 只能经 Device 角色发生。
        self.assertIn("let submission = context.device().submit()?;", transaction)
        # present 前后的代际检查只能读取 Surface 角色。
        self.assertGreaterEqual(transaction.count("context.surface_ref().token()"), 3)
        # 最终 present 必须显式取得 Surface 角色。
        self.assertIn(".surface()", transaction)
        # 最终 present 必须消费绑定同一 frame、submission 与 damage 的事务。
        self.assertIn(".present(RhiPresentTransaction::new(", transaction)
        # submit-before-present 观察器只能取得窄 Surface 角色。
        self.assertIn("context.surface(),", transaction)
        # 观察器不得重新取得完整组合 context。
        self.assertNotIn("before_present(context)", transaction)
        # 禁止重新直接穿透组合上下文执行 acquire。
        self.assertNotIn("context.acquire()", transaction)
        # 禁止重新直接穿透组合上下文执行 submit。
        self.assertNotIn("context.submit()", transaction)
        # 禁止重新直接穿透组合上下文执行 present。
        self.assertNotIn("context.present(", transaction)

    # 校验通用执行门面不会把组合上下文交给离屏 Device 入口。
    def test_offscreen_execution_receives_only_device_role(self) -> None:
        # 读取 Drawing 到 FramePlan 的统一执行门面。
        renderer_execution = (
            # 拼出执行门面文件的绝对路径。
            ROOT / "src/draw/backend/rhi_renderer_execution.rs"
            # 读取当前源码。
        ).read_text(encoding="utf-8")
        # Offscreen 分派路径不得再持有组合 context 后临时取 Device。
        self.assertNotIn("plan.execute_offscreen_on_device(context.device())?;", renderer_execution)
        # 封闭 Offscreen 变体必须直接进入 Device-only 执行入口。
        self.assertIn("execute_plan_without_present(&mut **device, plan)", renderer_execution)
        # no-present 入口收到的参数本身已经是 Device。
        self.assertEqual(
            # 统计直接使用窄 Device 参数的执行形式。
            renderer_execution.count("plan.execute_offscreen_on_device(device)?;"),
            # 显式 no-present 入口只有一处 Device-only 提交。
            1,
        )
        # 禁止恢复把完整组合上下文传给离屏入口的旧形式。
        self.assertNotIn("plan.execute_offscreen_on_device(context)?;", renderer_execution)

    # 校验所有明确 no-present 的 helper 都无法取得 Surface 角色。
    def test_no_present_helpers_are_device_only_by_signature(self) -> None:
        # 读取统一 no-present 执行门面。
        renderer_execution = (
            # 拼出执行门面文件路径。
            ROOT / "src/draw/backend/rhi_renderer_execution.rs"
            # 读取当前源码。
        ).read_text(encoding="utf-8")
        # 截取 no-present 函数正文。
        no_present = renderer_execution[
            # 从唯一 no-present 入口开始。
            renderer_execution.index("pub(super) fn execute_plan_without_present(") :
        ]
        # no-present 入口必须只接收 Device 角色。
        self.assertIn("device: &mut dyn GraphicsDevice", no_present)
        # no-present 正文不得再借用组合 context。
        self.assertNotIn("context: &mut dyn GraphicsContextRhi", no_present)
        # 读取完全离屏的 Blur lowering。
        blur = (ROOT / "src/draw/backend/rhi_renderer_blur.rs").read_text(encoding="utf-8")
        # Blur 模块不得依赖组合上下文类型。
        self.assertNotIn("GraphicsContextRhi", blur)
        # Blur 最终目标必须是无法表示 Surface 的类型化 texture handle。
        self.assertIn("target: TextureHandle", blur)
        # Blur 计划必须直接建立 Offscreen 作用域。
        self.assertIn("let mut plan = super::FramePlan::offscreen();", blur)
        # 离屏 Blur helper 不得再接收或伪造最终 present damage。
        self.assertNotIn("PresentDamage", blur)
        # 读取单个已有纹理的无 present sampled lowering。
        sampled = (ROOT / "src/draw/backend/rhi_renderer_sampled.rs").read_text(encoding="utf-8")
        # 截取无 present sampled helper 的参数与正文。
        sampled_offscreen = sampled[sampled.index("pub(crate) fn execute_sampled_quad_without_present(") :]
        # sampled 离屏 helper 必须直接取得 Device 角色。
        self.assertIn("device: &mut dyn GraphicsDevice", sampled_offscreen)
        # sampled 离屏 helper 必须取得无法表示 Surface 的 texture 句柄。
        self.assertIn("target: TextureHandle", sampled_offscreen)
        # sampled 离屏 helper 不得重新取得组合 context。
        self.assertNotIn("context: &mut dyn GraphicsContextRhi", sampled_offscreen)
        # sampled 离屏 helper 不得接收最终 present damage。
        self.assertNotIn("damage: PresentDamage", sampled_offscreen)
        # 读取 FramePlan 自有的封闭执行作用域。
        plan = (ROOT / "src/draw/backend/frame_plan.rs").read_text(encoding="utf-8")
        # 截取唯一作用域类型，单独核对 Surface 所有的呈现事实。
        scope_contract = plan[plan.index("enum FramePlanScope") : plan.index("pub(crate) struct FramePlan")]
        # Surface 变体必须原子拥有代际与最终 damage。
        self.assertIn("Surface {", scope_contract)
        # 最终 damage 只能由 Surface 作用域保存。
        self.assertIn("damage: PresentDamage", scope_contract)
        # 截取 FramePlan 顶层状态，防止 present-only 数据重新泄漏为公共字段。
        frame_plan_state = plan[plan.index("pub(crate) struct FramePlan {") : plan.index("impl FramePlan {")]
        # FramePlan 顶层不得让 Offscreen 计划被迫携带最终 damage。
        self.assertNotIn("damage: PresentDamage", frame_plan_state)
        # Offscreen 构造器必须从类型上拒绝任何 present 参数。
        self.assertIn("pub(crate) fn offscreen() -> Self", plan)
        # 读取两条 retained TextureMove helper。
        frame = (ROOT / "src/draw/backend/gpu/backend/rhi_frame.rs").read_text(encoding="utf-8")
        # 读取通用 mixed lowering 的 TextureMove helper。
        lowering = (ROOT / "src/draw/backend/gpu/rhi_lowering.rs").read_text(encoding="utf-8")
        # 读取空 retained target 初始化 helper。
        soft = (ROOT / "src/draw/backend/gpu/backend/rhi_surface_soft.rs").read_text(encoding="utf-8")
        # FrameEncoder move boundary 只能取得 Device。
        self.assertIn("fn execute_frame_texture_move(\n    device: &mut dyn GraphicsDevice", frame)
        # 通用 mixed move boundary 只能取得 Device。
        self.assertIn("fn execute_texture_move(\n    device: &mut dyn GraphicsDevice", lowering)
        # 空 retained target 初始化只能取得 Device。
        self.assertIn("fn clear_empty_soft_target(\n    device: &mut dyn GraphicsDevice", soft)

    # 校验最终观察器与 soft staging 分别只取得 Surface 和 Device。
    def test_observer_and_soft_upload_use_orthogonal_roles(self) -> None:
        # 读取 FramePlan 的最终呈现事务。
        execution = (ROOT / "src/draw/backend/frame_plan_execution.rs").read_text(encoding="utf-8")
        # 读取 Renderer 封闭帧的观察钩子契约。
        renderer = (ROOT / "src/draw/backend/rhi_renderer_execution.rs").read_text(encoding="utf-8")
        # 读取 sampled 最终合成入口。
        sampled = (ROOT / "src/draw/backend/rhi_renderer_sampled.rs").read_text(encoding="utf-8")
        # 读取 test-harness 的 Drawing 回读边界。
        backend = (ROOT / "src/draw/backend/gpu/backend/impl_main.rs").read_text(encoding="utf-8")
        # 读取 retained/Picture soft staging helper。
        soft = (ROOT / "src/draw/backend/gpu/backend/rhi_surface_soft.rs").read_text(encoding="utf-8")
        # FramePlan 钩子必须只接收 Surface 角色。
        self.assertIn("before_present: &mut dyn FnMut(&mut dyn GraphicsSurface)", execution)
        # FramePlan 不得把完整组合 context 交给观察器。
        self.assertNotIn("before_present: &mut dyn FnMut(&mut dyn GraphicsContextRhi)", execution)
        # 封闭 Renderer Surface 帧必须保存同一个窄观察器类型。
        self.assertIn("FnMut(&mut dyn GraphicsSurface)", renderer)
        # sampled 最终合成入口必须延续 Surface-only 回调。
        self.assertIn("before_present: &mut dyn FnMut(&mut dyn GraphicsSurface)", sampled)
        # 截取 test-harness 回读 helper 的完整实现。
        readback = backend[backend.index("pub(super) fn try_readback(") : backend.index("pub(crate) fn last_soft_upload_bytes")]
        # 回读 helper 必须直接取得 Surface 角色。
        self.assertIn("surface: &mut dyn GraphicsSurface", readback)
        # 回读 helper 不得取得组合 context。
        self.assertNotIn("GraphicsContextRhi", readback)
        # 回读必须从同一个 Surface 读取 capability、token 与像素。
        self.assertIn("surface.surface_capabilities().readback", readback)
        # 回读 token 必须来自同一个窄角色。
        self.assertIn("let token = surface.token();", readback)
        # 像素读取必须直接使用同一个窄角色。
        self.assertIn("surface.read_surface_pixels(RhiScissor", readback)
        # 截取通用 soft staging helper，排除后续组合调用方。
        soft_upload = soft[soft.index("pub(super) fn try_upload_rhi_canvas_soft(") : soft.index("impl GpuBackend")]
        # soft helper 必须直接取得 Device。
        self.assertIn("device: &mut dyn GraphicsDevice", soft_upload)
        # soft helper 不得再取得组合 context。
        self.assertNotIn("GraphicsContextRhi", soft_upload)
        # soft 最终帧只能由 Device 构造 Offscreen 变体。
        self.assertIn("RhiRendererFrame::offscreen(device, target)", soft_upload)

    # 校验 GPU queue lowering 入口只能取得 Device、显式 extent 与纹理目标。
    def test_gpu_queue_lowering_is_device_only_by_signature(self) -> None:
        # 读取专用 solid、shape、shadow、glyph 与 textured lowering。
        submit = (ROOT / "src/draw/backend/gpu/submit.rs").read_text(encoding="utf-8")
        # 读取专用 gradient lowering。
        gradient = (ROOT / "src/draw/backend/gpu/rhi_gradient_submit.rs").read_text(encoding="utf-8")
        # 读取 mixed、scroll 与 retained clear lowering。
        lowering = (ROOT / "src/draw/backend/gpu/rhi_lowering.rs").read_text(encoding="utf-8")
        # 读取封闭 Renderer 帧构造器。
        execution = (ROOT / "src/draw/backend/rhi_renderer_execution.rs").read_text(encoding="utf-8")
        # 专用 queue lowering 不得取得组合 context。
        self.assertNotIn("GraphicsContextRhi", submit)
        # 专用 queue lowering 不得接收最终 present damage。
        self.assertNotIn("PresentDamage", submit)
        # 专用 queue lowering 不得接收可表示 Surface 的目标选择器。
        self.assertNotIn("RenderTargetRef", submit)
        # Gradient lowering 必须遵守同一 Device-only 边界。
        self.assertNotIn("GraphicsContextRhi", gradient)
        # Gradient lowering 不得接收最终 present damage。
        self.assertNotIn("PresentDamage", gradient)
        # Gradient lowering 不得接收可表示 Surface 的目标选择器。
        self.assertNotIn("RenderTargetRef", gradient)
        # Mixed lowering 不得重新取得组合 context。
        self.assertNotIn("GraphicsContextRhi", lowering)
        # Mixed lowering 不得接收最终 present damage。
        self.assertNotIn("PresentDamage", lowering)
        # Mixed 的公开目标参数必须是显式纹理句柄。
        self.assertNotIn("target: RenderTargetRef", lowering)
        # 三类 lowering 都必须从签名取得 Device 角色。
        for source in (submit, gradient, lowering):
            # Device 是资源、命令与 submit 的唯一 owner 视图。
            self.assertIn("device: &mut dyn GraphicsDevice", source)
            # extent 是统一物理几何的唯一范围事实。
            self.assertIn("extent: RhiExtent", source)
            # TextureHandle 从签名上排除主 Surface 目标。
            self.assertIn("target: TextureHandle", source)
        # 物理几何 helper 必须直接接收 API 无关的 extent。
        geometry = submit[submit.index("pub(crate) fn rhi_physical_geometry(") : submit.index("fn rhi_physical_scissor(")]
        # 几何换算不得隐式读取组合 Surface。
        self.assertNotIn("GraphicsContextRhi", geometry)
        # 几何换算必须显式取得目标范围。
        self.assertIn("extent: RhiExtent", geometry)
        # 删除会重新接收 target 与 damage 组合的过渡构造器。
        self.assertNotIn("fn for_target(", execution)

    # 校验 Renderer 资源缓存 helper 不再依赖组合上下文。
    def test_renderer_resource_helpers_accept_only_device_role(self) -> None:
        # 读取 Renderer 主资源缓存实现。
        renderer = (ROOT / "src/draw/backend/rhi_renderer.rs").read_text(encoding="utf-8")
        # 读取 coverage 专用资源实现。
        coverage = (ROOT / "src/draw/backend/rhi_renderer_coverage.rs").read_text(encoding="utf-8")
        # 读取 MSDF pipeline、atlas 与临时纹理实现。
        msdf = (ROOT / "src/draw/backend/rhi_renderer_msdf.rs").read_text(encoding="utf-8")
        # 读取 shape 固定资源实现。
        shape = (ROOT / "src/draw/backend/rhi_renderer_shape.rs").read_text(encoding="utf-8")
        # 读取 shadow 固定资源实现。
        shadow = (ROOT / "src/draw/backend/rhi_renderer_shadow.rs").read_text(encoding="utf-8")
        # 读取 mixed sector 固定资源实现。
        mixed = (ROOT / "src/draw/backend/rhi_renderer_mixed.rs").read_text(encoding="utf-8")
        # 列出所有只拥有 Device 资源生命周期的 helper。
        helpers = (
            # Solid pipeline 与 buffer 缓存。
            (renderer, "ensure_solid_resources"),
            # Sampled pipeline、buffer 与 sampler 缓存。
            (renderer, "ensure_textured_resources"),
            # Additive sampled pipeline 缓存。
            (renderer, "ensure_additive_textured_pipeline"),
            # Gradient pipeline 与 buffer 缓存。
            (renderer, "ensure_gradient_resources"),
            # 临时纹理清理。
            (renderer, "destroy_textures"),
            # Coverage pipeline 与 sampler 缓存。
            (coverage, "ensure_coverage_resources"),
            # MSDF pipeline 与 sampler 缓存。
            (msdf, "ensure_msdf_resources"),
            # MSDF atlas placement 与上传。
            (msdf, "ensure_msdf_texture"),
            # MSDF atlas page 创建。
            (msdf, "create_msdf_atlas_page"),
            # MSDF 临时纹理创建。
            (msdf, "create_transient_msdf_texture"),
            # Shape pipeline 与 buffer 缓存。
            (shape, "ensure_shape_resources"),
            # Shadow pipeline 与 buffer 缓存。
            (shadow, "ensure_shadow_resources"),
            # Sector pipeline 与 buffer 缓存。
            (mixed, "ensure_sector_resources"),
        )
        # 逐一锁定每个 helper 的参数角色。
        for source, name in helpers:
            # 找到当前 helper 的签名起点。
            start = source.index(f"fn {name}(")
            # 签名在函数体左花括号前结束。
            end = source.index("{", start)
            # 截取不含函数体的角色契约。
            signature = source[start:end]
            # 资源 helper 必须显式取得 Device。
            self.assertIn("device: &mut dyn GraphicsDevice", signature, name)
            # 资源 helper 不得重新取得组合上下文。
            self.assertNotIn("GraphicsContextRhi", signature, name)


# 允许直接运行该文件执行架构契约检查。
if __name__ == "__main__":
    # 交给 unittest 输出稳定结果与退出码。
    unittest.main()
