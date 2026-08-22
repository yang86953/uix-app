#!/usr/bin/env python3
"""锁定真实 Vulkan WSI 验收仍复用共享 Drawing/RHI 生命周期。"""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
DRAW_BRIDGE = ROOT / "src/draw/backend/production_chain_parity.rs"
COMPOSITION = ROOT / "src/graphics_parity.rs"
LIFECYCLE = ROOT / "src/platform/presentation/rhi/surface_lifecycle.rs"
TARGET = ROOT / "tests/vulkan_gpu_parity.rs"
VULKAN_CONTEXT = ROOT / "src/native/presentation/graphics/vulkan/adapter/context/mod.rs"
VULKAN_METHODS = ROOT / "src/native/presentation/graphics/vulkan/adapter/context/methods.rs"
VULKAN_SURFACE = ROOT / "src/native/presentation/graphics/vulkan/adapter/context/rhi_surface.rs"
VULKAN_READBACK = (
    ROOT
    / "src/native/presentation/graphics/vulkan/adapter/context/rhi_surface_readback.rs"
)
VULKAN_TEXTURE = ROOT / "src/native/presentation/graphics/vulkan/adapter/context/rhi_texture.rs"


def source(path: Path) -> str:
    """读取契约检查使用的 UTF-8 源码。"""
    return path.read_text(encoding="utf-8")


