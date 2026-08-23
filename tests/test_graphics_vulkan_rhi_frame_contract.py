"""Vulkan GPU-native 帧录制、提交、呈现与生产路由源码契约。"""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "src/native/presentation/graphics/vulkan/adapter"
FRAME = ADAPTER / "context/rhi_frame.rs"
SURFACE = ADAPTER / "context/rhi_surface.rs"
METHODS = ADAPTER / "context/methods.rs"
PIPELINE = ADAPTER / "context/rhi_pipeline.rs"
LIFECYCLE = ROOT / "src/platform/presentation/rhi/surface_lifecycle.rs"
WINDOW_DRIVER = ROOT / "src/app/window/window_driver/driver.rs"
GPU_BACKEND = ROOT / "src/draw/backend/gpu/execution/render_backend.rs"
RECOVERY_DRIVER = ROOT / "src/draw/renderer/recovery_driver.rs"
RUNTIME = ROOT / "src/draw/renderer/runtime.rs"
FINAL_SURFACE = ROOT / "src/draw/backend/gpu/execution/rhi_surface_final.rs"
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan_execution.rs"
COMMON_FRAME_SOURCES = (
    ADAPTER / "context/rhi_device.rs",
    FRAME,
    PIPELINE,
    SURFACE,
    ADAPTER / "context/rhi_texture.rs",
)
VULKAN = ADAPTER / "mod.rs"
REGISTRIES = (
    ROOT / "src/native/factory/registry_linux.rs",
    ROOT / "src/native/factory/registry_windows.rs",
    ROOT / "src/native/factory/registry_macos.rs",
)


