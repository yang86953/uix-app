"""锁定 D3D11 真实 GPU parity 对共享规范和生产 Adapter 的结构依赖。"""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SHARED = ROOT / "src/draw/backend/rhi_renderer_consistency.rs"
RENDERER = ROOT / "src/draw/backend/rhi_renderer.rs"
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


if __name__ == "__main__":
    unittest.main()
