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
        # RHI resize 必须从单一 surface 快照读取 DPR。
        self.assertIn("self.present_surface().device_pixel_ratio", facade)
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
        # 三条默认错误路径都应直接读取同一静态 recipe 事实。
        self.assertGreaterEqual(facade.count("let backend = self.caps().backend;"), 3)
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
