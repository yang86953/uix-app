# -*- coding: utf-8 -*-
"""Keep production graphics routing and legacy teardown scope explicit."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
GRAPHICS = ROOT / "src/native/presentation/graphics/mod.rs"
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


if __name__ == "__main__":
    unittest.main()
