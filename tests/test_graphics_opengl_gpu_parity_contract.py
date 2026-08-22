"""锁定 OpenGL 真实 GPU parity 对共享规范和生产 registry 的架构依赖。"""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SHARED = ROOT / "src/draw/backend/rhi_renderer_consistency.rs"
RENDERER = ROOT / "src/draw/backend/rhi_renderer.rs"
DRAW_BRIDGE = ROOT / "src/draw/backend/production_chain_parity.rs"
COMPOSITION = ROOT / "src/graphics_parity.rs"
LIB = ROOT / "src/lib.rs"
HARNESS = ROOT / (
    "tests/unit/native/presentation/graphics/opengl/raster/"
    "rhi_device__gpu_parity_tests.rs"
)
VULKAN_HARNESS = ROOT / (
    "tests/unit/native/presentation/graphics/vulkan/adapter/context/"
    "rhi_device__gpu_parity_tests.rs"
)
CARGO = ROOT / "Cargo.toml"
GPU_TARGET = ROOT / "tests/opengl_gpu_parity.rs"
WSI_TARGET = ROOT / "tests/opengl_wsi_parity.rs"
SURFACE_LIFECYCLE = ROOT / "src/platform/presentation/rhi/surface_lifecycle.rs"
OPENGL_HOST = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
EGL = ROOT / "src/native/presentation/graphics/opengl/adapter/egl.rs"
EGL_RHI = ROOT / "src/native/presentation/graphics/opengl/adapter/egl_rhi.rs"
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

        self.assertIn('feature = "graphics-parity-test"', renderer)
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
        self.assertIn(
            'opengl-parity-test = ["opengles", "test-harness", "graphics-parity-test"]',
            cargo,
        )
        opengles_feature = next(
            line for line in cargo.splitlines() if line.startswith("opengles = ")
        )
        self.assertNotIn("test-harness", opengles_feature)
        self.assertIn('name = "opengl_gpu_parity"', cargo)
        default_features = next(
            line for line in cargo.splitlines() if line.startswith("default = ")
        )
        self.assertNotIn("opengles", default_features)

    def test_real_ui_chain_reuses_drawing_scene_and_gles_device(self) -> None:
        harness = HARNESS.read_text(encoding="utf-8")
        bridge = DRAW_BRIDGE.read_text(encoding="utf-8")

        for marker in (
            "execute_ui_production_chain",
            "production_chain_scene",
            "render_shared_production_scene",
            "validate_production_chain_readback",
            "GraphicsDevice::destroy_texture",
            "gl.finish()",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, harness)
        self.assertIn("PaintContext::new(", bridge)
        self.assertIn("NativeGpuCanvas2D::new_gpu_only", bridge)
        self.assertIn("canvas.submit_rhi_solid(", bridge)
        for forbidden in ("crate::native", "OpenGL", "EGL", "Vulkan", "D3D11"):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, bridge)

    def test_real_wayland_egl_surface_reuses_api_neutral_surface_bridge(self) -> None:
        cargo = CARGO.read_text(encoding="utf-8")
        composition = COMPOSITION.read_text(encoding="utf-8")
        lib = LIB.read_text(encoding="utf-8")
        target = WSI_TARGET.read_text(encoding="utf-8")

        self.assertIn('name = "opengl_wsi_parity"', cargo)
        self.assertIn('path = "tests/opengl_wsi_parity.rs"', cargo)
        for marker in (
            "EglContext::new(native_surface",
            "disable_swap_interval_for_parity_test",
            "execute_ui_production_surface_chain_with_present_hook",
            ".expect_err(\"zero-width WSI resize must be rejected\")",
            "resized.generation, first_present.generation + 1",
            "OpenGlSurfaceFaultForParity::Acquire",
            "OpenGlSurfaceFaultForParity::PresentAfterSubmit",
            "inject_surface_lost_for_test",
            "retry=RetryFrame(GraphicsSurfaceChanged)",
            "egl-surface-replacements=1",
            "recovered-submit=ok",
            "recovered-eglSwapBuffers=ok",
            "presents=2; shutdown=ok",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, composition)
        self.assertIn("__run_opengl_wsi_production_chain_test", lib)
        self.assertIn("uix::__run_opengl_wsi_production_chain_test();", target)
        for path in (HARNESS, COMPOSITION):
            text = path.read_text(encoding="utf-8")
            with self.subTest(no_copied_authority=path.relative_to(ROOT)):
                self.assertNotIn("CONSISTENCY_PIPELINES", text)
                self.assertNotRegex(text, r"(?:struct|enum)\s+RhiSurfaceLifecycle\b")
        lifecycle_definitions = [
            path
            for path in (ROOT / "src").rglob("*.rs")
            if "struct RhiSurfaceLifecycle" in path.read_text(encoding="utf-8")
        ]
        self.assertEqual(lifecycle_definitions, [SURFACE_LIFECYCLE])

    def test_real_egl_recovery_reuses_shared_authority_and_feature_only_probes(self) -> None:
        composition = COMPOSITION.read_text(encoding="utf-8")
        host = OPENGL_HOST.read_text(encoding="utf-8")
        egl = EGL.read_text(encoding="utf-8")
        egl_rhi = EGL_RHI.read_text(encoding="utf-8")

        self.assertIn("rhi_recover_rejected_frame", host)
        self.assertIn("RhiSurfaceRecreateReason::AcquisitionRejected", host)
        self.assertIn("RhiSurfaceRecreateReason::PresentationRejected", host)
        self.assertIn('#[cfg(feature = "test-harness")]', host)
        self.assertEqual(egl_rhi.count("self.recreate_window_surface()?"), 1)
        self.assertIn("window_surface_replacements_for_test", egl)
        self.assertIn("parity_gpu_diagnostic", egl)
        self.assertIn("query_string(Some(self.display), egl::VERSION)", egl)
        self.assertNotIn("RhiSurfaceLifecycle::uninitialized", composition)
        self.assertNotIn("recreate_window_surface", composition)
        self.assertNotIn("eglSwapBuffers(", composition)

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
        for path in (
            SHARED,
            RENDERER,
            DRAW_BRIDGE,
            COMPOSITION,
            LIB,
            HARNESS,
            VULKAN_HARNESS,
            GPU_TARGET,
            WSI_TARGET,
            OPENGL_HOST,
            EGL,
            EGL_RHI,
        ):
            self.assertLess(len(path.read_text(encoding="utf-8").splitlines()), 1500, path)


if __name__ == "__main__":
    unittest.main()
