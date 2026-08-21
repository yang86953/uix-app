"""Vulkan GPU-native RHI 资源所有权与阶段能力源码契约。"""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
CONTEXT = ROOT / "src/native/presentation/graphics/vulkan/adapter/context/mod.rs"
METHODS = ROOT / "src/native/presentation/graphics/vulkan/adapter/context/methods.rs"
DEVICE = ROOT / "src/native/presentation/graphics/vulkan/adapter/context/rhi_device.rs"


class VulkanRhiResourceContractTests(unittest.TestCase):
    """锁定 platform 资源表所有权和 Vulkan 原生对象生命周期。"""

    def test_context_has_one_platform_backed_rhi_device(self) -> None:
        context = CONTEXT.read_text(encoding="utf-8")
        methods = METHODS.read_text(encoding="utf-8")

        self.assertIn("rhi_device: VulkanRhiDevice", context)
        self.assertEqual(
            methods.count("rhi_device: VulkanRhiDevice::new(uniform_alignment)"), 1
        )

    def test_buffer_and_sampler_use_shared_resource_tables(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")

        self.assertIn("RhiBufferResourceTable<VulkanRhiBuffer>", device)
        self.assertIn("RhiResourceTable<SamplerHandle, VulkanRhiSampler>", device)
        self.assertIn("impl RhiBufferResource for VulkanRhiBuffer", device)
        self.assertIn("let native_desc = desc.validate()?", device)
        self.assertIn("let validated = upload.validate(resource.desc)?", device)
        self.assertNotIn("HashMap", device)

    def test_native_resources_are_destroyed_before_vulkan_parents(self) -> None:
        methods = METHODS.read_text(encoding="utf-8")

        resource_shutdown = methods.index("self.rhi_device.shutdown(&self.device)")
        command_pool_shutdown = methods.index("destroy_command_pool(self.command_pool")
        self.assertLess(resource_shutdown, command_pool_shutdown)

    def test_gpu_native_stage_claims_and_implements_complete_device_baseline(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")

        self.assertIn("GraphicsDeviceCapabilities::full_gpu_baseline()", device)
        self.assertIn("capabilities.clear_rect = true", device)
        for operation in (
            "begin_render_pass",
            "draw",
            "end_render_pass",
            "submit",
        ):
            self.assertIn(f"fn {operation}(", device)
            self.assertNotIn(f'vulkan_rhi_unavailable("{operation}")', device)

    def test_deferred_draws_use_one_shared_frame_upload_snapshot_arena(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        frame = (
            ROOT
            / "src/native/presentation/graphics/vulkan/adapter/context/rhi_frame.rs"
        ).read_text(encoding="utf-8")

        self.assertIn("shadow: Vec<u8>", device)
        self.assertIn("struct VulkanFrameUploadArena", frame)
        self.assertIn("upload_buffer_snapshot", frame)
        self.assertIn("VERTEX_BUFFER", frame)
        self.assertIn("INDEX_BUFFER", frame)
        self.assertIn("UNIFORM_BUFFER", frame)
        self.assertIn("self.uploads.reset()", frame)
        self.assertIn("self.uploads.shutdown(device)", frame)

    def test_resource_destroy_waits_for_the_recorded_frame_boundary(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")

        destroy_impl = device[device.index("fn destroy_buffer(&mut self, buffer") :]
        self.assertGreaterEqual(destroy_impl.count("ensure_rhi_frame_prepared()"), 4)


if __name__ == "__main__":
    unittest.main()
