"""共享 Canvas2D lowering、Vulkan 能力与受控回退架构门禁。"""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
PENDING = ROOT / "src/draw/backend/gpu/pending.rs"
LOWERING = ROOT / "src/draw/backend/gpu/rhi_lowering.rs"
MIXED = ROOT / "src/draw/backend/rhi_renderer_mixed.rs"
VULKAN_DEVICE = (
    ROOT / "src/native/presentation/graphics/vulkan/adapter/context/rhi_device.rs"
)
PARITY = (
    ROOT
    / "tests/unit/native/presentation/graphics/vulkan/adapter/context/rhi_device__gpu_parity_tests.rs"
)
CARGO = ROOT / "Cargo.toml"


class VulkanCanvas2dContractTests(unittest.TestCase):
    """锁定 UI 命令只经共享 lowering 进入 Vulkan，不建立平台分叉。"""

    def test_every_pending_canvas_operation_has_shared_lowering(self) -> None:
        pending = PENDING.read_text(encoding="utf-8")
        lowering = LOWERING.read_text(encoding="utf-8")
        variants = (
            "SolidRect",
            "StrokeRect",
            "Glyph",
            "LinearGradient",
            "RadialGradient",
            "Sector",
            "SolidMesh",
            "BoxShadow",
            "ImageBlit",
            "ScrollCopy",
        )

        for variant in variants:
            self.assertIn(f"{variant}(", pending)
            self.assertIn(f"PendingNativeOp::{variant}", lowering)
        self.assertNotIn("GraphicsApi::", lowering)
        self.assertNotIn("Vulkan", lowering)

    def test_shared_mixed_operations_cover_the_fixed_pipeline_set(self) -> None:
        mixed = MIXED.read_text(encoding="utf-8")
        vulkan = VULKAN_DEVICE.read_text(encoding="utf-8")
        operations = (
            "Solid",
            "Textured",
            "Sampled",
            "Coverage",
            "Msdf",
            "Gradient",
            "Shape",
            "AdditiveShape",
            "Sector",
            "Shadow",
        )

        for operation in operations:
            self.assertIn(f"RhiOp::{operation}", mixed)
        self.assertIn("GraphicsDeviceCapabilities::full_gpu_baseline()", vulkan)
        self.assertIn("capabilities.clear_rect = true", vulkan)

    def test_missing_or_unrepresentable_operations_fall_back_atomically(self) -> None:
        lowering = LOWERING.read_text(encoding="utf-8")

        self.assertIn("let Some(operation) = lower_operation", lowering)
        self.assertIn("不在未验证的混合 pass 中伪造成功", lowering)
        self.assertIn("Errc::NotImplemented", lowering)
        self.assertIn("不能无损表达时整条 native queue 原子回退", lowering)
        self.assertIn("return Ok(false);", lowering)

    def test_real_vulkan_parity_harness_is_explicit_and_shared_spec_driven(self) -> None:
        parity = PARITY.read_text(encoding="utf-8")
        cargo = CARGO.read_text(encoding="utf-8")

        self.assertIn(
            'vulkan-parity-test = ["vulkan", "graphics-parity-test"]', cargo
        )
        self.assertIn('required-features = ["vulkan-parity-test"]', cargo)
        self.assertIn("canonical_scenes", parity)
        self.assertIn("validate_canonical_scenes", parity)
        self.assertIn("sample.accepts(actual)", parity)
        self.assertIn("create_graphics_pipelines", (
            ROOT / "src/native/presentation/graphics/vulkan/adapter/context/rhi_pipeline.rs"
        ).read_text(encoding="utf-8"))
        self.assertIn("cmd_copy_image_to_buffer", parity)
        self.assertNotIn("fn canonical_scenes", parity)
        self.assertNotIn("fn blur_subregion_scenario", parity)


if __name__ == "__main__":
    unittest.main()
