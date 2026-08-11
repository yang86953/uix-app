# -*- coding: utf-8 -*-
"""锁定按 recipe 拆分后的最终图形提交边界。"""

# 导入标准单元测试框架。
import unittest
# 导入稳定的路径读取类型。
from pathlib import Path

# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]


# 聚合最终提交契约的源码守卫。
class GraphicsPresentContractTests(unittest.TestCase):
    # 校验共享 context lifecycle 不承载 PixelUpload 与 swapchain payload。
    def test_recipe_specific_present_contract_leaves_graphics_context(self) -> None:
        # 读取 present value 与 presenter 定义。
        present_module = (ROOT / "src/native/present/mod.rs").read_text(encoding="utf-8")
        # 读取 recipe 专用视图和共享生命周期契约。
        graphics_traits = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # 截取共享 lifecycle 本身，避免专用 trait 方法干扰负断言。
        lifecycle_start = graphics_traits.index("pub(crate) trait GraphicsContextLifecycle")
        # 以 GPU recipe trait 作为 lifecycle 定义终点。
        lifecycle_end = graphics_traits.index("pub(crate) trait GpuRecipeContext", lifecycle_start)
        # 保存共同生命周期片段。
        graphics_lifecycle = graphics_traits[lifecycle_start:lifecycle_end]
        # 读取 owner-thread wrapper 的两类专用转发。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # 读取 PixelUpload runtime 的最终提交状态机。
        runtime = (ROOT / "src/draw/renderer/runtime.rs").read_text(encoding="utf-8")
        # PresentFrame 载荷并集必须从生产契约物理退出。
        self.assertNotIn("PresentFrame", present_module + graphics_traits + runtime)
        # 共享 lifecycle 不承担任何最终提交方法。
        self.assertNotIn("fn present(", graphics_lifecycle)
        # CPU pixels 不能重新进入通用 context 门面。
        self.assertNotIn("fn present_pixels(", graphics_lifecycle)
        # acquired image 身份不能继续作为无实现的 context 查询。
        self.assertNotIn("fn present_image(", graphics_lifecycle)
        # PixelUploadSurface 必须同时持有 resize 与最终像素提交。
        self.assertIn("trait PixelUploadSurface", graphics_traits)
        # 专用 PixelUpload trait 必须声明像素提交方法。
        self.assertIn("fn present_pixels(", graphics_traits)
        # 未接线的 external presenter 视图不得继续扩张通用 context。
        self.assertNotIn("trait SwapchainPresentation", graphics_traits)
        # owner-thread wrapper 必须转发仍有生产消费者的 PixelUpload 视图。
        self.assertIn(
            "impl PixelUploadSurface for ThreadBoundGraphicsContext<dyn PixelUploadSurface>",
            thread_bound,
        )
        # owner-thread wrapper 不得保留无构造消费者的平行 swapchain 视图。
        self.assertNotIn("SwapchainPresentation", thread_bound)
        # 统一 PresentFrame forwarding 宏必须退出。
        self.assertNotIn("forward_result!", thread_bound)
        # Renderer 必须通过已验证 PixelUpload owner 提交像素。
        self.assertIn(".present_pixels(cpu.pixels(), width, height, damage)?", runtime)

    # 校验各 adapter 只实现其被选择 recipe 所需的提交视图。
    def test_adapters_only_implement_their_selected_present_recipe(self) -> None:
        # 收集只应实现 PixelUploadSurface 的 adapter。
        pixel_upload_contexts = (
            # Vulkan 只承接 CPU pixels 与专用 swapchain 生命周期。
            ROOT / "src/native/presentation/graphics/vulkan/platform/context/graphics.rs",
            # Metal 只承接 CAMetalLayer CPU upload。
            ROOT / "src/native/presentation/graphics/metal/platform/context.rs",
        )
        # 收集不再需要兼容 present 的 GPU context 与 fake。
        gpu_contexts = (
            # fake context 不再记录无生产消费者的统一 present。
            ROOT / "src/native/test_harness/fake_graphics_context.rs",
            # D3D11 最终提交由 thin RHI GraphicsSurface 持有。
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs",
            # D3D12 测试期 context 不再伪造统一 payload。
            ROOT / "src/native/presentation/graphics/d3d12/platform/context/graphics.rs",
            # WGL 最终提交由共享 OpenGL thin RHI surface 持有。
            ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs",
            # EGL 最终提交同样由共享 OpenGL thin RHI surface 持有。
            ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
        )
        # 每个 PixelUpload adapter 都必须只在专用 trait 中提交 pixels。
        for context in pixel_upload_contexts:
            # 读取单个实现文件。
            source = context.read_text(encoding="utf-8")
            # adapter 必须实现专用 PixelUploadSurface。
            self.assertIn("impl PixelUploadSurface", source)
            # adapter 必须提供专用 pixels 提交。
            self.assertIn("fn present_pixels(", source)
            # adapter 不得保留统一 payload。
            self.assertNotIn("PresentFrame", source)
        # 每个 thin RHI GPU context 与 fake 都必须退出兼容 present。
        for context in gpu_contexts:
            # 读取单个实现文件，避免低层 GraphicsSurface::present 造成误判。
            source = context.read_text(encoding="utf-8")
            # 类型化 context 实现文件不得再声明统一 present。
            self.assertNotIn("fn present(", source)
            # 实现文件不得再依赖 payload 并集。
            self.assertNotIn("PresentFrame", source)

    # 校验未接线的 Wayland 平行 GPU presenter 已被物理移除。
    def test_wayland_parallel_gpu_presenter_is_removed(self) -> None:
        # 固定曾经承载平行 GPU presenter 的文件路径。
        wayland_presenter = ROOT / "src/native/backends/linux/wayland/gpu_presenter.rs"
        # 零构造调用的平行提交模块不得继续编译或制造第二 owner。
        self.assertFalse(wayland_presenter.exists())
        # 读取 Wayland 子模块清单。
        wayland_module = (ROOT / "src/native/backends/linux/wayland/mod.rs").read_text(
            # 保持源码读取编码稳定。
            encoding="utf-8"
        )
        # 子模块清单不得恢复已删除的平行 presenter。
        self.assertNotIn("mod gpu_presenter", wayland_module)
        # 读取统一 context trait。
        graphics_traits = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # 通用 context 不得再暴露平行 swapchain 提交视图。
        self.assertNotIn("SwapchainPresentation", graphics_traits)
        # 读取 EGL 的生产 context 实现。
        egl = (ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs").read_text(encoding="utf-8")
        # EGL 不得保留第二套 swapchain 提交 owner。
        self.assertNotIn("SwapchainPresentation", egl)
        # 读取共享 OpenGL thin RHI surface 实现。
        rhi_host = (ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs").read_text(
            # 保持源码读取编码稳定。
            encoding="utf-8"
        )
        # 最终提交必须继续由 GraphicsSurface::present 持有。
        self.assertIn("fn present(", rhi_host)
        # 唯一 thin RHI present 必须继续调用同一原生 swap_buffers。
        self.assertIn("self.rhi_swap_buffers(damage)", rhi_host)
        # EGL 不得重新引入统一 payload。
        self.assertNotIn("PresentFrame", egl)


# 支持直接运行该契约测试文件。
if __name__ == "__main__":
    # 执行本文件内的全部测试。
    unittest.main()
