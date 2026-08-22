"""锁定 D3D11 真实 GPU parity 对共享规范和生产 Adapter 的结构依赖。"""

from pathlib import Path
import re
import unittest

from tests.test_graphics_source_boundary_contract import (
    TARGET_CFG,
    UPPER_API,
    UPPER_ROOTS,
    rust_files,
    without_comments,
)


ROOT = Path(__file__).resolve().parents[1]
SHARED = ROOT / "src/draw/backend/rhi_renderer_consistency.rs"
RENDERER = ROOT / "src/draw/backend/rhi_renderer.rs"
DRAW_BRIDGE = ROOT / "src/draw/backend/production_chain_parity.rs"
UI_ENTRY = ROOT / "src/ui/widgets/combinators.rs"
HARNESS = ROOT / (
    "tests/unit/native/presentation/graphics/d3d11/adapter/context/"
    "rhi_device__gpu_parity_tests.rs"
)
D3D11_RHI = ROOT / (
    "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs"
)
D3D11_DRAW = ROOT / (
    "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_draw.rs"
)
D3D11_SURFACE = ROOT / (
    "src/native/presentation/graphics/d3d11/adapter/context/rhi.rs"
)
D3D11_PIPELINES = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline"
SURFACE_LIFECYCLE = ROOT / "src/platform/presentation/rhi/surface_lifecycle.rs"
WINDOWS_REGISTRY = ROOT / "src/native/factory/registry_windows.rs"
CARGO = ROOT / "Cargo.toml"
ENTRY = ROOT / "tests/d3d11_gpu_parity.rs"


