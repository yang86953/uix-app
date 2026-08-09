# -*- coding: utf-8 -*-
"""Keep graphics context metadata and Vulkan ownership contracts explicit."""

# 引入单元测试框架。
import unittest
# 引入跨平台路径工具。
from pathlib import Path


# 固定仓库根目录，供源码契约读取使用。
ROOT = Path(__file__).resolve().parents[1]
# 固定拆分后的 Vulkan context 组合模块位置。
VULKAN_CONTEXT = ROOT / "src/native/presentation/graphics/vulkan/platform/context"
# 固定 Vulkan 共享 device 管理实现位置。
VULKAN_DEVICE = ROOT / "src/native/presentation/graphics/vulkan/platform/device.rs"
# 固定 Vulkan fault 映射实现位置。
VULKAN_FAULT = ROOT / "src/native/presentation/graphics/vulkan/platform/fault.rs"
# 固定 GFX-R5 诊断模块位置。
GFX_R5 = ROOT / "src/gfx_r5_support"


# 组合读取拆分目录中的 Rust 源码，保持稳定的路径顺序。
def read_rust_module(path: Path) -> str:
    # 单文件模块直接按 UTF-8 读取。
    if path.is_file():
        # 返回单文件源码。
        return path.read_text(encoding="utf-8")
    # 目录模块按相对路径排序后拼接所有 Rust 文件。
    return "\n".join(
        # 读取当前 Rust 文件内容。
        source.read_text(encoding="utf-8")
        # 递归枚举目录内的 Rust 文件。
        for source in sorted(path.rglob("*.rs"))
    )


