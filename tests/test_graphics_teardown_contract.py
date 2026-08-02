# -*- coding: utf-8 -*-
"""Keep production graphics routing and legacy teardown scope explicit."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
GRAPHICS = ROOT / "src/native/presentation/graphics/mod.rs"
VULKAN_CONTEXT = ROOT / "src/native/presentation/graphics/vulkan/platform/context.rs"
VULKAN_DEVICE = ROOT / "src/native/presentation/graphics/vulkan/platform/device.rs"
VULKAN_FAULT = ROOT / "src/native/presentation/graphics/vulkan/platform/fault.rs"
GFX_R5 = ROOT / "src/gfx_r5_support.rs"
REGISTRIES = (
    ROOT / "src/native/factory/registry_linux.rs",
    ROOT / "src/native/factory/registry_macos.rs",
    ROOT / "src/native/factory/registry_windows.rs",
)
LEGACY_CONTEXTS = {
    "d3d11": (
        ROOT / "src/native/presentation/graphics/d3d11/platform/context.rs",
        False,
    ),
    "d3d12": (
        ROOT / "src/native/presentation/graphics/d3d12/platform/context.rs",
        True,
    ),
    "metal": (
        ROOT / "src/native/presentation/graphics/metal/platform/context.rs",
        False,
    ),
    "vulkan": (
        ROOT / "src/native/presentation/graphics/vulkan/platform/context.rs",
        True,
    ),
    "egl": (
        ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
        True,
    ),
    "wgl": (
        ROOT / "src/native/presentation/graphics/opengl/platform/wgl.rs",
        True,
    ),
}


class GraphicsTeardownContractTests(unittest.TestCase):
    def test_production_registry_routes_active_recipes_through_wgpu(self) -> None:
        graphics = GRAPHICS.read_text(encoding="utf-8")
        self.assertIn("Production uses one shared wgpu renderer", graphics)
        self.assertIn('#[cfg(all(test, feature = "d3d11"))]', graphics)
        self.assertIn('#[cfg(all(test, feature = "d3d12"))]', graphics)
        self.assertIn('#[cfg(all(test, feature = "opengles"))]', graphics)
        for registry in REGISTRIES:
            source = registry.read_text(encoding="utf-8")
            with self.subTest(registry=registry.name):
                self.assertIn("wgpu_backend::create_", source)

    def test_legacy_contexts_have_checked_shutdown_and_drop_boundaries(self) -> None:
        for name, (path, has_explicit_drop) in LEGACY_CONTEXTS.items():
            source = path.read_text(encoding="utf-8")
            with self.subTest(context=name):
                self.assertIn("shutdown_result", source)
                self.assertIn("fn try_shutdown", source)
                if has_explicit_drop:
                    self.assertIn("impl Drop", source)

    def test_thread_bound_wrapper_calls_checked_shutdown(self) -> None:
        source = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        self.assertIn("inner.try_shutdown()", source)
        self.assertIn('self.with_owner("try_shutdown"', source)

    def test_direct_vulkan_uses_owner_shutdown_and_lost_device_generation(self) -> None:
        context = VULKAN_CONTEXT.read_text(encoding="utf-8")
        device = VULKAN_DEVICE.read_text(encoding="utf-8")
        fault = VULKAN_FAULT.read_text(encoding="utf-8")
        gfx_r5 = GFX_R5.read_text(encoding="utf-8")

        shutdown = context.index("fn shutdown_result")
        release = context.index("self.device_lease.take()", shutdown)
        close = context.index("self.surface_loader.destroy_surface", shutdown)
        self.assertLess(close, release)
        self.assertLess(release, context.index("self.runtime.take()", release))
        self.assertLess(
            context.index("self.shutdown = true", release),
            context.index("Ok(())", context.index("self.shutdown = true", release)),
        )
        self.assertIn("if let Err(error) = self.try_shutdown()", context)
        self.assertIn("std::mem::forget(device)", context)
        self.assertIn("std::mem::forget(runtime)", context)

        # Vulkan has no callback-owned PendingFailureSource: device loss is
        # synchronous and typed, while a lost shared device starts a new Rc
        # generation instead of being reused by another context.
        self.assertIn("if !device.is_lost()", device)
        self.assertIn("devices.insert(key, Rc::downgrade(&device))", device)
        self.assertIn("fn observe<T>", device)
        self.assertIn("Errc::GraphicsDeviceLost", fault)

        self.assertGreaterEqual(gfx_r5.count("VulkanContext::new"), 1)
        self.assertIn("impl RenderTarget for VulkanRecoveryTarget", gfx_r5)
        self.assertIn("fn try_shutdown(&mut self)", gfx_r5)
        self.assertIn("driver.try_shutdown()", gfx_r5)
        self.assertIn("context.try_shutdown()", gfx_r5)


if __name__ == "__main__":
    unittest.main()
