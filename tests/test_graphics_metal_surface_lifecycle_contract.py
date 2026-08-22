# -*- coding: utf-8 -*-
"""锁定禁用 Metal PixelUpload adapter 的诚实能力与共享 Surface 生命周期。"""

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
METAL_CONTEXT = ROOT / "src/native/presentation/graphics/metal/adapter/context.rs"
METAL_ADAPTER = ROOT / "src/native/presentation/graphics/metal/adapter/mod.rs"
MACOS_REGISTRY = ROOT / "src/native/factory/registry_macos.rs"
SURFACE_LIFECYCLE = ROOT / "src/platform/presentation/rhi/surface_lifecycle.rs"


def function_range(source: str, start: str, end: str) -> str:
    """返回两个稳定标记之间的源码片段。"""
    begin = source.index(start)
    return source[begin : source.index(end, begin)]


def without_line_comments(source: str) -> str:
    """删除 Rust 行注释，让否定断言只检查实际代码。"""
    return "\n".join(line.split("//", 1)[0] for line in source.splitlines())


class GraphicsMetalSurfaceLifecycleContractTests(unittest.TestCase):
    # 禁用诊断行必须与实际 candidate 同形，但不得改变生产选择。
    def test_disabled_registry_row_declares_cpu_pixel_upload(self) -> None:
        registry = MACOS_REGISTRY.read_text(encoding="utf-8")
        metal = function_range(registry, "id: GraphicsApi::Metal,", "id: GraphicsApi::Vulkan,")
        create = function_range(registry, "fn create_metal(", "#[cfg(feature = \"vulkan\")]")

        self.assertIn("priority: 20", metal)
        self.assertIn("status: METAL_STATUS", metal)
        self.assertIn("raster: RasterMode::Cpu", metal)
        self.assertIn("present: PresentMode::PixelUpload", metal)
        self.assertIn("create: create_metal", metal)
        self.assertIn("const METAL_STATUS: BackendStatus = BackendStatus::Disabled", registry)
        self.assertIn("Errc::NotImplemented", create)
        self.assertIn("has no production implementation on macOS", create)
        self.assertIn("id: GraphicsApi::Vulkan,\n        priority: 100,", registry)
        self.assertNotIn("presentation::graphics::metal::create", registry)

    # 实际 adapter candidate 必须继续只陈述 CPU PixelUpload，不伪造 GPU raster。
    def test_adapter_candidate_remains_cpu_pixel_upload_only(self) -> None:
        adapter = METAL_ADAPTER.read_text(encoding="utf-8")
        context = without_line_comments(METAL_CONTEXT.read_text(encoding="utf-8"))

        self.assertIn("GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Metal)", adapter)
        self.assertIn("GraphicsContextCandidate::pixel_upload", adapter)
        self.assertNotIn("GraphicsContextCandidate::gpu", adapter)
        for forbidden in (
            "impl GraphicsDevice",
            "impl GraphicsSurface",
            "GpuNative",
            "Swapchain",
            "command_queue",
            "shader_module",
            "raster_pipeline",
        ):
            self.assertNotIn(forbidden, context + adapter)

    # 初始 extent、token 与 generation 必须由唯一共享 lifecycle 发布。
    def test_context_owns_one_lifecycle_and_projects_one_token(self) -> None:
        context = METAL_CONTEXT.read_text(encoding="utf-8")
        lifecycle = SURFACE_LIFECYCLE.read_text(encoding="utf-8")
        constructor = function_range(context, "pub(crate) fn new(", "fn shutdown_result(")
        present = function_range(context, "fn present_surface(&self)", "// 为 Metal CPU")

        self.assertEqual(context.count("surface_lifecycle: RhiSurfaceLifecycle"), 1)
        self.assertNotIn("surface_generation", context)
        self.assertNotIn("    width: i32,", context[context.index("pub struct"):context.index("impl Metal")])
        self.assertNotIn("    height: i32,", context[context.index("pub struct"):context.index("impl Metal")])
        self.assertNotIn("initialized:", context)
        self.assertIn("RhiSurfaceLifecycle::uninitialized(initial_extent)", constructor)
        self.assertLess(constructor.index("begin_recreate("), constructor.index("commit_recreate("))
        self.assertLess(constructor.index("commit_recreate("), constructor.index("Ok(Self"))
        self.assertNotIn(".max(1)", constructor)
        self.assertIn("let token = self.surface_lifecycle.token();", present)
        self.assertIn("token.extent.width as i32", present)
        self.assertIn("token.extent.height as i32", present)
        self.assertIn("token.generation", present)
        self.assertNotIn("\n            0,", present)
        self.assertIn("pub(crate) fn ensure_active(&self) -> Result<()>", lifecycle)

    # resize 必须前置门禁、同尺寸无副作用、原生成功后再提交。
    def test_resize_is_transactional_and_commits_after_native_success(self) -> None:
        context = METAL_CONTEXT.read_text(encoding="utf-8")
        resize = function_range(
            context,
            "fn resize_pixel_upload_surface(&mut self",
            "\n    }\n}",
        )
        native_resize = function_range(
            context,
            "fn resize_surface_native(",
            "impl GraphicsContextLifecycle",
        )

        ensure = resize.index('self.ensure_active("resize")?')
        token = resize.index("self.surface_lifecycle.token()")
        validate = resize.index("RhiSurfaceResizeTransaction::validate(requested, current)?")
        same_extent = resize.index("if resize.extent() == current.extent")
        begin = resize.index(".begin_recreate(resize.extent(), RhiSurfaceRecreateReason::Resize)?")
        native = resize.index("self.resize_surface_native(transaction)")
        commit = resize.index(".commit_recreate(transaction, actual)?")
        self.assertLess(ensure, token)
        self.assertLess(token, validate)
        self.assertLess(validate, same_extent)
        self.assertLess(same_extent, begin)
        self.assertLess(begin, native)
        self.assertLess(native, commit)
        self.assertIn("resize.complete(current)?;\n            return Ok(());", resize)
        self.assertIn("resize.complete(commit.token())?", resize)
        self.assertIn("self.surface_lifecycle.abort_recreate(transaction)", resize)
        self.assertNotIn(".max(1)", resize)
        self.assertLess(
            native_resize.index("platform::set_metal_layer_drawable_size("),
            native_resize.index("Ok(recreate.requested())"),
        )

    # present 必须在任何 AppKit helper 前验证 owner、载荷和当前 token extent。
    def test_present_gates_owner_payload_and_extent_before_appkit(self) -> None:
        context = METAL_CONTEXT.read_text(encoding="utf-8")
        present = function_range(context, "fn present_pixels(", "// 通过共享事务更新")

        ensure = present.index('self.ensure_active("present")?')
        payload = present.index("validate_pixel_buffer(pixels, width, height)?")
        token = present.index("let token = self.surface_lifecycle.token();")
        extent = present.index("if width != current_width || height != current_height")
        helper = present.index("platform::present_layer_pixels(")
        success = present.index("Ok(())", helper)
        self.assertLess(ensure, payload)
        self.assertLess(payload, token)
        self.assertLess(token, extent)
        self.assertLess(extent, helper)
        self.assertLess(helper, success)
        self.assertIn("self.surface_lifecycle.ensure_active()", context)
        self.assertIn("after shutdown", context)

    # 上层保持单一源码，Metal adapter 不扩张为 GPU-native 实现。
    def test_upper_sources_have_no_metal_platform_branch(self) -> None:
        forbidden = (
            "GraphicsApi::Metal",
            "presentation::graphics::metal",
            "CAMetalLayer",
            "AppKit",
        )
        for upper_root in (ROOT / "src/app", ROOT / "src/ui", ROOT / "src/draw"):
            for path in upper_root.rglob("*.rs"):
                source = path.read_text(encoding="utf-8")
                with self.subTest(path=path.relative_to(ROOT)):
                    for marker in forbidden:
                        self.assertNotIn(marker, source)

    # 本阶段直接修改与契约文件必须保持在单文件上限内。
    def test_touched_files_stay_below_limit(self) -> None:
        for path in (
            METAL_CONTEXT,
            METAL_ADAPTER,
            MACOS_REGISTRY,
            SURFACE_LIFECYCLE,
            Path(__file__),
        ):
            self.assertLess(len(path.read_text(encoding="utf-8").splitlines()), 1500, path)


if __name__ == "__main__":
    unittest.main()
