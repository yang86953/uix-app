"""Vulkan GPU-native 帧录制、提交、呈现与生产路由源码契约。"""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "src/native/presentation/graphics/vulkan/adapter"
FRAME = ADAPTER / "context/rhi_frame.rs"
SURFACE = ADAPTER / "context/rhi_surface.rs"
PIPELINE = ADAPTER / "context/rhi_pipeline.rs"
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

        self.assertIn("impl GraphicsSurface for VulkanContext", surface)
        self.assertIn("acquire_next_image", surface)
        self.assertIn("VulkanAcquiredFrame", surface)
        self.assertIn("VulkanSubmittedFrame", surface)
        self.assertIn("validate_present(transaction, current_token, coherency)", surface)
        self.assertIn("validate_latest_submission", surface)
        self.assertIn("queue_present", surface)

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
