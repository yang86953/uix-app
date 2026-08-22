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


def source(path: Path) -> str:
    """读取契约检查使用的 UTF-8 源码。"""
    return path.read_text(encoding="utf-8")


class VulkanWsiContractTests(unittest.TestCase):
    """验证 WSI 仅在测试组合根选择，Drawing 只消费共享 RHI。"""

    def test_drawing_surface_bridge_is_api_neutral_and_reuses_shared_plan(self) -> None:
        bridge = source(DRAW_BRIDGE)
        self.assertIn("execute_ui_production_chain(context.device()", bridge)
        self.assertIn("execute_sampled_quads(", bridge)
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
            "VulkanContext::new(native_surface",
            "execute_ui_production_surface_chain",
            ".expect_err(\"zero-width WSI resize must be rejected\")",
            "rejected resize must not commit generation or extent",
            "resized.generation, first_present.generation + 1",
            "baseline-presents=2; shutdown=ok",
            "verify_vulkan_surface_recovery",
            "recovery-ui-presents=4",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, composition)
        self.assertLess(composition.index(".try_shutdown()"), composition.index("window\n        .close()"))

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

    def test_explicit_vulkan_target_calls_the_real_wsi_entry(self) -> None:
        target = source(TARGET)
        self.assertIn("ui_drawing_frame_plan_presents_through_real_vulkan_wsi", target)
        self.assertIn("uix::__run_vulkan_wsi_production_chain_test();", target)


if __name__ == "__main__":
    unittest.main()