class VulkanRhiFrameContractTests(unittest.TestCase):
    """锁定共享 RHI 事务到 Vulkan 原生对象的单向映射。"""

    def test_one_frame_owner_records_pass_draw_copy_and_submit(self) -> None:
        frame = FRAME.read_text(encoding="utf-8")
        surface = SURFACE.read_text(encoding="utf-8")

        self.assertIn("struct VulkanRhiFrame", frame)
        self.assertIn("begin_command_buffer", frame)
        self.assertIn("cmd_begin_render_pass", frame)
        self.assertIn("cmd_end_render_pass", frame)
        self.assertIn("end_command_buffer", frame)
        self.assertEqual(surface.count(".queue_submit("), 1)
        self.assertIn("self.rhi_device.issue_submission()?", surface)

    def test_pipeline_is_materialized_per_target_format_from_shared_kind(self) -> None:
        pipeline = PIPELINE.read_text(encoding="utf-8")

        self.assertIn("kind: PipelineKind", pipeline)
        self.assertIn("variants: Vec<VulkanPipelineVariant>", pipeline)
        self.assertIn("fn materialize(", pipeline)
        self.assertIn("create_graphics_pipelines", pipeline)
        self.assertIn("VulkanPipelineState::from_contract", pipeline)

    def test_surface_transaction_binds_acquire_submit_and_present(self) -> None:
        surface = SURFACE.read_text(encoding="utf-8")
        frame = FRAME.read_text(encoding="utf-8")

        self.assertIn("impl GraphicsSurface for VulkanContext", surface)
        self.assertIn("acquire_next_image", surface)
        self.assertIn("VulkanAcquiredFrame", surface)
        self.assertIn("VulkanSubmittedFrame", surface)
        self.assertIn("validate_present(transaction, current_token, coherency)", surface)
        self.assertIn("validate_latest_submission", surface)
        self.assertIn("queue_present", surface)
        self.assertIn("let wait_stages = [vk::PipelineStageFlags::ALL_COMMANDS]", surface)
        present_source = frame[frame.index("fn source_scope") : frame.index("fn destination_scope")]
        self.assertIn("vk::ImageLayout::PRESENT_SRC_KHR", present_source)
        self.assertIn("vk::PipelineStageFlags::ALL_COMMANDS", present_source)

    def test_platform_owns_surface_recreation_semantics(self) -> None:
        lifecycle = LIFECYCLE.read_text(encoding="utf-8")

        for marker in (
            "RhiSurfaceLifecycle",
            "RhiSurfaceRecreateTransaction",
            "AcquisitionRejected",
            "PresentationRejected",
            "PresentedNeedsRecreate",
            "RetryFrame",
            "Presented",
            "GraphicsSurfaceChanged",
        ):
            self.assertIn(marker, lifecycle)
        self.assertNotIn("vk::", lifecycle)
        self.assertNotIn("ash::", lifecycle)

        for path in (WINDOW_DRIVER, GPU_BACKEND, RECOVERY_DRIVER, RUNTIME, FRAME_PLAN):
            source = path.read_text(encoding="utf-8")
            with self.subTest(upper_layer=path.name):
                self.assertNotIn("OUT_OF_DATE", source)
                self.assertNotIn("SUBOPTIMAL", source)
                self.assertNotIn("vk::", source)

    def test_vulkan_mechanically_maps_out_of_date_and_suboptimal(self) -> None:
        for path in (METHODS, SURFACE):
            source = path.read_text(encoding="utf-8")
            suboptimal = source[source.index("Err(vk::Result::SUBOPTIMAL_KHR)") :]
            end = (
                suboptimal.index("Err(error)")
                if "Err(error)" in suboptimal
                else suboptimal.index("Err(err)")
            )
            suboptimal = suboptimal[:end]
            with self.subTest(source=path.name):
                self.assertIn("PresentedNeedsRecreate", suboptimal)
                self.assertIn("ERROR_OUT_OF_DATE_KHR", source)
                self.assertIn("PresentationRejected", source)
                self.assertNotIn("enum RhiSurfaceRecreateReason", source)

    def test_zero_extent_pauses_without_creating_one_by_one_surface(self) -> None:
        window = WINDOW_DRIVER.read_text(encoding="utf-8")
        gpu = GPU_BACKEND.read_text(encoding="utf-8")
        recovery = RECOVERY_DRIVER.read_text(encoding="utf-8")

        resize = window[window.index("UiEventType::WindowResize =>") :]
        resize = resize[: resize.index("UiEventType::WindowShow =>")]
        self.assertIn("data.width > 0 && data.height > 0", resize)
        self.assertIn("SurfaceSuspendReason::ZeroExtent", resize)
        self.assertIn("if width <= 0 || height <= 0", gpu)
        self.assertNotIn("let logical_w = width.max(1)", gpu)
        self.assertIn("if width <= 0 || height <= 0", recovery)

    def test_gpu_recipe_production_route_cannot_fall_back_to_pixel_upload(self) -> None:
        runtime = RUNTIME.read_text(encoding="utf-8")
        final_surface = FINAL_SURFACE.read_text(encoding="utf-8")
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")

        gpu_recipe = runtime[runtime.index("GraphicsRecipeOwner::Gpu(owner)") :]
        gpu_recipe = gpu_recipe[: gpu_recipe.index("GraphicsRecipeOwner::PixelUpload")]
        self.assertIn("GpuBackend::new_gpu_only(owner)", gpu_recipe)
        self.assertIn("Presentation::BackendManaged", gpu_recipe)
        self.assertNotIn("PixelUploadPresentation", gpu_recipe)
        self.assertIn("present_rhi_surface_texture", final_surface)
        self.assertIn("renderer.execute_sampled_quads(", final_surface)
        self.assertNotIn("present_uploaded_pixels", final_surface)
        self.assertIn("context.surface().acquire()?", frame_plan)
        self.assertIn("context.device().submit()?", frame_plan)
        self.assertIn(".present(RhiPresentTransaction::new(", frame_plan)
        self.assertNotIn("PixelUpload", frame_plan)

    def test_all_platform_registries_prioritize_vulkan_gpu_swapchain(self) -> None:
        adapter = VULKAN.read_text(encoding="utf-8")
        self.assertIn("GraphicsContextCandidate::gpu", adapter)
        self.assertIn("GraphicsContextCaps::gpu_native_swapchain", adapter)

        for registry in REGISTRIES:
            source = registry.read_text(encoding="utf-8")
            vulkan = source[source.index("id: GraphicsApi::Vulkan") :]
            vulkan = vulkan[: vulkan.index("create: create_vulkan")]
            with self.subTest(registry=registry.name):
                self.assertIn("priority: 100", vulkan)
                self.assertIn("raster: RasterMode::GpuNative", vulkan)
                self.assertIn("present: PresentMode::Swapchain", vulkan)
                self.assertNotIn("RasterMode::Cpu", vulkan)
                self.assertNotIn("PresentMode::PixelUpload", vulkan)

    def test_vulkan_rhi_core_is_one_source_across_desktop_platforms(self) -> None:
        forbidden = ("cfg(windows)", "cfg(target_os", "cfg(unix)")

        for path in COMMON_FRAME_SOURCES:
            source = path.read_text(encoding="utf-8")
            with self.subTest(source=path.name):
                for marker in forbidden:
                    self.assertNotIn(marker, source)


if __name__ == "__main__":
    unittest.main()
