"""锁定 OpenGL 真实 GPU parity 对共享规范和生产 registry 的架构依赖。"""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SHARED = ROOT / "src/draw/backend/rhi_renderer_consistency.rs"
RENDERER = ROOT / "src/draw/backend/rhi_renderer.rs"
HARNESS = ROOT / (
    "tests/unit/native/presentation/graphics/opengl/raster/"
    "rhi_device__gpu_parity_tests.rs"
)
VULKAN_HARNESS = ROOT / (
    "tests/unit/native/presentation/graphics/vulkan/adapter/context/"
    "rhi_device__gpu_parity_tests.rs"
)
CARGO = ROOT / "Cargo.toml"
REGISTRIES = (
    ROOT / "src/native/factory/registry_linux.rs",
    ROOT / "src/native/factory/registry_windows.rs",
    ROOT / "src/native/factory/registry_macos.rs",
)


class GraphicsOpenGlGpuParityContractTests(unittest.TestCase):
    """防止 OpenGL harness 复制视觉规范或改变生产后端顺序。"""

    def test_opengl_and_vulkan_compile_against_one_closed_scene_module(self) -> None:
        renderer = RENDERER.read_text(encoding="utf-8")
        shared = SHARED.read_text(encoding="utf-8")

        self.assertIn('feature = "vulkan-parity-test"', renderer)
        self.assertIn('feature = "opengl-parity-test"', renderer)
        self.assertEqual(renderer.count('path = "rhi_renderer_consistency.rs"'), 1)
        self.assertIn("CONSISTENCY_PIPELINES.map(scene_for_pipeline)", shared)
        self.assertIn("match kind {", shared)

    def test_native_harness_consumes_shared_scenes_samples_and_tolerance(self) -> None:
        harness = HARNESS.read_text(encoding="utf-8")
        vulkan = VULKAN_HARNESS.read_text(encoding="utf-8")

        for native_harness in (harness, vulkan):
            self.assertIn("rhi_renderer::consistency::{", native_harness)
            for shared_entry in (
                "canonical_scenes",
                "validate_canonical_scenes",
                "blur_subregion_scenario",
                "sample.accepts(actual)",
                "sample.tolerance.amount()",
            ):
                self.assertIn(shared_entry, native_harness)
            self.assertEqual(
                native_harness.count(
                    'validate_samples("BlurPassTwoPass", &scenario.final_samples'
                ),
                1,
            )
        self.assertNotIn("ConsistencySample {", harness)
        self.assertNotIn("ConsistencyTolerance::", harness)
        self.assertNotIn("PendingNativeOp", harness)
        self.assertNotIn("PixelUpload", harness)

    def test_harness_is_real_headless_egl_and_cargo_is_explicit(self) -> None:
        harness = HARNESS.read_text(encoding="utf-8")
        cargo = CARGO.read_text(encoding="utf-8")

        for marker in (
            "get_platform_display(",
            "create_pbuffer_surface(",
            "OPENGL_ES3_BIT",
            "rhi.draw(gl, packet)",
            "rhi.submit(gl)",
            "gl.finish()",
            "gl.read_pixels(",
        ):
            self.assertIn(marker, harness)
        self.assertNotIn("create_window_surface", harness)
        self.assertIn('opengl-parity-test = ["opengles"]', cargo)
        self.assertIn('name = "opengl_gpu_parity"', cargo)
        default_features = next(
            line for line in cargo.splitlines() if line.startswith("default = ")
        )
        self.assertNotIn("opengles", default_features)

    def test_vulkan_remains_first_on_all_production_registries(self) -> None:
        for registry in REGISTRIES:
            source = registry.read_text(encoding="utf-8")
            vulkan = source[source.index("id: GraphicsApi::Vulkan") :]
            self.assertIn("priority: 100", vulkan[:160], registry)
        for registry in REGISTRIES[:2]:
            source = registry.read_text(encoding="utf-8")
            opengl = source[source.index("id: GraphicsApi::OpenGlEs") :]
            self.assertIn("priority: 10", opengl[:160], registry)

    def test_touched_source_and_harness_files_stay_below_limit(self) -> None:
        for path in (SHARED, RENDERER, HARNESS, VULKAN_HARNESS):
            self.assertLess(len(path.read_text(encoding="utf-8").splitlines()), 1500, path)


if __name__ == "__main__":
    unittest.main()