# 集中验证 context 元数据与 Vulkan 所有权边界。
class GraphicsContextContractTests(unittest.TestCase):
    # 校验 live drawable 元数据只通过单一 PresentSurface 快照传播。
    def test_context_drawable_metadata_is_atomic_present_surface(self) -> None:
        # 读取 context 能力模型与构造器。
        contracts = (ROOT / "src/native/present/mod.rs").read_text(encoding="utf-8")
        # 截取 GraphicsContextCaps 的字段定义。
        caps_start = contracts.index("pub struct GraphicsContextCaps")
        # 以构造器实现起点作为结构体字段终点。
        caps_end = contracts.index("impl GraphicsContextCaps", caps_start)
        # 保存只包含静态 recipe 事实的 capability 结构体片段。
        caps_struct = contracts[caps_start:caps_end]
        # 动态 DPR 不得继续伪装成 capability 字段。
        self.assertNotIn("device_pixel_ratio", caps_struct)
        # 读取统一 context trait。
        facade = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # context 必须显式提供完整 surface 快照。
        self.assertIn("fn present_surface(&self) -> PresentSurface;", facade)
        # 通用 trait 不得继续拆分暴露 drawable 宽度。
        self.assertNotIn("fn width(&self)", facade)
        # 通用 trait 不得继续拆分暴露 drawable 高度。
        self.assertNotIn("fn height(&self)", facade)
        # 通用 trait 不得继续拆分暴露 DPR。
        self.assertNotIn("fn device_pixel_ratio(&self)", facade)
        # RHI resize helper 必须从调用前取得的单一 surface 快照读取 DPR。
        self.assertIn("let device_pixel_ratio = present_surface.device_pixel_ratio;", facade)
        # 读取 owner-thread wrapper。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # 截取 wrapper 字段定义，避免把方法参数误判为缓存字段。
        bound_start = thread_bound.index("pub(crate) struct ThreadBoundGraphicsContext")
        # 以 wrapper 实现起点作为字段定义终点。
        bound_end = thread_bound.index("impl ThreadBoundGraphicsContext", bound_start)
        # 保存只包含 wrapper 缓存字段的片段。
        bound_struct = thread_bound[bound_start:bound_end]
        # wrapper 必须缓存一个完整 PresentSurface。
        self.assertIn("present_surface: PresentSurface", bound_struct)
        # wrapper 不得继续分开缓存 live width。
        self.assertNotIn("width: i32", bound_struct)
        # wrapper 不得继续分开缓存 live height。
        self.assertNotIn("height: i32", bound_struct)
        # wrapper 不得继续分开缓存 live DPR。
        self.assertNotIn("device_pixel_ratio: f32", bound_struct)
        # 生命周期变更后必须原子刷新 surface 快照。
        self.assertIn("self.present_surface = self.inner.present_surface();", thread_bound)
        # 逐个核对所有 context 实现都显式提供 surface 元数据。
        for adapter in (
            # D3D11 生产 context。
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs",
            # D3D12 测试期 context。
            ROOT / "src/native/presentation/graphics/d3d12/platform/context/graphics.rs",
            # WGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs",
            # EGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
            # Vulkan PixelUpload context。
            ROOT / "src/native/presentation/graphics/vulkan/platform/context/graphics.rs",
            # Metal PixelUpload context。
            ROOT / "src/native/presentation/graphics/metal/platform/context.rs",
            # GPU-native 测试 fake。
            ROOT / "src/native/test_harness/fake_graphics_context.rs",
        ):
            # 每个实现必须返回单一 PresentSurface 快照。
            adapter_source = adapter.read_text(encoding="utf-8")
            # 禁止依赖已删除的默认元数据拼装。
            self.assertIn("fn present_surface(&self)", adapter_source)
        # 读取 GPU backend 的 live metadata 派生边界。
        gpu_backend = (ROOT / "src/draw/backend/gpu/backend/mod.rs").read_text(encoding="utf-8")
        # GPU backend 必须从 PresentSurface 派生逻辑元数据。
        self.assertIn("logical_metadata_from_surface", gpu_backend)
        # 读取 GPU backend 构造、回读与 surface 采纳路径。
        gpu_lifecycle = (ROOT / "src/draw/backend/gpu/backend/impl_main.rs").read_text(encoding="utf-8")
        # GPU backend 不得重新读取分离的 context width。
        self.assertNotIn("gpu_ctx.width()", gpu_lifecycle)
        # GPU backend 不得重新读取分离的 context height。
        self.assertNotIn("gpu_ctx.height()", gpu_lifecycle)
        # 构造与 surface 采纳都必须显式读取完整快照。
        self.assertGreaterEqual(gpu_lifecycle.count("gpu_ctx.present_surface()"), 3)
        # 读取 PixelUpload runtime。
        runtime = (ROOT / "src/draw/renderer/runtime.rs").read_text(encoding="utf-8")
        # PixelUpload runtime 必须从单一 surface 快照读取物理宽度。
        self.assertIn("present_surface.drawable_width", runtime)
        # PixelUpload runtime 必须从单一 surface 快照读取物理高度。
        self.assertIn("present_surface.drawable_height", runtime)
        # PixelUpload runtime 不得恢复分离的 context width 查询。
        self.assertNotIn("upload.context.width()", runtime)
        # PixelUpload runtime 不得恢复分离的 context height 查询。
        self.assertNotIn("upload.context.height()", runtime)

    # 校验 recipe 派生查询不会继续扩张通用 context 门面。
    def test_context_recipe_queries_are_not_duplicated_by_adapters(self) -> None:
        # 读取统一 context trait。
        facade = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # backend 身份只通过静态 caps 快照读取。
        self.assertNotIn("fn graphics_backend(&self)", facade)
        # 无消费者的 GL proc 查询不得保留在生产门面。
        self.assertNotIn("fn supports_gl_proc_address(&self)", facade)
        # 无消费者的 PixelUpload 查询不得保留在生产门面。
        self.assertNotIn("fn supports_pixel_present(&self)", facade)
        # 原子 GPU recipe 已证明 owner 完整，resize helper 不再重复读取 backend 身份。
        self.assertNotIn("context.caps().backend", facade)
        # 逐个核对曾重复声明 backend 的 adapter 已删除派生实现。
        for adapter in (
            # D3D11 生产 context。
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs",
            # WGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs",
            # EGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
            # Vulkan PixelUpload context。
            ROOT / "src/native/presentation/graphics/vulkan/platform/context/graphics.rs",
            # GPU-native 测试 fake。
            ROOT / "src/native/test_harness/fake_graphics_context.rs",
        ):
            # adapter 只能在 caps 构造中陈述 backend 身份。
            adapter_source = adapter.read_text(encoding="utf-8")
            # 禁止重新引入重复的 trait 方法实现。
            self.assertNotIn("fn graphics_backend(&self)", adapter_source)

    # 校验生产 context 构造只接受完整 recipe 与运行时故障队列。
    def test_internal_context_factory_has_no_single_backend_shortcut(self) -> None:
        # 读取 native factory 的公开组合入口。
        factory = (ROOT / "src/native/factory/mod.rs").read_text(encoding="utf-8")
        # 读取 registry 的 recipe 校验与构造实现。
        registry = (ROOT / "src/native/factory/registry.rs").read_text(encoding="utf-8")
        # 内部 factory 不得恢复只接收 GraphicsApi 的兼容入口。
        self.assertNotIn("fn create_gpu_context_with_backend", factory)
        # 平台组合根不得静默创建脱离 runtime 的空故障队列。
        self.assertNotIn("fn create_platform()", factory)
        # 正式平台构造必须显式接收 runtime-scoped 故障队列。
        self.assertIn("fn create_platform_with_pending", factory)
        # registry 不得恢复选择同一 API 首行的 raw surface 构造入口。
        self.assertNotIn("fn try_create_gpu_context", registry)
        # registry 不得把多行 recipe 候选压缩成有损 backend-only 列表。
        self.assertNotIn("fn gpu_probe_candidates", registry)
        # recipe 构造不得静默创建脱离 runtime 的空故障队列。
        self.assertNotIn("fn try_create_gpu_recipe(", registry)
        # 正式运行时构造必须继续接收完整 GraphicsRecipe。
        self.assertIn("fn try_create_gpu_recipe_with_queue", registry)
        # 正式运行时构造必须继续携带 callback 故障队列。
        self.assertIn("pending_failures: PendingFailureQueue", registry)
        # recipe 必须通过精确 registry 行查找，不能退化为 backend-only 查找。
        self.assertIn("let entry = entry_for_recipe(recipe)", registry)
        # recipe factory 返回值必须是已验证 owner，而不是兼容 context trait object。
        self.assertIn(") -> Result<GraphicsRecipeOwner, Error>", registry)
        # context 必须在离开 native factory 前完成正交 owner 构造。
        self.assertIn("GraphicsRecipeOwner::try_new(context)", registry)

    # 校验 thin RHI 与 surface resize 只通过原子 GPU recipe 视图传播。
    def test_gpu_recipe_context_keeps_rhi_and_lifecycle_atomic(self) -> None:
        # 读取统一 context trait 与原生 resize helper。
        facade = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # 定位统一 context trait 的起点。
        context_start = facade.index("pub trait IGraphicsContext")
        # 以原生 helper 起点作为 trait 定义终点。
        context_end = facade.index("pub(crate) fn resize_native_rhi_surface", context_start)
        # 保存只包含 IGraphicsContext 定义的片段。
        context_trait = facade[context_start:context_end]
        # 通用 context 门面不得重新声明 resize 操作。
        self.assertNotIn("fn resize_rhi_surface", context_trait)
        # 通用 context 只允许返回一个不可拆分的 GPU recipe 视图。
        self.assertIn("fn gpu_recipe_context", context_trait)
        # 通用 context 不得分别暴露 thin RHI 与 lifecycle。
        self.assertNotIn("fn rhi_context", context_trait)
        # 原子 recipe 契约必须同时承载 RHI Result 与 typed resize。
        self.assertIn("trait GpuRecipeContext", facade)
        # 已退出的分裂生命周期契约不得保留。
        self.assertNotIn("trait RhiSurfaceLifecycle", facade)
        # 原生 adapter 必须共享同一 DPR 与 extent 校验 helper。
        self.assertIn("fn resize_native_rhi_surface", facade)
        # 读取 owner-thread wrapper 的生命周期事务实现。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(
            # 保持源码契约读取编码稳定。
            encoding="utf-8"
        )
        # wrapper 必须独立实现完整 GPU recipe 契约。
        self.assertIn("impl GpuRecipeContext for ThreadBoundGraphicsContext", thread_bound)
        # wrapper 必须先从真实 owner 借用同一原子视图。
        self.assertIn("inner.gpu_recipe_context()", thread_bound)
        # 定位真实 owner resize 调用。
        resize_call = thread_bound.index("recipe.resize_surface(width, height)")
        # 定位随后发生的原子元数据刷新。
        metadata_refresh = thread_bound.index("self.refresh_metadata();", resize_call)
        # 元数据只能在成功 resize 返回后刷新。
        self.assertLess(resize_call, metadata_refresh)
        # 三个生产 GPU-native adapter 都必须显式暴露并实现原子视图。
        for adapter in (
            # D3D11 生产 context。
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs",
            # WGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs",
            # EGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
        ):
            # 读取当前 native adapter 的完整 recipe 实现。
            adapter_source = adapter.read_text(encoding="utf-8")
            # adapter 必须通过 IGraphicsContext 暴露唯一原子视图。
            self.assertIn("fn gpu_recipe_context", adapter_source)
            # adapter 必须实现同时拥有 RHI 与 resize 的契约。
            self.assertIn("GpuRecipeContext for", adapter_source)
            # adapter 必须复用统一的原生 resize helper。
            self.assertIn(
                "resize_native_rhi_surface(self, present_surface, width, height)",
                adapter_source,
            )
        # 读取生产 GPU backend 的 resize 消费者。
        gpu_backend = (
            # 固定 GPU backend 生命周期实现路径。
            ROOT / "src/draw/backend/gpu/backend/render_backend.rs"
        ).read_text(encoding="utf-8")
        # GPU backend 必须通过已验证 recipe owner 执行专用生命周期事务。
        self.assertIn("self.gpu_ctx.resize_surface(logical_w, logical_h)?", gpu_backend)
        # GPU backend 不得重新直接查询兼容 trait 的可选 recipe 视图。
        self.assertNotIn("self.gpu_ctx.gpu_recipe_context()", gpu_backend)
        # GPU backend 不得调用已退出 IGraphicsContext 的 resize 方法。
        self.assertNotIn("self.gpu_ctx.resize_rhi_surface", gpu_backend)

    # 校验生产 GPU backend 只持有构造期已验证的 recipe owner。
    def test_gpu_backend_uses_validated_recipe_owner(self) -> None:
        # 读取 GPU owner 的唯一 native 边界实现。
        owner = (ROOT / "src/native/present/gpu_recipe_owner.rs").read_text(encoding="utf-8")
        # 读取 GPU backend 的状态定义。
        backend = (ROOT / "src/draw/backend/gpu/backend/mod.rs").read_text(encoding="utf-8")
        # 读取唯一 renderer 装配入口。
        runtime = (ROOT / "src/draw/renderer/runtime.rs").read_text(encoding="utf-8")
        # 读取 native factory 使用的正交 recipe owner。
        recipe_owner = (ROOT / "src/native/present/recipe_owner.rs").read_text(encoding="utf-8")
        # 截取生产 owner 实现，排除测试 context 的 caps 方法。
        owner_contract = owner[: owner.index("#[cfg(test)]")]
        # 兼容 trait object 只能封装在 native owner 内。
        self.assertIn("context: Box<dyn IGraphicsContext>", owner)
        # owner 必须固化构造期验证过的静态 capability 快照。
        self.assertIn("caps: GraphicsContextCaps", owner_contract)
        # 生产 owner 只能在构造门禁读取一次兼容 context caps。
        self.assertEqual(owner_contract.count("context.caps()"), 1)
        # GPU owner 必须在 backend 构造前一次证明完整 recipe owner 存在。
        self.assertIn("if context.gpu_recipe_context().is_none()", owner_contract)
        # 构造门禁不得分别探测 thin RHI 与 lifecycle。
        self.assertEqual(owner_contract.count("context.gpu_recipe_context().is_none()"), 1)
        # 已退出的分裂 lifecycle 查询不得保留。
        self.assertNotIn("rhi_surface_lifecycle", owner_contract)
        # owner 必须将可选 thin RHI 查询收口为 Result。
        self.assertIn("fn rhi_context(&mut self) -> Result<&mut dyn GraphicsContextRhi>", owner)
        # owner 必须独立承接 recipe 专用 resize。
        self.assertIn("fn resize_surface(&mut self, width: i32, height: i32) -> Result<()>", owner)
        # GPU backend 状态只持有已验证 owner。
        self.assertIn("pub(crate) gpu_ctx: GpuRecipeOwner", backend)
        # GPU backend 不得重新持有兼容 trait object。
        self.assertNotIn("Box<dyn IGraphicsContext>", backend)
        # native recipe owner 必须在进入 renderer 前完成 GPU owner 校验。
        self.assertIn("GpuRecipeOwner::try_new(context).map(Self::Gpu)", recipe_owner)
        # owner 校验后必须直接构造唯一 GPU backend。
        self.assertIn("let backend = GpuBackend::new_gpu_only(owner)?;", runtime)
        # draw backend 模块不得恢复兼容 context factory。
        self.assertFalse((ROOT / "src/draw/backend/factory.rs").exists())
        # 读取 backend 模块根，锁定兼容依赖退出。
        backend_module = (ROOT / "src/draw/backend/mod.rs").read_text(encoding="utf-8")
        # 汇总整个 draw backend 目录的 Rust 源码。
        backend_sources = "\n".join(
            # 逐文件读取源码，避免只验证模块根形成假阴性。
            path.read_text(encoding="utf-8")
            # 覆盖 backend 下的所有拆分模块。
            for path in (ROOT / "src/draw/backend").rglob("*.rs")
        )
        # 整个 draw backend 不再依赖 IGraphicsContext。
        self.assertNotIn("IGraphicsContext", backend_sources)
        # 模块根不再声明 factory 子模块。
        self.assertNotIn("mod factory", backend_module)

    # 校验通用会话不再保留第二条 staged GPU context 装配链。
    def test_render_session_has_no_staged_gpu_context_path(self) -> None:
        # 读取通用后端工厂。
        backend_factory = (ROOT / "src/draw/backend/mod.rs").read_text(encoding="utf-8")
        # 读取会话生命周期实现。
        session = (ROOT / "src/draw/renderer/session.rs").read_text(encoding="utf-8")
        # 通用工厂不得接收兼容 context trait object。
        self.assertNotIn("IGraphicsContext", backend_factory)
        # 通用 GPU 选择必须返回稳定 typed error。
        self.assertIn("GPU 后端必须由已验证的原生图形配方构造", backend_factory)
        # 会话不得持有 staged context 字段。
        self.assertNotIn("gpu_ctx", session)
        # 会话不得恢复 staged context 注入入口。
        self.assertNotIn("set_gpu_context", session)

    # 校验 renderer 装配只保留共享 assemble_renderer 入口。
    def test_renderer_assembly_has_no_forwarding_context_factory(self) -> None:
        # 读取 renderer 模块声明。
        renderer_module = (ROOT / "src/draw/renderer/mod.rs").read_text(encoding="utf-8")
        # 读取 bootstrap 与 recovery 共用的装配函数。
        bootstrap = (ROOT / "src/draw/renderer/bootstrap.rs").read_text(encoding="utf-8")
        # 单函数转发模块必须物理删除。
        self.assertFalse((ROOT / "src/draw/renderer/factory.rs").exists())
        # renderer 模块不得重新声明 forwarding factory。
        self.assertNotIn("mod factory", renderer_module)
        # 共享装配入口必须直接调用唯一 recipe 分派。
        self.assertIn("Renderer::from_recipe_owner(owner)", bootstrap)
        # 汇总 renderer 生产源码，锁定兼容 context 不再跨越 native factory。
        renderer_sources = "\n".join(
            # 逐文件读取 renderer 模块源码。
            path.read_text(encoding="utf-8")
            # 覆盖 renderer 下的全部生产 Rust 文件。
            for path in (ROOT / "src/draw/renderer").rglob("*.rs")
        )
        # renderer 层不得重新依赖迁移期 IGraphicsContext。
        self.assertNotIn("IGraphicsContext", renderer_sources)
        # bootstrap 不得重新导入平行 create_renderer 门面。
        self.assertNotIn("create_renderer", bootstrap)

    # 校验 windowing 不再重复拥有 renderer 的图形 context。
    def test_platform_window_has_no_graphics_context_owner(self) -> None:
        # 读取窗口公共契约。
        window_contract = (ROOT / "src/native/windowing/window.rs").read_text(encoding="utf-8")
        # 读取平台共享窗口实现。
        shared_window = (ROOT / "src/native/windowing/shared/window.rs").read_text(encoding="utf-8")
        # 读取测试窗口实现，避免测试门面复活兼容所有权。
        fake_window = (ROOT / "src/native/test_harness/fake_window.rs").read_text(encoding="utf-8")
        # 窗口公共契约不得暴露 renderer context 借用入口。
        self.assertNotIn("graphics_context", window_contract)
        # 平台共享窗口不得持有或关闭图形 context。
        self.assertNotIn("IGraphicsContext", shared_window)
        # 零调用的带 GPU 窗口构造入口必须物理删除。
        self.assertNotIn("with_gpu", shared_window)
        # 测试窗口也不得保留平行 GPU context 状态。
        self.assertNotIn("gpu_ctx", fake_window)
        # live context 的 present recipe 不得混入没有 context 的 app-only presenter。
        present_contract = (ROOT / "src/native/present/mod.rs").read_text(encoding="utf-8")
        # 删除没有任何构造者的 CpuPresenter 枚举值。
        self.assertNotIn("CpuPresenter", present_contract)

    # 校验 CPU PixelUpload presentation 只持有构造期已验证的 recipe owner。
    def test_pixel_upload_presentation_uses_validated_recipe_owner(self) -> None:
        # 读取 PixelUpload owner 的唯一 native 边界实现。
        owner = (ROOT / "src/native/present/pixel_upload_recipe_owner.rs").read_text(encoding="utf-8")
        # 读取统一 renderer 的 presentation 状态机。
        runtime = (ROOT / "src/draw/renderer/runtime.rs").read_text(encoding="utf-8")
        # 读取离开 native factory 前的正交 owner 分派。
        recipe_owner = (ROOT / "src/native/present/recipe_owner.rs").read_text(encoding="utf-8")
        # 截取生产 owner 实现，排除测试 context 的 caps 方法。
        owner_contract = owner[: owner.index("#[cfg(test)]")]
        # 兼容 trait object 只能封装在 native owner 内。
        self.assertIn("context: Box<dyn IGraphicsContext>", owner)
        # owner 必须固化构造期验证过的静态 capability 快照。
        self.assertIn("caps: GraphicsContextCaps", owner_contract)
        # 生产 owner 只能在构造门禁读取一次兼容 context caps。
        self.assertEqual(owner_contract.count("context.caps()"), 1)
        # owner 必须独立承接 PixelUpload resize。
        self.assertIn("fn resize_surface(&mut self, width: i32, height: i32) -> Result<()>", owner)
        # owner 必须独立承接最终像素提交。
        self.assertIn("fn present_pixels(", owner)
        # presentation 状态只持有已验证 owner。
        self.assertIn("owner: PixelUploadRecipeOwner", runtime)
        # presentation 不得重新持有兼容 trait object。
        presentation_start = runtime.index("struct PixelUploadPresentation")
        # 截取 presentation 实现结束前的局部源码。
        presentation_end = runtime.index("/// UIX 唯一的图形运行时类型。", presentation_start)
        # 保存只属于 PixelUpload presentation 的状态与方法。
        presentation = runtime[presentation_start:presentation_end]
        # 局部状态不得恢复兼容 context 字段。
        self.assertNotIn("Box<dyn IGraphicsContext>", presentation)
        # 局部方法不得直接查询可选 surface 视图。
        self.assertNotIn("pixel_upload_surface()", presentation)
        # native recipe owner 必须在进入 renderer 前完成 PixelUpload owner 校验。
        self.assertIn(
            "PixelUploadRecipeOwner::try_new(context).map(Self::PixelUpload)",
            recipe_owner,
        )
    # 校验无帧遮挡探测只属于 thin RHI surface 生命周期。
    def test_idle_present_probe_belongs_to_graphics_surface(self) -> None:
        # 读取迁移期 context 门面。
        facade = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # 通用 context 不得继续声明 surface 专属探测。
        self.assertNotIn("fn test_present(&mut self)", facade)
        # 读取 thin RHI 契约。
        rhi = (ROOT / "src/native/present/rhi.rs").read_text(encoding="utf-8")
        # GraphicsSurface 必须显式声明无帧探测入口。
        self.assertIn("fn test_present(&mut self) -> Result<PresentTestResult>", rhi)
        # 默认 surface 必须通过 typed RHI 错误拒绝未实现能力。
        self.assertIn('rhi_not_implemented("test_present")', rhi)
        # 读取 D3D11 兼容 context 实现。
        d3d11_context = (
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs"
        ).read_text(encoding="utf-8")
        # D3D11 context 门面不得重复实现无帧探测。
        self.assertNotIn("fn test_present(&mut self)", d3d11_context)
        # 读取 D3D11 thin RHI surface 实现。
        d3d11_surface = (
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi.rs"
        ).read_text(encoding="utf-8")
        # Windows 专属探测必须落在 GraphicsSurface 实现中。
        self.assertIn("fn test_present(&mut self) -> Result<PresentTestResult>", d3d11_surface)
        # 探测继续使用无帧数据的 DXGI 标志。
        self.assertIn("DXGI_PRESENT_TEST", d3d11_surface)
        # 既有 Presentable/Occluded/error 映射必须继续复用。
        self.assertIn("map_dxgi_present_test_result", d3d11_surface)
        # 读取 owner-thread context wrapper。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # wrapper 不得继续转发已经下沉的兼容方法。
        self.assertNotIn("forward_result!(test_present", thread_bound)
        # 读取 GPU backend 的最终 surface 入口。
        gpu_backend = (
            ROOT / "src/draw/backend/gpu/backend/render_backend.rs"
        ).read_text(encoding="utf-8")
        # backend 必须借用组合 RHI 后调用 surface 探测。
        self.assertIn("context.test_present()", gpu_backend)
        # backend 不得继续调用 IGraphicsContext 兼容探测。
        self.assertNotIn("self.gpu_ctx.test_present()", gpu_backend)
        # 读取统一 renderer 的 presentation 分派。
        runtime = (ROOT / "src/draw/renderer/runtime.rs").read_text(encoding="utf-8")
        # PixelUpload 不得继续借用 context 的 swapchain 探测。
        self.assertNotIn("upload.context.test_present()", runtime)
        # PixelUpload 必须保留 recipe 级 typed 未支持说明。
        self.assertIn("pixel-upload Renderer does not support idle present tests", runtime)

    # 校验 overlay backdrop 的资源失败不会重新退化为布尔 fallback。
    def test_overlay_backdrop_lifecycle_preserves_typed_failures(self) -> None:
        # 读取 backend 与 renderer 的两层生命周期契约。
        backend_contract = (ROOT / "src/draw/backend/contract.rs").read_text(encoding="utf-8")
        # 读取场景侧 RenderTarget 契约。
        target_contract = (ROOT / "src/draw/target.rs").read_text(encoding="utf-8")
        # 读取 GPU owner 事务实现。
        backdrop = (
            # 固定独立 backdrop 模块路径。
            ROOT / "src/draw/backend/gpu/backend/render_backend_backdrop.rs"
        ).read_text(encoding="utf-8")
        # 读取场景管线失败边界。
        pipeline = (
            # 固定 ScenePipeline 实现路径。
            ROOT / "src/draw/renderer/scene_pipeline/pipeline.rs"
        ).read_text(encoding="utf-8")
        # 读取有界恢复包装器，确认场景外错误会登记到下一帧。
        recovery = (ROOT / "src/draw/renderer/recovery_driver.rs").read_text(
            # 保持源码契约读取编码稳定。
            encoding="utf-8"
        )
        # 两层快照契约都必须返回 typed Result。
        for contract in (backend_contract, target_contract):
            # 快照不支持使用 Ok(false)，真实失败使用 Error。
            self.assertIn(
                "fn snapshot_overlay_backdrop(&mut self) -> Result<bool, Error>", contract
            )
            # 恢复不支持使用 Ok(false)，真实失败使用 Error。
            self.assertIn(
                "fn restore_overlay_backdrop(&mut self) -> Result<bool, Error>", contract
            )
            # 释放必须允许 owner-thread 销毁失败越过接口。
            self.assertIn(
                "fn release_overlay_backdrop(&mut self) -> Result<(), Error>", contract
            )
        # 替换旧快照前的检查式销毁失败必须原样传播。
        self.assertIn("self.destroy_rhi_overlay_backdrop_texture()?;", backdrop)
        # snapshot 与 restore 的 device maintenance 都必须传播失败。
        self.assertGreaterEqual(backdrop.count("self.prepare_rhi_device()?;"), 2)
        # 恢复 copy/submit 事务不得只记录日志后返回 false。
        self.assertIn("result?;", backdrop)
        # 显式 release 必须直接返回检查式销毁结果。
        self.assertIn("self.destroy_rhi_overlay_backdrop_texture()", backdrop)
        # 场景管线必须统一生成不可提交的失败帧。
        self.assertIn("fn failed_backdrop_frame", pipeline)
        # 禁止恢复曾经吞掉快照错误的无结果调用。
        self.assertNotIn("let _ = engine.snapshot_overlay_backdrop()", pipeline)
        # 恢复失败必须在 end_frame 前直接返回失败帧。
        self.assertIn(
            "Err(error) => return Self::failed_backdrop_frame(error, cur_version)", pipeline
        )
        # RecoveryDriver 必须观察 snapshot 的 begin_frame 外失败。
        self.assertIn("let result = self.engine.snapshot_overlay_backdrop();", recovery)
        # RecoveryDriver 必须观察 begin_frame 后的 restore 失败。
        self.assertIn("let result = self.engine.restore_overlay_backdrop();", recovery)
        # RecoveryDriver 必须观察 idle 前的 release 失败。
        self.assertIn("let result = self.engine.release_overlay_backdrop();", recovery)

    # 校验 Vulkan owner shutdown 与 lost-device generation 契约。
    def test_direct_vulkan_uses_owner_shutdown_and_lost_device_generation(self) -> None:
        # 组合读取 Vulkan context 的拆分模块，以保持顺序审计语义。
        context = read_rust_module(VULKAN_CONTEXT)
        # 读取共享 device 管理实现。
        device = VULKAN_DEVICE.read_text(encoding="utf-8")
        # 读取 Vulkan fault 映射实现。
        fault = VULKAN_FAULT.read_text(encoding="utf-8")
        # 读取 GFX-R5 拆分目录。
        gfx_r5 = read_rust_module(GFX_R5)
        # 定位 checked shutdown 实现。
        shutdown = context.index("fn shutdown_result")
        # 定位共享 device lease 释放点。
        release = context.index("self.device_lease.take()", shutdown)
        # 定位 native surface 销毁点。
        close = context.index("self.surface_loader.destroy_surface", shutdown)
        # surface 必须先于共享 device lease 释放。
        self.assertLess(close, release)
        # device lease 必须先于 runtime 释放。
        self.assertLess(release, context.index("self.runtime.take()", release))
        # shutdown 标记必须在成功返回前写入。
        self.assertLess(
            # 定位 shutdown 状态写入。
            context.index("self.shutdown = true", release),
            # 定位随后成功返回的位置。
            context.index("Ok(())", context.index("self.shutdown = true", release)),
        )
        # Drop 必须调用 checked shutdown。
        self.assertIn("if let Err(error) = self.try_shutdown()", context)
        # teardown 失败时不得错误释放 device。
        self.assertIn("std::mem::forget(device)", context)
        # teardown 失败时不得错误释放 runtime。
        self.assertIn("std::mem::forget(runtime)", context)
        # lost device 不得被共享池复用。
        self.assertIn("if !device.is_lost()", device)
        # 新 device generation 必须重新写回共享池。
        self.assertIn("devices.insert(key, Rc::downgrade(&device))", device)
        # device 操作必须通过统一观察入口。
        self.assertIn("fn observe<T>", device)
        # Vulkan fault 必须映射为 typed device-lost 错误。
        self.assertIn("Errc::GraphicsDeviceLost", fault)
        # GFX-R5 必须构造真实 Vulkan context。
        self.assertGreaterEqual(gfx_r5.count("VulkanContext::new"), 1)
        # engine recovery 必须通过专用 target 验证。
        self.assertIn("impl RenderTarget for VulkanRecoveryTarget", gfx_r5)
        # recovery target 必须暴露 checked shutdown。
        self.assertIn("fn try_shutdown(&mut self)", gfx_r5)
        # driver 必须显式执行 checked shutdown。
        self.assertIn("driver.try_shutdown()", gfx_r5)
        # context 必须显式执行 checked shutdown。
        self.assertIn("context.try_shutdown()", gfx_r5)


# 支持直接执行本测试模块。
if __name__ == "__main__":
    # 运行模块内全部 unittest。
    unittest.main()