class GraphicsD3d11GpuParityContractTests(unittest.TestCase):
    """防止 D3D11 runner 复制视觉规范或绕过生产 GPU Adapter。"""

    def test_d3d11_compiles_against_the_single_shared_scene_module(self) -> None:
        renderer = RENDERER.read_text(encoding="utf-8")
        shared = SHARED.read_text(encoding="utf-8")
        harness = HARNESS.read_text(encoding="utf-8")

        self.assertIn('feature = "graphics-parity-test"', renderer)
        self.assertNotIn('feature = "d3d11-parity-test"', renderer)
        self.assertEqual(renderer.count('path = "rhi_renderer_consistency.rs"'), 1)
        self.assertIn("CONSISTENCY_PIPELINES.map(scene_for_pipeline)", shared)
        self.assertIn("canonical_scenes()", harness)
        self.assertIn("validate_canonical_scenes(&scenes)", harness)
        self.assertNotIn("CONSISTENCY_PIPELINES", harness)
        self.assertNotIn("ConsistencySample {", harness)
        self.assertNotIn("ConsistencyTolerance::", harness)

    def test_runner_uses_production_d3d11_draw_submit_and_staging_readback(self) -> None:
        harness = HARNESS.read_text(encoding="utf-8")
        rhi = D3D11_RHI.read_text(encoding="utf-8")
        draw = D3D11_DRAW.read_text(encoding="utf-8")
        pipelines = "\n".join(
            path.read_text(encoding="utf-8")
            for path in D3D11_PIPELINES.glob("rhi_*.rs")
        )

        for marker in (
            "D3d11Context::new(",
            "GraphicsDevice::draw(rhi, packet)",
            "DrawRange::vertices(",
            "DrawRange::indices(",
            "GraphicsDevice::submit(",
            "D3D11_USAGE_STAGING",
            ".CopyResource(",
            ".Map(&staging",
            "sample.accepts(actual)",
            "sample.tolerance.amount()",
            'validate_samples("BlurPassTwoPass", &scenario.final_samples',
        ):
            self.assertIn(marker, harness)
        self.assertIn("self.draw_rhi_packet(packet)", rhi)
        self.assertIn("match pipeline_kind {", draw)
        self.assertIn("context.DrawIndexed(", pipelines)
        self.assertIn("context.Draw(", pipelines)
        for forbidden in ("PendingNativeOp", "PixelUpload", "mock", "fake adapter"):
            self.assertNotIn(forbidden, harness)

    def test_real_ui_and_surface_chain_reuse_shared_production_bridges(self) -> None:
        harness = HARNESS.read_text(encoding="utf-8")
        bridge = DRAW_BRIDGE.read_text(encoding="utf-8")
        ui_entry = UI_ENTRY.read_text(encoding="utf-8")

        for marker in (
            "execute_ui_production_chain",
            "execute_ui_production_surface_chain",
            "production_chain_scene",
            "render_shared_production_scene",
            "validate_production_chain_readback",
            "read_target(rhi, target)",
            "GraphicsDevice::destroy_texture",
            "Errc::GraphicsOccluded",
        ):
            with self.subTest(harness_marker=marker):
                self.assertIn(marker, harness)
        for marker in (
            "PaintContext::new(",
            "NativeGpuCanvas2D::new_gpu_only",
            "canvas.submit_rhi_solid(",
            "RhiRenderer::default().execute_sampled_quads(",
        ):
            with self.subTest(shared_bridge_marker=marker):
                self.assertIn(marker, bridge)
        self.assertIn("WidgetRender::render(&widget", ui_entry)
        self.assertNotIn("crate::platform::presentation::rhi", ui_entry)
        for forbidden in ("crate::native", "Win32", "DXGI", "D3D11", "Vulkan", "OpenGL"):
            with self.subTest(api_neutral_bridge=forbidden):
                self.assertNotIn(forbidden, bridge)

    def test_d3d11_remains_a_windows_adapter_and_vulkan_stays_reference(self) -> None:
        for root in UPPER_ROOTS:
            for path in rust_files(root):
                clean = without_comments(path.read_text(encoding="utf-8"))
                with self.subTest(upper_source=path.relative_to(ROOT)):
                    self.assertIsNone(TARGET_CFG.search(clean))
                    self.assertIsNone(UPPER_API.search(clean))

        registry = WINDOWS_REGISTRY.read_text(encoding="utf-8")
        vulkan = registry[registry.index("id: GraphicsApi::Vulkan") :]
        d3d11 = registry[registry.index("id: GraphicsApi::D3d11") :]
        self.assertIn("priority: 100", vulkan[:160])
        self.assertIn("priority: 30", d3d11[:160])

        d3d11_sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted((ROOT / "src/native/presentation/graphics/d3d11").rglob("*.rs"))
        )
        for definition in (
            r"\bstruct\s+ProductionChainScene\b",
            r"\bconst\s+CONSISTENCY_PIPELINES\b",
            r"\bfn\s+canonical_scenes\b",
            r"\bstruct\s+RhiSurfaceLifecycle\b",
        ):
            with self.subTest(no_copied_authority=definition):
                self.assertIsNone(re.search(definition, d3d11_sources))

    def test_windows_command_and_touched_files_are_explicit_and_bounded(self) -> None:
        cargo = CARGO.read_text(encoding="utf-8")
        entry = ENTRY.read_text(encoding="utf-8")

        self.assertIn(
            'd3d11-parity-test = ["d3d11", "test-harness", "graphics-parity-test"]',
            cargo,
        )
        self.assertIn('graphics-parity-test = []', cargo)
        self.assertIn('name = "d3d11_gpu_parity"', cargo)
        self.assertIn('required-features = ["d3d11-parity-test"]', cargo)
        self.assertIn("#![cfg(windows)]", entry)
        self.assertIn("__run_d3d11_gpu_parity_test", entry)
        self.assertIn(
            "cargo test --no-default-features --features d3d11-parity-test "
            "--test d3d11_gpu_parity -- --nocapture",
            entry,
        )
        for path in (SHARED, RENDERER, HARNESS, D3D11_RHI, ENTRY, Path(__file__)):
            self.assertLess(len(path.read_text(encoding="utf-8").splitlines()), 1500, path)

    def test_same_hidden_window_runner_covers_surface_lifecycle_and_recovery(self) -> None:
        harness = HARNESS.read_text(encoding="utf-8")
        surface = D3D11_SURFACE.read_text(encoding="utf-8")

        for marker in (
            "run_surface_lifecycle_test(&mut rhi)",
            "GraphicsSurface::acquire(rhi)",
            "GraphicsDevice::submit(rhi)",
            "GraphicsSurface::present(",
            "GraphicsSurface::resize(rhi, resized_extent)",
            "RhiExtent::new(0, resized_extent.height)",
            "RhiExtent::new(i32::MAX as u32 + 1, resized_extent.height)",
            "GraphicsSurface::inject_surface_lost_for_test(rhi)",
            "Errc::GraphicsSurfaceChanged",
            "Errc::GraphicsSurfaceLost",
        ):
            self.assertIn(marker, harness)
        self.assertGreaterEqual(harness.count("present_swapchain_surface(rhi);"), 3)
        self.assertIn("RhiSurfaceRecreateReason::AcquisitionRejected", surface)
        self.assertIn("RhiSurfaceRecreateReason::PresentationRejected", surface)
        self.assertNotIn("PixelUpload", harness)

        resize = surface.split("fn resize", maxsplit=1)[1].split(
            "fn read_surface_pixels", maxsplit=1
        )[0]
        self.assertLess(
            resize.index("RhiSurfaceResizeTransaction::validate"),
            resize.index("self.run_surface_recreate("),
        )
        recreate = surface.split("fn run_surface_recreate", maxsplit=1)[1].split(
            "fn recover_rejected_frame", maxsplit=1
        )[0]
        self.assertLess(
            recreate.index("begin_recreate"), recreate.index("recreate_surface_native")
        )
        self.assertLess(
            recreate.index("recreate_surface_native"), recreate.index("commit_recreate")
        )
        lifecycle_locations = [
            path
            for path in (ROOT / "src").rglob("*.rs")
            if re.search(
                r"\bstruct\s+RhiSurfaceLifecycle\b",
                without_comments(path.read_text(encoding="utf-8")),
            )
        ]
        self.assertEqual(lifecycle_locations, [SURFACE_LIFECYCLE])


if __name__ == "__main__":
    unittest.main()
