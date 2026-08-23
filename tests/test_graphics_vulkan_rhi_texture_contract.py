"""Vulkan Texture、RenderTarget 与传输实现的源码契约。"""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
DEVICE = ROOT / "src/native/presentation/graphics/vulkan/adapter/context/rhi_device.rs"
TEXTURE = ROOT / "src/native/presentation/graphics/vulkan/adapter/context/rhi_texture.rs"


class VulkanRhiTextureContractTests(unittest.TestCase):
    """锁定共享语义验证与 Vulkan 对象、布局生命周期的分工。"""

    def test_texture_identity_and_render_target_use_platform_table(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")

        self.assertIn("RhiTextureResourceTable<VulkanRhiTexture>", device)
        self.assertIn("self.rhi_device.textures.resolve_render_target(texture)", device)
        self.assertIn("upload.validate(texture.desc())?", device)
        self.assertIn("copy.validate_transfer(source.desc(), destination.desc())?", device)

    def test_native_texture_has_one_image_memory_view_owner(self) -> None:
        texture = TEXTURE.read_text(encoding="utf-8")

        self.assertIn("struct VulkanRhiTexture", texture)
        self.assertIn("image: vk::Image", texture)
        self.assertIn("memory: vk::DeviceMemory", texture)
        self.assertIn("view: vk::ImageView", texture)
        self.assertIn("layout: Cell<vk::ImageLayout>", texture)
        self.assertIn("device.destroy_image_view(texture.view, None)", texture)
        self.assertIn("device.destroy_image(texture.image, None)", texture)
        self.assertIn("device.free_memory(texture.memory, None)", texture)

    def test_upload_and_copy_use_validated_regions_and_layout_barriers(self) -> None:
        texture = TEXTURE.read_text(encoding="utf-8")

        self.assertIn("upload.bounds().native_origin_and_size_i32()", texture)
        self.assertIn("bounds.source().native_origin_and_size_i32()", texture)
        self.assertIn("bounds.destination().native_origin_and_size_i32()", texture)
        self.assertNotIn("native_rect_u32()", texture)
        self.assertIn("cmd_copy_buffer_to_image", texture)
        self.assertIn("cmd_copy_image", texture)
        self.assertIn("cmd_pipeline_barrier", texture)
        self.assertIn("vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL", texture)

    def test_one_serial_executor_owns_resource_commands(self) -> None:
        texture = TEXTURE.read_text(encoding="utf-8")

        self.assertIn("struct VulkanImmediateCommands", texture)
        self.assertEqual(texture.count("fn execute<F>"), 1)
        self.assertIn("vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT", texture)
        self.assertIn("queue_wait_idle(queue)", texture)
        self.assertNotIn("#[cfg(windows)]", texture)
        self.assertNotIn("#[cfg(target_os", texture)


if __name__ == "__main__":
    unittest.main()
