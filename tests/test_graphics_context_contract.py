# -*- coding: utf-8 -*-
"""Keep graphics context metadata and Vulkan ownership contracts explicit."""

# 引入单元测试框架。
import unittest
# 引入跨平台路径工具。
from pathlib import Path


# 固定仓库根目录，供源码契约读取使用。
ROOT = Path(__file__).resolve().parents[1]
# 固定拆分后的 Vulkan context 组合模块位置。
VULKAN_CONTEXT = ROOT / "src/native/presentation/graphics/vulkan/adapter/context"
# 固定 Vulkan 共享 device 管理实现位置。
VULKAN_DEVICE = ROOT / "src/native/presentation/graphics/vulkan/adapter/device.rs"
# 固定 Vulkan fault 映射实现位置。
VULKAN_FAULT = ROOT / "src/native/presentation/graphics/vulkan/adapter/fault.rs"
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
        contracts = (ROOT / "src/native/presentation/contracts/mod.rs").read_text(encoding="utf-8")
        # 截取 crate-private GraphicsContextCaps 的字段定义并锁定其边界。
        caps_start = contracts.index("pub(crate) struct GraphicsContextCaps")
        # 以构造器实现起点作为结构体字段终点。
        caps_end = contracts.index("impl GraphicsContextCaps", caps_start)
        # 保存只包含静态 recipe 事实的 capability 结构体片段。
        caps_struct = contracts[caps_start:caps_end]
        # 动态 DPR 不得继续伪装成 capability 字段。
        self.assertNotIn("device_pixel_ratio", caps_struct)
        # 读取统一 context trait。
        facade = (ROOT / "src/native/presentation/contracts/traits.rs").read_text(encoding="utf-8")
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
        bound_end = thread_bound.index(
            "impl<T: GraphicsContextLifecycle + ?Sized> ThreadBoundGraphicsContext",
            bound_start,
        )
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
        # wrapper 不得缓存或重新查询已经退出 context 生命周期的静态 capability。
        self.assertNotIn("inner.caps()", thread_bound)
        # 静态快照只能留在 registry 验证记录与 recipe owner。
        self.assertNotIn("caps: GraphicsContextCaps", bound_struct)
        # 逐个核对所有 context 实现都显式提供 surface 元数据。
        for adapter in (
            # D3D11 生产 context。
            ROOT / "src/native/presentation/graphics/d3d11/adapter/context/graphics.rs",
            # D3D12 测试期 context。
            ROOT / "src/native/presentation/graphics/d3d12/adapter/context/graphics.rs",
            # WGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/adapter/wgl_graphics.rs",
            # EGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/adapter/egl_rhi.rs",
            # Vulkan PixelUpload context。
            ROOT / "src/native/presentation/graphics/vulkan/adapter/context/graphics.rs",
            # Metal PixelUpload context。
            ROOT / "src/native/presentation/graphics/metal/adapter/context.rs",
            # GPU-native 测试 fake。
            ROOT / "tests/support/native/test_harness/fake_graphics_context.rs",
        ):
            # 每个实现必须返回单一 PresentSurface 快照。
            adapter_source = adapter.read_text(encoding="utf-8")
            # 禁止依赖已删除的默认元数据拼装。
            self.assertIn("fn present_surface(&self)", adapter_source)
            # 所有 context 实现都不得恢复静态 capability SPI。
            self.assertNotIn("fn caps(&self)", adapter_source)
        # 读取 GPU backend 的 live metadata 派生边界。
        gpu_backend = (ROOT / "src/draw/backend/gpu/execution/mod.rs").read_text(encoding="utf-8")
        # GPU backend 必须从 PresentSurface 派生逻辑元数据。
        self.assertIn("logical_metadata_from_surface", gpu_backend)
        # 读取 GPU backend 构造、回读与 surface 采纳路径。
        gpu_lifecycle = (ROOT / "src/draw/backend/gpu/execution/impl_main.rs").read_text(encoding="utf-8")
        # GPU backend 不得重新读取分离的 context width。
        self.assertNotIn("gpu_ctx.width()", gpu_lifecycle)
        # GPU backend 不得重新读取分离的 context height。
        self.assertNotIn("gpu_ctx.height()", gpu_lifecycle)
        # 构造路径必须一次读取传入 owner 的完整快照。
        self.assertIn("let present_surface = gpu_ctx.present_surface();", gpu_lifecycle)
        # surface 采纳路径必须一次读取当前 owner 的完整快照。
        self.assertIn("let present_surface = self.gpu_ctx.present_surface();", gpu_lifecycle)
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
        facade = (ROOT / "src/native/presentation/contracts/traits.rs").read_text(encoding="utf-8")
        # backend 身份不得恢复为派生 trait 查询。
        self.assertNotIn("fn graphics_backend(&self)", facade)
        # 静态 recipe 能力已经退出兼容 context SPI。
        self.assertNotIn("fn caps(&self)", facade)
        # 无消费者的 GL proc 查询不得保留在生产门面。
        self.assertNotIn("fn supports_gl_proc_address(&self)", facade)
        # 无消费者的 PixelUpload 查询不得保留在生产门面。
        self.assertNotIn("fn supports_pixel_present(&self)", facade)
        # 原子 GPU recipe 已证明 owner 完整，resize helper 不再重复读取 backend 身份。
        self.assertNotIn("context.caps().backend", facade)
        # 逐个核对曾重复声明 backend 的 adapter 已删除派生实现。
        for adapter in (
            # D3D11 生产 context。
            ROOT / "src/native/presentation/graphics/d3d11/adapter/context/graphics.rs",
            # WGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/adapter/wgl_graphics.rs",
            # EGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/adapter/egl_rhi.rs",
            # Vulkan PixelUpload context。
            ROOT / "src/native/presentation/graphics/vulkan/adapter/context/graphics.rs",
            # GPU-native 测试 fake。
            ROOT / "tests/support/native/test_harness/fake_graphics_context.rs",
        ):
            # adapter 只能在 caps 构造中陈述 backend 身份。
            adapter_source = adapter.read_text(encoding="utf-8")
            # 禁止重新引入重复的 trait 方法实现。
            self.assertNotIn("fn graphics_backend(&self)", adapter_source)
            # adapter context 实现不得恢复静态 capability SPI。
            self.assertNotIn("fn caps(&self)", adapter_source)

    # 校验生产 context 构造只接受完整 recipe 与运行时故障队列。
    def test_internal_context_factory_has_no_single_backend_shortcut(self) -> None:
        # 读取 native factory 的公开组合入口。
        factory = (ROOT / "src/native/factory/mod.rs").read_text(encoding="utf-8")
        # 读取 registry 的 recipe 校验与构造实现。
        registry = (ROOT / "src/native/factory/registry.rs").read_text(encoding="utf-8")
        # 读取 presentation 侧的 adapter candidate 交接契约。
        present = (ROOT / "src/native/presentation/contracts/mod.rs").read_text(encoding="utf-8")
        # 截取 registry 生产实现，排除测试 context。
        registry_contract = registry[: registry.index("#[cfg(test)]")]
        # 读取线程亲和 wrapper 的静态快照构造边界。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(
            # 保持源码契约读取编码稳定。
            encoding="utf-8"
        )
        # 截取 thread-bound 生产实现，排除测试模块。
        thread_bound_contract = thread_bound[: thread_bound.index("#[cfg(test)]")]
        # 读取顶层正交 recipe owner 的生产分派。
        recipe_owner = (ROOT / "src/native/presentation/contracts/recipe_owner.rs").read_text(
            # 保持源码契约读取编码稳定。
            encoding="utf-8"
        )
        # 截取 recipe owner 生产实现，排除测试 context。
        recipe_owner_contract = recipe_owner[: recipe_owner.index("#[cfg(test)]")]
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
        # adapter 创建层必须用显式 candidate 同时交付 context 与 capability。
        self.assertIn("struct GraphicsContextCandidate", present)
        # candidate 只允许在 registry 边界一次转移两个同源值。
        self.assertIn("fn into_parts(self)", present)
        # registry factory 指针必须直接返回 candidate，而不是裸 context。
        self.assertIn("Result<GraphicsContextCandidate, Error>", registry_contract)
        # registry 不得通过兼容 trait object 重新查询 adapter 静态 capability。
        self.assertNotIn("ctx.caps()", registry_contract)
        # registry 必须消费 adapter 创建层交付的 candidate。
        self.assertIn("candidate.into_parts()", registry_contract)
        # registry 只把 context 交给 thread-bound wrapper，静态快照留在验证记录中。
        self.assertIn("bind_to_current_thread(context)", registry_contract)
        # thread-bound wrapper 不得重新读取 inner 的静态 capability。
        self.assertNotIn("inner.caps()", thread_bound_contract)
        # wrapper 构造不得继续接收或缓存 registry 已验证快照。
        self.assertNotIn("caps: GraphicsContextCaps", thread_bound_contract)
        # context 必须在离开 native factory 前消费同一快照完成正交 owner 构造。
        self.assertIn("GraphicsRecipeOwner::try_new(context, caps)", registry)
        # 顶层 recipe owner 不得恢复兼容 context 的静态查询。
        self.assertNotIn("context.caps()", recipe_owner_contract)
        # 两个当前生产 adapter 必须在具体创建层组装 candidate。
        for adapter_factory in (
            # Windows 参考 D3D11 adapter 创建层。
            ROOT / "src/native/presentation/graphics/d3d11/adapter/mod.rs",
            # Windows/Linux 共用的 OpenGL ES 平台创建层。
            ROOT / "src/native/presentation/graphics/opengl/adapter/mod.rs",
        ):
            # 读取 adapter 平台创建实现。
            adapter_source = adapter_factory.read_text(encoding="utf-8")
            # context 与 capability 必须在仍可静态分派的边界成对交付。
            self.assertIn("GraphicsContextCandidate::gpu", adapter_source)
            # adapter 创建模块必须显式拥有静态 capability 构造函数（兼容多行签名）。
            self.assertIn("fn context_caps(", adapter_source)
            # candidate 组装不得反向调用 context trait method。
            self.assertNotIn(".caps()", adapter_source)

    # 校验 thin RHI 与 surface resize 只通过原子 GPU recipe 视图传播。
    def test_gpu_recipe_context_keeps_rhi_and_lifecycle_atomic(self) -> None:
        # 读取统一 context trait 与原生 resize helper。
        facade = (ROOT / "src/native/presentation/contracts/traits.rs").read_text(encoding="utf-8")
        # 定位共享 lifecycle trait 的起点。
        lifecycle_start = facade.index("pub(crate) trait GraphicsContextLifecycle")
        # 以 GPU recipe trait 起点作为共享 lifecycle 定义终点。
        lifecycle_end = facade.index("pub(crate) trait GpuRecipeContext", lifecycle_start)
        # 保存只包含所有 recipe 共用生命周期的片段。
        lifecycle_trait = facade[lifecycle_start:lifecycle_end]
        # 共享 lifecycle 不得重新声明 recipe 专用 resize 操作。
        self.assertNotIn("fn resize_rhi_surface", lifecycle_trait)
        # 共享 lifecycle 只承载 surface 快照与 checked shutdown。
        self.assertIn("fn present_surface(&self) -> PresentSurface", lifecycle_trait)
        # 共享 lifecycle 不得暴露 recipe 能力查询。
        self.assertNotIn("gpu_recipe_context", lifecycle_trait)
        # 共享 lifecycle 不得直接暴露 thin RHI。
        self.assertNotIn("fn rhi_context", lifecycle_trait)
        # 原子 recipe 契约必须同时承载 RHI Result 与 typed resize。
        self.assertIn("trait GpuRecipeContext: GraphicsContextLifecycle", facade)
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
        self.assertIn(
            "impl GpuRecipeContext for ThreadBoundGraphicsContext<dyn GpuRecipeContext>",
            thread_bound,
        )
        # wrapper 必须直接借用类型已经证明的 GPU owner。
        self.assertNotIn("inner.gpu_recipe_context()", thread_bound)
        # 定位真实 owner resize 调用。
        resize_call = thread_bound.index("inner.resize_surface(width, height)")
        # 定位随后发生的完整 surface 快照刷新。
        surface_refresh = thread_bound.index("self.refresh_present_surface();", resize_call)
        # live surface 只能在成功 resize 返回后刷新。
        self.assertLess(resize_call, surface_refresh)
        # 三个生产 GPU-native adapter 都必须显式暴露并实现原子视图。
        for adapter in (
            # D3D11 生产 context。
            ROOT / "src/native/presentation/graphics/d3d11/adapter/context/graphics.rs",
            # WGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/adapter/wgl_graphics.rs",
            # EGL 生产 context。
            ROOT / "src/native/presentation/graphics/opengl/adapter/egl_rhi.rs",
        ):
            # 读取当前 native adapter 的完整 recipe 实现。
            adapter_source = adapter.read_text(encoding="utf-8")
            # adapter 不得重新暴露可选 recipe 视图。
            self.assertNotIn("fn gpu_recipe_context", adapter_source)
            # adapter 必须实现所有 recipe 共享的 lifecycle。
            self.assertIn("GraphicsContextLifecycle for", adapter_source)
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
            ROOT / "src/draw/backend/gpu/execution/render_backend.rs"
        ).read_text(encoding="utf-8")
        # GPU backend 必须通过已验证 recipe owner 执行专用生命周期事务。
        self.assertIn("self.gpu_ctx.resize_surface(logical_w, logical_h)?", gpu_backend)
        # GPU backend 不得重新直接查询兼容 trait 的可选 recipe 视图。
        self.assertNotIn("self.gpu_ctx.gpu_recipe_context()", gpu_backend)
        # GPU backend 不得调用已退出共享 lifecycle 的 recipe resize 方法。
        self.assertNotIn("self.gpu_ctx.resize_rhi_surface", gpu_backend)

    # 校验生产 GPU backend 只持有构造期已验证的 recipe owner。
    def test_gpu_backend_uses_validated_recipe_owner(self) -> None:
        # 读取 GPU owner 的唯一 native 边界实现。
        owner = (ROOT / "src/native/presentation/contracts/gpu_recipe_owner.rs").read_text(encoding="utf-8")
        # 读取 GPU backend 的状态定义。
        backend = (ROOT / "src/draw/backend/gpu/execution/mod.rs").read_text(encoding="utf-8")
        # 读取唯一 renderer 装配入口。
        runtime = (ROOT / "src/draw/renderer/runtime.rs").read_text(encoding="utf-8")
        # 读取 native factory 使用的正交 recipe owner。
        recipe_owner = (ROOT / "src/native/presentation/contracts/recipe_owner.rs").read_text(encoding="utf-8")
        # 截取正交 owner 的生产实现，排除测试 context。
        recipe_owner_contract = recipe_owner[: recipe_owner.index("#[cfg(test)]")]
        # 截取生产 owner 实现，排除测试 context 的 caps 方法。
        owner_contract = owner[: owner.index("#[cfg(test)]")]
        # native owner 必须直接持有类型化 GPU trait object。
        self.assertIn("context: Box<dyn GpuRecipeContext>", owner)
        # owner 必须固化构造期验证过的静态 capability 快照。
        self.assertIn("caps: GraphicsContextCaps", owner_contract)
        # 专用 GPU owner 必须复用顶层已经捕获的静态快照。
        self.assertNotIn("context.caps()", owner_contract)
        # 顶层正交 owner 必须直接消费 registry 已验证的静态快照。
        self.assertNotIn("context.caps()", recipe_owner_contract)
        # GPU owner 不得在构造后重新执行可选 recipe 查询。
        self.assertNotIn("gpu_recipe_context()", owner_contract)
        # 类型化 context 必须直接进入 owner。
        self.assertIn("context: Box<dyn GpuRecipeContext>", owner_contract)
        # 已退出的分裂 lifecycle 查询不得保留。
        self.assertNotIn("rhi_surface_lifecycle", owner_contract)
        # owner 必须将可选 thin RHI 查询收口为 Result。
        self.assertIn("fn rhi_context(&mut self) -> Result<&mut dyn GraphicsContextRhi>", owner)
        # owner 必须独立承接 recipe 专用 resize。
        self.assertIn("fn resize_surface(&mut self, width: i32, height: i32) -> Result<()>", owner)
        # GPU backend 状态只持有已验证 owner。
        self.assertIn("pub(crate) gpu_ctx: GpuRecipeOwner", backend)
        # GPU backend 不得重新持有共享生命周期 trait object。
        self.assertNotIn("Box<dyn GraphicsContextLifecycle>", backend)
        # native recipe owner 必须在进入 renderer 前完成 GPU owner 校验。
        self.assertIn("GpuRecipeOwner::try_new(context, caps).map(Self::Gpu)", recipe_owner)
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
        # 整个 draw backend 不再依赖共享 lifecycle trait object。
        self.assertNotIn("Box<dyn GraphicsContextLifecycle>", backend_sources)
        # 模块根不再声明 factory 子模块。
        self.assertNotIn("mod factory", backend_module)

    # 校验通用会话不再保留第二条 staged GPU context 装配链。
    def test_render_session_has_no_staged_gpu_context_path(self) -> None:
        # 读取通用后端工厂。
        backend_factory = (ROOT / "src/draw/backend/mod.rs").read_text(encoding="utf-8")
        # 读取会话生命周期实现。
        session = (ROOT / "src/draw/renderer/session.rs").read_text(encoding="utf-8")
        # 通用工厂不得接收兼容 context trait object。
        self.assertNotIn("Box<dyn GraphicsContextLifecycle>", backend_factory)
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
        # renderer 层不得重新依赖共享 lifecycle trait object。
        self.assertNotIn("Box<dyn GraphicsContextLifecycle>", renderer_sources)
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
        self.assertNotIn("GraphicsContextLifecycle", shared_window)
        # 零调用的带 GPU 窗口构造入口必须物理删除。
        self.assertNotIn("with_gpu", shared_window)
        # 测试窗口也不得保留平行 GPU context 状态。
        self.assertNotIn("gpu_ctx", fake_window)
        # live context 的 present recipe 不得混入没有 context 的 app-only presenter。
        present_contract = (ROOT / "src/native/presentation/contracts/mod.rs").read_text(encoding="utf-8")
        # 删除没有任何构造者的 CpuPresenter 枚举值。
        self.assertNotIn("CpuPresenter", present_contract)

    # 校验 CPU PixelUpload presentation 只持有构造期已验证的 recipe owner。
    def test_pixel_upload_presentation_uses_validated_recipe_owner(self) -> None:
        # 读取 PixelUpload owner 的唯一 native 边界实现。
        owner = (ROOT / "src/native/presentation/contracts/pixel_upload_recipe_owner.rs").read_text(encoding="utf-8")
        # 读取统一 renderer 的 presentation 状态机。
        runtime = (ROOT / "src/draw/renderer/runtime.rs").read_text(encoding="utf-8")
        # 读取离开 native factory 前的正交 owner 分派。
        recipe_owner = (ROOT / "src/native/presentation/contracts/recipe_owner.rs").read_text(encoding="utf-8")
        # 截取正交 owner 的生产实现，排除测试 context。
        recipe_owner_contract = recipe_owner[: recipe_owner.index("#[cfg(test)]")]
        # 截取生产 owner 实现，排除测试 context 的 caps 方法。
        owner_contract = owner[: owner.index("#[cfg(test)]")]
        # native owner 必须直接持有类型化 PixelUpload trait object。
        self.assertIn("context: Box<dyn PixelUploadSurface>", owner)
        # owner 必须固化构造期验证过的静态 capability 快照。
        self.assertIn("caps: GraphicsContextCaps", owner_contract)
        # 专用 PixelUpload owner 必须复用顶层已经捕获的静态快照。
        self.assertNotIn("context.caps()", owner_contract)
        # 顶层正交 owner 必须直接消费 registry 已验证的静态快照。
        self.assertNotIn("context.caps()", recipe_owner_contract)
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
        self.assertNotIn("Box<dyn GraphicsContextLifecycle>", presentation)
        # 局部方法不得直接查询可选 surface 视图。
        self.assertNotIn("pixel_upload_surface()", presentation)
        # native recipe owner 必须在进入 renderer 前完成 PixelUpload owner 校验。
        self.assertIn(
            "PixelUploadRecipeOwner::try_new(context, caps).map(Self::PixelUpload)",
            recipe_owner,
        )
    # 校验无帧遮挡探测只属于 thin RHI surface 生命周期。
    def test_idle_present_probe_belongs_to_graphics_surface(self) -> None:
        # 读取迁移期 context 门面。
        facade = (ROOT / "src/native/presentation/contracts/traits.rs").read_text(encoding="utf-8")
        # 通用 context 不得继续声明 surface 专属探测。
        self.assertNotIn("fn test_present(&mut self)", facade)
        # 读取 thin RHI 契约。
        rhi = (ROOT / "src/platform/presentation/rhi/mod.rs").read_text(encoding="utf-8")
        # GraphicsSurface 必须显式声明无帧探测入口。
        self.assertIn("fn test_present(&mut self) -> Result<PresentTestResult>", rhi)
        # 默认 surface 必须通过 typed RHI 错误拒绝未实现能力。
        self.assertIn('rhi_not_implemented("test_present")', rhi)
        # 读取 D3D11 兼容 context 实现。
        d3d11_context = (
            ROOT / "src/native/presentation/graphics/d3d11/adapter/context/graphics.rs"
        ).read_text(encoding="utf-8")
        # D3D11 context 门面不得重复实现无帧探测。
        self.assertNotIn("fn test_present(&mut self)", d3d11_context)
        # 读取 D3D11 thin RHI surface 实现。
        d3d11_surface = (
            ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi.rs"
        ).read_text(encoding="utf-8")
        # 读取 D3D11 冻结 swapchain adapter 实现。
        d3d11_swapchain = (
            ROOT / "src/native/presentation/graphics/d3d11/adapter/swapchain.rs"
        ).read_text(encoding="utf-8")
        # Windows 专属探测必须落在 GraphicsSurface 实现中。
        self.assertIn("fn test_present(&mut self) -> Result<PresentTestResult>", d3d11_surface)
        # 探测必须继续使用无帧数据的 DXGI 标志。
        self.assertIn("DXGI_PRESENT_TEST", d3d11_swapchain)
        # 既有 Presentable/Occluded/error 映射必须留在冻结 swapchain adapter 内。
        self.assertIn("fn map_dxgi_present_test_result", d3d11_swapchain)
        # surface 只允许委托 swapchain adapter，不得保留第二份映射。
        self.assertNotIn("map_dxgi_present_test_result", d3d11_surface)
        # 读取 owner-thread context wrapper。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # wrapper 不得继续转发已经下沉的兼容方法。
        self.assertNotIn("forward_result!(test_present", thread_bound)
        # 读取 GPU backend 的最终 surface 入口。
        gpu_backend = (
            ROOT / "src/draw/backend/gpu/execution/render_backend.rs"
        ).read_text(encoding="utf-8")
        # backend 必须借用组合 RHI 后调用 surface 探测。
        self.assertIn("context.test_present()", gpu_backend)
        # backend 不得继续调用统一 context 门面的兼容探测。
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
            ROOT / "src/draw/backend/gpu/execution/render_backend_backdrop.rs"
        ).read_text(encoding="utf-8")
        # 读取通用 RHI blur wrapper，确认 overlay 不形成平行 shader 路径。
        blur_renderer = (ROOT / "src/draw/backend/rhi_renderer_blur.rs").read_text(
            # 保持源码契约读取编码稳定。
            encoding="utf-8"
        )
        # 读取场景管线失败边界。
        pipeline = (
            # 固定 ScenePipeline 实现路径。
            ROOT / "src/draw/renderer/scene_pipeline/pipeline.rs"
        ).read_text(encoding="utf-8")
        # 读取拆分后的 clean refresh 与统一失败边界。
        backdrop_refresh = (
            # 固定 ScenePipeline backdrop 子模块路径。
            ROOT / "src/draw/renderer/scene_pipeline/backdrop_refresh.rs"
        ).read_text(encoding="utf-8")
        # 合并同一 ScenePipeline component 的两个实现文件供契约审计。
        pipeline_contract = pipeline + backdrop_refresh
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
            # blur 必须以可检测未支持和 typed failure 的 Result 暴露。
            self.assertIn("fn blur_overlay_backdrop", contract)
            # blur 返回值必须区分已执行与不支持。
            self.assertIn("-> Result<bool, Error>", contract)
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
        self.assertGreaterEqual(backdrop.count("self.prepare_rhi_device()?;"), 3)
        # backdrop blur 必须使用当前同代 owner 的显式实现。
        self.assertIn("fn blur_overlay_backdrop_impl", backdrop)
        # 逻辑区域只能在 backend 边界应用一次 surface DPR。
        self.assertIn("fn lower_overlay_blur_region", backdrop)
        # backend 必须委托通用 renderer 的语义型 wrapper。
        self.assertIn("renderer.execute_overlay_backdrop_blur", backdrop)
        # wrapper 必须复用唯一通用双 pass 实现。
        self.assertIn("self.execute_blur_without_present(", blur_renderer)
        # overlay blur 的最终 target 必须直接保持同一 backdrop texture 身份。
        self.assertIn("// 最终写回目标保留同一类型化 texture 身份。\n            backdrop,", blur_renderer)
        # 恢复 copy/submit 事务不得只记录日志后返回 false。
        self.assertIn("result?;", backdrop)
        # 显式 release 必须直接返回检查式销毁结果。
        self.assertIn("self.destroy_rhi_overlay_backdrop_texture()", backdrop)
        # 场景管线必须统一生成不可提交的失败帧。
        self.assertIn("fn failed_backdrop_frame", pipeline_contract)
        # 禁止恢复曾经吞掉快照错误的无结果调用。
        self.assertNotIn("let _ = engine.snapshot_overlay_backdrop()", pipeline)
        # 恢复失败必须在 end_frame 前直接返回失败帧。
        self.assertIn(
            "Err(error) => return Self::failed_backdrop_frame(error, cur_version)", pipeline
        )
        # RecoveryDriver 必须观察 snapshot 的 begin_frame 外失败。
        self.assertIn("let result = self.engine.snapshot_overlay_backdrop();", recovery)
        # RecoveryDriver 必须观察无帧 blur 事务的 typed failure。
        self.assertIn("let result = self.engine.blur_overlay_backdrop(region, radius);", recovery)
        # RecoveryDriver 必须观察 begin_frame 后的 restore 失败。
        self.assertIn("let result = self.engine.restore_overlay_backdrop();", recovery)
        # RecoveryDriver 必须观察 idle 前的 release 失败。
        self.assertIn("let result = self.engine.release_overlay_backdrop();", recovery)

    # 校验生产 OpenGL context 的 Drop 不会静默吞掉 checked shutdown 失败。
    def test_opengl_drop_reports_checked_shutdown_failures(self) -> None:
        # 固定 EGL/WGL context、类型名与私有 shutdown 入口的对应关系。
        adapters = (
            # Linux EGL context 通过公开生命周期 trait 执行 checked shutdown。
            ("egl.rs", "EglContext", "try_shutdown"),
            # Windows WGL context 直接调用同一 owner 的 inherent shutdown。
            ("wgl.rs", "WglContext", "shutdown_result"),
        )
        # 逐个验证两个生产 OpenGL adapter 的最终 Drop 诊断边界。
        for filename, context_name, shutdown in adapters:
            # 读取当前平台 context 的完整源码。
            source = (
                ROOT / f"src/native/presentation/graphics/opengl/adapter/{filename}"
            ).read_text(encoding="utf-8")
            # 截取目标 context 的 Drop 实现之后的源码。
            drop_body = source[source.index(f"impl Drop for {context_name}") :]
            # Drop 必须显式观察既有 checked shutdown 结果。
            self.assertIn(f"if let Err(error) = self.{shutdown}()", drop_body)
            # 最终失败必须进入结构化错误日志。
            self.assertIn("tracing::error!", drop_body)
            # 日志必须保留 typed error 的稳定摘要。
            self.assertIn("error.short_what()", drop_body)
            # 静默丢弃结果的旧路径不得恢复。
            self.assertNotIn(f"let _ = self.{shutdown}()", drop_body)

    # 校验 WGL 启动期 bootstrap owner 使用可传播、可重试的检查式关闭。
    def test_wgl_bootstrap_context_uses_checked_shutdown(self) -> None:
        # 读取生产 WGL adapter 的完整源码。
        source = (
            ROOT / "src/native/presentation/graphics/opengl/adapter/wgl.rs"
        ).read_text(encoding="utf-8")
        # 截取 bootstrap owner 实现与 Drop 之间的检查式生命周期主体。
        bootstrap = source[
            source.index("impl BootstrapContext") : source.index(
                "fn create_es_context"
            )
        ]
        # bootstrap 必须暴露私有 Result 生命周期入口。
        self.assertIn("fn shutdown_result(&mut self) -> Result<(), Error>", bootstrap)
        # WGL context 必须检查解绑与删除结果。
        self.assertIn("if unsafe { wglMakeCurrent", bootstrap)
        # 删除 context 失败不得静默继续。
        self.assertIn("if unsafe { wglDeleteContext", bootstrap)
        # DC 释放必须使用已有的 checked Windows helper。
        self.assertIn("release_device_context_checked", bootstrap)
        # 隐藏窗口销毁失败也必须进入 typed error 路径。
        self.assertIn("if unsafe { DestroyWindow", bootstrap)
        # Drop 必须记录最终重试失败。
        self.assertIn("bootstrap checked shutdown failed during Drop", bootstrap)
        # 正式 context 创建路径必须显式观察临时 owner 的关闭结果。
        self.assertIn("bootstrap.shutdown_result()?", source)
        # 无法传播失败的显式 drop 路径不得恢复。
        self.assertNotIn("drop(bootstrap)", source)

    # 校验 WGL 正式 context 构造失败会传播目标 HDC 的清理失败。
    def test_wgl_construction_rollback_checks_target_hdc_release(self) -> None:
        # 读取生产 WGL adapter 的完整源码。
        source = (
            ROOT / "src/native/presentation/graphics/opengl/adapter/wgl.rs"
        ).read_text(encoding="utf-8")
        # 截取正式 WglContext 构造事务，避免把 bootstrap 检查误算为覆盖。
        # 先定位正式构造函数起点，避免命中更早的 bootstrap 生命周期入口。
        construction_start = source.index("pub(crate) fn new(native_window")
        # 从正式构造函数之后寻找 WglContext 自身的 shutdown 边界。
        construction_end = source.index(
            "fn shutdown_result(&mut self)", construction_start
        )
        # 只检查正式 context 构造与失败回滚主体。
        construction = source[construction_start:construction_end]
        # 目标 HDC 的失败回滚必须使用 checked Windows helper。
        self.assertIn("release_device_context_checked(hwnd, hdc)", construction)
        # 清理失败必须保留触发回滚的原始构造失败。
        self.assertIn(".with_source(primary_error)", construction)
        # 未检查 ReleaseDC 的旧回滚入口不得恢复。
        self.assertNotIn("release_device_context(hwnd, hdc)", construction)

    # 校验尚未交付的正式 HGLRC 由构造期临时 owner 检查式回滚。
    def test_wgl_pending_context_owns_failed_construction_cleanup(self) -> None:
        # 读取生产 WGL adapter 的完整源码。
        source = (
            ROOT / "src/native/presentation/graphics/opengl/adapter/wgl.rs"
        ).read_text(encoding="utf-8")
        # 临时 owner 必须显式记录句柄与 current 状态。
        self.assertIn("struct PendingWglContext", source)
        # 创建失败必须通过同一 helper 合并主错误与清理错误。
        self.assertGreaterEqual(source.count("pending_context.finish_failure"), 2)
        # 创建成功必须显式把唯一句柄移交给正式 WglContext。
        self.assertIn("pending_context.into_handle()", source)
        # 临时 owner 的 Drop 必须记录最终重试失败。
        self.assertIn("pending context checked shutdown failed during Drop", source)
        # 旧的裸 HGLRC 删除语句不得在正式构造事务中恢复。
        self.assertNotIn("wglDeleteContext(hglrc);", source)

    # 校验 EGL 构造期 guard 统一接管全部失败回滚与成功移交。
    def test_egl_construction_guard_owns_all_native_rollback(self) -> None:
        # 读取生产 EGL adapter 的完整源码。
        source = (
            ROOT / "src/native/presentation/graphics/opengl/adapter/egl.rs"
        ).read_text(encoding="utf-8")
        # 截取构造 guard，避免把正式 EglContext shutdown 误算为创建回滚。
        guard = source[
            source.index("struct PendingEglContext") : source.index(
                "pub struct EglContext"
            )
        ]
        # guard 必须通过清理错误的 source 保留原初始化失败。
        self.assertIn("cleanup_error.with_source(primary_error)", guard)
        # 回滚必须严格按 current、context、surface、display、native window 排序。
        operations = (
            "make_current(self.display, None, None, None)",
            "destroy_context(self.display, context)",
            "destroy_surface(self.display, surface)",
            ".terminate(self.display)",
            "wl_egl_window_destroy(self.egl_window)",
        )
        # 提取每个 native teardown 操作在 guard 中的位置。
        positions = tuple(guard.index(operation) for operation in operations)
        # 位置必须单调递增，固定依赖逆序。
        self.assertEqual(positions, tuple(sorted(positions)))
        # 每个构造失败分支必须委托同一个 guard 完成回滚。
        self.assertGreaterEqual(source.count("pending.finish_failure"), 7)
        # 成功路径必须显式移交所有 native 句柄。
        self.assertIn("pending.into_handles()", source)
        # Drop 必须记录最终重试仍失败的 typed error。
        self.assertIn("construction guard rollback failed during Drop", guard)
        # 构造函数不得恢复静默丢弃 EGL 清理结果的旧写法。
        self.assertNotIn("let _ = egl.terminate", source)
        # context 与 surface 删除结果同样不得被静默丢弃。
        self.assertNotIn("let _ = egl.destroy_", source)

    # 校验 Vulkan 构造 guard 保活 device 并统一接管交付前的 native 资源。
    def test_vulkan_construction_guard_owns_pre_context_rollback(self) -> None:
        # 读取独立的 Vulkan 构造 owner 实现。
        guard_source = (
            ROOT
            / "src/native/presentation/graphics/vulkan/adapter/context/construction.rs"
        ).read_text(encoding="utf-8")
        # 读取 VulkanContext 构造函数接线。
        methods_source = (
            ROOT / "src/native/presentation/graphics/vulkan/adapter/context/methods.rs"
        ).read_text(encoding="utf-8")
        # 只截取正式 context 交付前的构造函数，排除运行期 shutdown 清理。
        constructor = methods_source[
            methods_source.index("pub(crate) fn new") : methods_source.index(
                "pub(super) fn active_device"
            )
        ]
        # guard 必须持有共享 device lease，避免 child 清理晚于 native device 销毁。
        self.assertIn("device_lease: Option<Rc<VulkanDevice>>", guard_source)
        # 回滚必须严格按 fence、semaphore、command pool、surface 排序。
        operations = (
            "destroy_fence(self.frame_fence, None)",
            "destroy_semaphore(self.image_available, None)",
            "destroy_command_pool(self.command_pool, None)",
            "destroy_failed_surface(&self.surface_loader, self.surface)",
        )
        # 提取每个 native teardown 操作在 guard 中的位置。
        positions = tuple(guard_source.index(operation) for operation in operations)
        # 位置必须单调递增，固定依赖逆序。
        self.assertEqual(positions, tuple(sorted(positions)))
        # 每个成功创建的 device child 都必须立即登记到唯一 owner。
        self.assertIn("pending.set_command_pool(command_pool)", constructor)
        # semaphore 同样不得游离于构造 owner。
        self.assertIn("pending.set_image_available(image_available)", constructor)
        # fence 同样不得游离于构造 owner。
        self.assertIn("pending.set_frame_fence(frame_fence)", constructor)
        # 成功路径必须显式、一次性移交全部 native 句柄。
        self.assertIn("pending.into_handles()", constructor)
        # 旧的 surface 手工失败清理不得残留在构造函数。
        self.assertNotIn("destroy_failed_surface(", constructor)
        # 旧的 command-pool 手工失败清理不得残留在构造函数。
        self.assertNotIn("destroy_command_pool(", constructor)
        # 旧的 semaphore 手工失败清理不得残留在构造函数。
        self.assertNotIn("destroy_semaphore(", constructor)

    # 校验 D3D11 checked shutdown 提交关闭事实并由 Drop 兜底。
    def test_d3d11_checked_shutdown_is_idempotent_and_drop_bound(self) -> None:
        # 读取 D3D11 context 状态、生命周期与 native 方法实现。
        context_dir = ROOT / "src/native/presentation/graphics/d3d11/adapter/context"
        # 单独读取状态定义。
        state = (context_dir / "mod.rs").read_text(encoding="utf-8")
        # 单独读取 checked shutdown 实现。
        methods = (context_dir / "methods.rs").read_text(encoding="utf-8")
        # 单独读取 lifecycle 与 Drop 接线。
        graphics = (context_dir / "graphics.rs").read_text(encoding="utf-8")
        # 截取 shutdown 方法，避免混入正常 present 的 RTV 重建。
        shutdown = methods[
            methods.index("pub(super) fn shutdown_result") : methods.index(
                "pub(super) fn ensure_active"
            )
        ]
        # adapter 必须拥有唯一的关闭事实。
        self.assertIn("shutdown: bool", state)
        # 重复关闭必须在触碰 COM context 前幂等返回。
        self.assertLess(shutdown.index("if self.shutdown"), shutdown.index("self.release_rtv()"))
        # checked cleanup 必须先解绑 RTV，再清状态、flush，最后提交关闭事实。
        operations = ("self.release_rtv()", ".ClearState()", ".Flush()", "self.shutdown = true")
        # 提取每个关闭操作的位置。
        positions = tuple(shutdown.index(operation) for operation in operations)
        # 位置必须单调递增，固定关闭依赖顺序。
        self.assertEqual(positions, tuple(sorted(positions)))
        # thin RHI owner 在 shutdown 后必须拒绝重新借出。
        self.assertIn("self.ensure_active()?", graphics)
        # Drop 必须显式观察同一 checked shutdown 结果。
        self.assertIn("if let Err(error) = self.shutdown_result()", graphics)
        # Drop 不得恢复静默丢弃关闭结果的写法。
        self.assertNotIn("let _ = self.shutdown_result()", graphics)

    # 校验 Vulkan owner shutdown 与 lost-device generation 契约。
    def test_direct_vulkan_uses_owner_shutdown_and_lost_device_generation(self) -> None:
        # 组合读取 Vulkan context 的拆分模块，以保持顺序审计语义。
        context = read_rust_module(VULKAN_CONTEXT)
        # 读取共享 device 管理实现。
        device = VULKAN_DEVICE.read_text(encoding="utf-8")
        # 读取 Vulkan fault 映射实现。
        fault = VULKAN_FAULT.read_text(encoding="utf-8")
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

# 支持直接执行本测试模块。
if __name__ == "__main__":
    # 运行模块内全部 unittest。
    unittest.main()