class VulkanWsiContractTests(unittest.TestCase):
    """验证 WSI 仅在测试组合根选择，Drawing 只消费共享 RHI。"""

    def test_drawing_surface_bridge_is_api_neutral_and_reuses_shared_plan(self) -> None:
        bridge = source(DRAW_BRIDGE)
        self.assertIn("execute_ui_production_chain(context.device()", bridge)
        self.assertIn("execute_sampled_quads_with_present_hook(", bridge)
        self.assertIn("PresentDamage::Full", bridge)
        self.assertIn("context.surface_ref().token()", bridge)
        for forbidden in ("crate::native", "GraphicsApi::", "Vulkan", "ash::", "vk::"):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, bridge)

    def test_real_window_harness_owns_wsi_resize_and_shutdown_order(self) -> None:
        composition = source(COMPOSITION)
        for marker in (
            "create_platform_with_pending",
            "create_app_window(",
            "production_presents: 8",
            "total_timeout: Duration::from_secs(15)",
            ".event_loop()",
            ".wait_timeout(dispatch_timeout",
            "VulkanContext::new(native_surface",
            "execute_ui_production_surface_chain",
            ".expect_err(\"zero-width WSI resize must be rejected\")",
            "rejected resize must not commit generation or extent",
            "resized.generation, first_present.generation + 1",
            "event_observation.dispatches.get() >= plan.production_presents",
            "baseline-presents=2; paced-presents={}; production-presents={production_presents}",
            "timing-claim=bounded-platform-dispatch-only",
            "verify_vulkan_surface_recovery",
            "recovery-ui-presents=4",
            "present_shared_production_scene_with_readback",
            "Vulkan WSI Surface readback verified",
            "context.parity_surface_diagnostic()",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, composition)
        self.assertEqual(composition.count("fn run_wsi_production_chain_test"), 1)
        self.assertEqual(composition.count("struct WsiPacingObservationPlan"), 1)
        self.assertEqual(composition.count("const WSI_PACING_OBSERVATION_PLAN"), 1)
        self.assertEqual(composition.count("Some(WSI_PACING_OBSERVATION_PLAN)"), 2)
        self.assertNotIn("OPENGL_WSI_PACING_PLAN", composition)
        vulkan_entry = composition[
            composition.index("fn run_vulkan_wsi_production_chain_test") :
            composition.index("fn run_opengl_wsi_production_chain_test")
        ]
        self.assertIn("Some(WSI_PACING_OBSERVATION_PLAN)", vulkan_entry)
        self.assertNotIn("None,", vulkan_entry)
        self.assertLess(composition.index(".try_shutdown()"), composition.index("window\n        .close()"))

    def test_upper_sources_do_not_own_vulkan_or_wayland_pacing(self) -> None:
        for upper_root in (ROOT / "src/app", ROOT / "src/ui", ROOT / "src/draw"):
            for path in upper_root.rglob("*.rs"):
                text = source(path)
                for forbidden in (
                    "WSI_PACING_OBSERVATION_PLAN",
                    "wait_timeout(dispatch_timeout",
                    "VulkanContext",
                    "wayland_client",
                ):
                    with self.subTest(upper=path.relative_to(ROOT), forbidden=forbidden):
                        self.assertNotIn(forbidden, text)

    def test_shared_lifecycle_and_consistency_authorities_are_not_copied(self) -> None:
        definitions = []
        pattern = re.compile(r"\bstruct\s+RhiSurfaceLifecycle\b")
        for path in (ROOT / "src").rglob("*.rs"):
            if pattern.search(source(path)):
                definitions.append(path)
        self.assertEqual(definitions, [LIFECYCLE])
        for path in (DRAW_BRIDGE, COMPOSITION):
            with self.subTest(path=path.relative_to(ROOT)):
                text = source(path)
                self.assertNotIn("CONSISTENCY_PIPELINES", text)
                self.assertNotIn("canonical_scenes", text)
                self.assertNotRegex(text, r"(?:struct|enum)\s+RhiSurfaceLifecycle\b")

    def test_fault_injection_is_feature_gated_inside_vulkan_adapter(self) -> None:
        context = source(VULKAN_CONTEXT)
        methods = source(VULKAN_METHODS)
        surface = source(VULKAN_SURFACE)
        composition = source(COMPOSITION)

        self.assertIn('#[cfg(feature = "vulkan-parity-test")]', context)
        for marker in (
            "VulkanSurfaceFaultForParity",
            "AcquireOutOfDate",
            "PresentOutOfDate",
            "AcquireSuboptimal",
            "PresentSuboptimal",
        ):
            self.assertIn(marker, context)
            self.assertIn(marker, composition)
        self.assertIn("inject_surface_fault_for_parity_test", surface)
        self.assertIn("replace_present_sync_for_parity", methods)

        acquire = surface[surface.index("fn acquire(&mut self)") : surface.index("fn resize(&mut self")]
        self.assertLess(
            acquire.index("reject_acquire_for_parity"),
            acquire.index("acquire_next_image"),
        )
        present = surface[surface.index("fn present(&mut self") :]
        self.assertLess(
            present.index("reject_present_for_parity"),
            present.index("queue_present"),
        )
        self.assertIn("present_result.map(|_| true)", present)

        for upper_root in (ROOT / "src/app", ROOT / "src/ui"):
            for path in upper_root.rglob("*.rs"):
                with self.subTest(upper=path.relative_to(ROOT)):
                    self.assertNotIn("VulkanSurfaceFaultForParity", source(path))

    def test_shared_surface_bridge_reports_presented_recreated_generation(self) -> None:
        bridge = source(DRAW_BRIDGE)

        self.assertIn("token.generation.saturating_add(1)", bridge)
        self.assertIn("Ok(current)", bridge)
        self.assertNotIn("VulkanSurfaceFaultForParity", bridge)
        self.assertNotIn("ERROR_OUT_OF_DATE_KHR", bridge)
        self.assertNotIn("SUBOPTIMAL_KHR", bridge)

    def test_transfer_src_capability_comes_from_the_real_surface_generation(self) -> None:
        methods = source(VULKAN_METHODS)
        surface = source(VULKAN_SURFACE)

        self.assertIn("caps.supported_usage_flags", methods)
        self.assertIn("contains(vk::ImageUsageFlags::TRANSFER_SRC)", methods)
        self.assertIn("required_usage | vk::ImageUsageFlags::TRANSFER_SRC", methods)
        self.assertIn(".image_usage(image_usage)", methods)
        self.assertIn(
            "self.surface_supported_usage_flags = caps.supported_usage_flags",
            methods,
        )
        self.assertIn("surface_supported_usage_flags", surface)
        self.assertIn("supports_surface_readback_format(self.swapchain_format)", surface)
        self.assertNotIn("readback: true", surface)

    def test_vulkan_readback_reuses_submit_before_present_and_immediate_sync(self) -> None:
        surface = source(VULKAN_SURFACE)
        readback = source(VULKAN_READBACK)
        texture = source(VULKAN_TEXTURE)
        bridge = source(DRAW_BRIDGE)

        method = surface[
            surface.index("fn read_surface_pixels(") : surface.index("fn present(")
        ]
        for marker in (
            "RhiSurfaceReadback::validate_region",
            "self.submitted_frame.as_ref()",
            "vk::ImageLayout::PRESENT_SRC_KHR",
            "execute_immediate",
            "RhiSurfaceReadback::try_new",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, method)
        for marker in (
            "vk::BufferUsageFlags::TRANSFER_DST",
            "vk::ImageLayout::TRANSFER_SRC_OPTIMAL",
            "cmd_copy_image_to_buffer",
            ".buffer_row_length(0)",
            "vk::AccessFlags::HOST_READ",
            "invalidate_mapped_memory_ranges",
            "B8G8R8A8_UNORM",
            "R8G8B8A8_UNORM",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, readback)
        self.assertIn("queue_wait_idle", texture)
        self.assertIn("execute_ui_production_surface_chain_with_present_hook", bridge)
        self.assertNotIn('feature = "test-harness"', readback)
        self.assertNotIn("crate::draw", readback)
        self.assertNotIn("crate::ui", readback)

    def test_explicit_vulkan_target_calls_the_real_wsi_entry(self) -> None:
        target = source(TARGET)
        self.assertIn("ui_drawing_frame_plan_presents_through_real_vulkan_wsi", target)
        self.assertIn("uix::__run_vulkan_wsi_production_chain_test();", target)


if __name__ == "__main__":
    unittest.main()
