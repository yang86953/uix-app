# -*- coding: utf-8 -*-
"""锁定 D3D12 原生 Surface 复用 platform 唯一生命周期权威。"""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 D3D12 唯一 context owner、原生事务与契约投影。
D3D12_CONTEXT = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/mod.rs"
D3D12_METHODS = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/methods.rs"
D3D12_GRAPHICS = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/graphics.rs"
D3D12_RHI_SURFACE = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/rhi_surface.rs"
D3D12_ADAPTER = ROOT / "src/native/presentation/graphics/d3d12/adapter/mod.rs"
D3D12_MODULES = ROOT / "src/native/presentation/graphics/mod.rs"
# 定位共享 Surface 生命周期与 resize 事务 Component。
SURFACE_LIFECYCLE = ROOT / "src/platform/presentation/rhi/surface_lifecycle.rs"
RESIZE_TRANSACTION = ROOT / "src/platform/presentation/rhi/resize_transaction.rs"
# 定位生产 registry 与 feature 默认选择。
WINDOWS_REGISTRY = ROOT / "src/native/factory/registry_windows.rs"
CARGO = ROOT / "Cargo.toml"


# 删除行注释，让否定断言只检查真实代码。
def without_line_comments(source: str) -> str:
    return "\n".join(line.split("//", 1)[0] for line in source.splitlines())


# 集中验证 D3D12 adapter 只机械消费共享 Surface 事务。
class GraphicsD3d12SurfaceLifecycleContractTests(unittest.TestCase):
    # Context 不得保留私有 generation 或物理 extent 字段。
    def test_context_owns_exactly_one_shared_surface_lifecycle(self) -> None:
        context = D3D12_CONTEXT.read_text(encoding="utf-8")
        lifecycle = SURFACE_LIFECYCLE.read_text(encoding="utf-8")

        self.assertEqual(context.count("surface_lifecycle: RhiSurfaceLifecycle"), 1)
        self.assertNotIn("surface_generation", context)
        self.assertNotIn("    width: i32,", context)
        self.assertNotIn("    height: i32,", context)
        definitions = [
            path
            for path in (ROOT / "src").rglob("*.rs")
            if "struct RhiSurfaceLifecycle" in path.read_text(encoding="utf-8")
        ]
        self.assertEqual(definitions, [SURFACE_LIFECYCLE])
        self.assertIn("RhiSurfaceLifecycleState::Recreating(transaction.id)", lifecycle)
        self.assertIn('"RHI surface recreate transaction is stale"', lifecycle)

    # Initialize 必须先签发，原生 owner 全部成功后才发布初始 token。
    def test_initialize_wraps_the_single_native_creation_flow(self) -> None:
        methods = D3D12_METHODS.read_text(encoding="utf-8")
        start = methods.index("pub(super) fn create_with_factory(")
        end = methods.index("fn rtv_handle_from_heap(", start)
        initialize = methods[start:end]

        begin = initialize.index(
            ".begin_recreate(initial_extent, RhiSurfaceRecreateReason::Initialize)?"
        )
        native_create = initialize.index("select_hardware_adapter(&factory)?")
        back_buffers = initialize.index("Self::build_back_buffers(")
        commit = initialize.index(
            "surface_lifecycle.commit_recreate(surface_initialize, initial_extent)?"
        )
        publish = initialize.index("Ok(Self {")
        self.assertLess(begin, native_create)
        self.assertLess(native_create, back_buffers)
        self.assertLess(back_buffers, commit)
        self.assertLess(commit, publish)
        self.assertIn("surface_lifecycle.abort_recreate(surface_initialize)", initialize)
        self.assertEqual(methods.count("fn build_back_buffers("), 1)
        self.assertIn("struct PendingFenceEvent", methods)
        self.assertIn("impl Drop for PendingFenceEvent", methods)
        self.assertIn("fence_event: Some(fence_event.release())", initialize)

    # resize 的值域、旧 token 和 lifecycle 门禁必须先于所有 COM 副作用。
    def test_resize_gates_before_native_and_commits_after_success(self) -> None:
        methods = D3D12_METHODS.read_text(encoding="utf-8")
        start = methods.index("pub(crate) fn resize_result(")
        run_start = methods.index("pub(super) fn run_surface_resize(", start)
        recipe = methods[start:run_start]
        run_end = methods.index("fn resize_surface_native(", run_start)
        resize = methods[run_start:run_end]

        ensure = recipe.index("self.ensure_healthy()?")
        zero_gate = recipe.index("if width <= 0 || height <= 0")
        drawable = recipe.index("win_surface::drawable_size(")
        validate = recipe.index("RhiSurfaceResizeTransaction::validate(requested, current)?")
        shared = recipe.index("self.run_surface_resize(")
        same_extent = resize.index("if resize.extent() == current.extent")
        begin = resize.index(
            ".begin_recreate(resize.extent(), RhiSurfaceRecreateReason::Resize)?"
        )
        native = resize.index("self.resize_surface_native(")
        commit = resize.index(".commit_recreate(transaction, actual)?")
        self.assertLess(ensure, zero_gate)
        self.assertLess(zero_gate, drawable)
        self.assertLess(drawable, validate)
        self.assertLess(validate, shared)
        self.assertLess(same_extent, begin)
        self.assertLess(begin, native)
        self.assertLess(native, commit)
        self.assertIn("return resize.complete(current);", resize)
        self.assertIn("resize.complete(commit.token())", resize)
        self.assertIn("self.surface_lifecycle.abort_recreate(transaction)", resize)
        self.assertIn("Err(lifecycle_error.with_source(error))", resize)

    # 原生 resize helper 只消费封闭投影，不拥有代际或提交权。
    def test_native_resize_is_mechanical_and_transactional(self) -> None:
        methods = D3D12_METHODS.read_text(encoding="utf-8")
        start = methods.index("fn resize_surface_native(")
        end = methods.index("pub(crate) fn read_pixels_result(", start)
        native = methods[start:end]
        code = without_line_comments(native)

        projected = native.index("let (physical_width, physical_height) = recreate.native_size_i32();")
        wait = native.index("self.transition_current_buffer_to_present_and_wait()?")
        release = native.index("self.back_buffers.clear()")
        resize_buffers = native.index("self.swap_chain.ResizeBuffers(")
        rebuild = native.index("self.rebuild_back_buffers()")
        self.assertLess(projected, wait)
        self.assertLess(wait, release)
        self.assertLess(release, resize_buffers)
        self.assertLess(resize_buffers, rebuild)
        self.assertNotIn("surface_lifecycle", code)
        self.assertNotIn("generation", code)
        self.assertNotIn("commit_recreate", code)
        self.assertIn("return Err(resize_error);", native)
        self.assertIn("self.latch_fault(\"rebuild resized back buffers\", &error)", native)

    # PresentSurface 必须从同一 token 投影 extent 与 generation。
    def test_present_surface_consumes_the_shared_token(self) -> None:
        graphics = D3D12_GRAPHICS.read_text(encoding="utf-8")
        start = graphics.index("fn present_surface(")
        end = graphics.index("fn try_shutdown(", start)
        present = graphics[start:end]

        self.assertIn("let token = self.surface_lifecycle.token();", present)
        self.assertIn("let width = token.extent.width as i32;", present)
        self.assertIn("let height = token.extent.height as i32;", present)
        self.assertIn("token.generation", present)
        self.assertNotIn("self.width", present)
        self.assertNotIn("self.height", present)
        self.assertNotIn("\n            0,", present)

    # 通用 Surface 必须只实现一次，并机械消费共享 token、resize 与 readback 合同。
    def test_single_graphics_surface_maps_existing_native_primitives(self) -> None:
        context = D3D12_CONTEXT.read_text(encoding="utf-8")
        surface = D3D12_RHI_SURFACE.read_text(encoding="utf-8")
        d3d12_sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (ROOT / "src/native/presentation/graphics/d3d12").rglob("*.rs")
        )

        self.assertIn("mod rhi_surface;", context)
        self.assertEqual(d3d12_sources.count("impl GraphicsSurface for"), 1)
        self.assertIn("impl GraphicsSurface for super::D3d12Context", surface)
        self.assertIn("self.surface_lifecycle.token()", surface)
        acquire = surface[surface.index("fn acquire("):surface.index("fn resize(")]
        self.assertLess(
            acquire.index("self.ensure_healthy()?"),
            acquire.index("self.surface_lifecycle.ensure_active()?"),
        )
        self.assertLess(
            acquire.index("self.surface_lifecycle.ensure_active()?"),
            acquire.index("SurfaceFrame::new"),
        )
        self.assertIn("if self.frame_index >= self.back_buffers.len()", acquire)
        resize = surface[surface.index("fn resize("):surface.index("fn read_surface_pixels(")]
        self.assertLess(
            resize.index("RhiSurfaceResizeTransaction::validate(extent, current)?"),
            resize.index("self.run_surface_resize("),
        )
        readback = surface[
            surface.index("fn read_surface_pixels("):surface.index("fn present(")
        ]
        self.assertLess(
            readback.index("self.surface_lifecycle.ensure_active()?"),
            readback.index("RhiSurfaceReadback::validate_region(region, token.extent)?"),
        )
        self.assertLess(
            readback.index("RhiSurfaceReadback::validate_region(region, token.extent)?"),
            readback.index("self.read_pixels_result("),
        )
        self.assertLess(
            readback.index("self.read_pixels_result("),
            readback.index("RhiSurfaceReadback::try_new("),
        )

    # Surface 能力必须与 flip-discard 和真实回读方法同源，禁止重复声明保留事实。
    def test_surface_capabilities_match_actual_methods(self) -> None:
        adapter = D3D12_ADAPTER.read_text(encoding="utf-8")
        surface = D3D12_RHI_SURFACE.read_text(encoding="utf-8")
        d3d12_sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (ROOT / "src/native/presentation/graphics/d3d12").rglob("*.rs")
        )

        self.assertEqual(d3d12_sources.count("PresentCoherency::FullOnly"), 1)
        self.assertIn("const D3D12_PRESENT_COHERENCY", adapter)
        self.assertIn(
            "GraphicsContextCaps::gpu_native_swapchain(GraphicsApi::D3d12, D3D12_PRESENT_COHERENCY)",
            adapter,
        )
        self.assertIn("GraphicsSurfaceCapabilities::with_readback(", surface)
        self.assertIn("super::super::D3D12_PRESENT_COHERENCY", surface)
        self.assertIn("fn read_surface_pixels(", surface)
        self.assertIn("fn present(", surface)

    # checked shutdown 与缺失 Device 必须先拒绝，thin RHI 和 registry 仍保持未激活。
    def test_shutdown_gate_precedes_recipe_use_and_rhi_stays_planned(self) -> None:
        graphics = D3D12_GRAPHICS.read_text(encoding="utf-8")
        methods = D3D12_METHODS.read_text(encoding="utf-8")
        context = D3D12_CONTEXT.read_text(encoding="utf-8")
        surface = D3D12_RHI_SURFACE.read_text(encoding="utf-8")
        rhi_start = graphics.index("fn rhi_context(")
        rhi_end = graphics.index("fn resize_surface(", rhi_start)
        rhi = graphics[rhi_start:rhi_end]
        resize_start = methods.index("pub(crate) fn resize_result(")
        resize_end = methods.index("fn resize_surface_native(", resize_start)
        resize = methods[resize_start:resize_end]

        self.assertLess(rhi.index("self.ensure_healthy()?"), rhi.index("Err(Error::new("))
        self.assertIn("Errc::NotImplemented", rhi)
        self.assertIn('"D3D12 thin RHI is not implemented"', rhi)
        self.assertLess(resize.index("self.ensure_healthy()?"), resize.index("surface_lifecycle.token()"))
        d3d12_sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (ROOT / "src/native/presentation/graphics/d3d12").rglob("*.rs")
        )
        self.assertNotIn("impl GraphicsDevice for D3d12Context", d3d12_sources)
        self.assertIn("rhi_submissions: RhiSubmissionSequence", context)
        self.assertIn("rhi_submissions: RhiSubmissionSequence::new()", methods)
        self.assertNotIn(".issue()", without_line_comments(d3d12_sources))
        present = surface[surface.index("fn present("):]
        self.assertLess(
            present.index("self.surface_lifecycle.ensure_active()?"),
            present.index("transaction.validate(current, coherency, &self.rhi_submissions)?"),
        )
        self.assertLess(
            present.index("transaction.validate(current, coherency, &self.rhi_submissions)?"),
            present.index("self.present_result(&present)"),
        )
        native_present = methods[
            methods.index("pub(super) fn present_result("):
            methods.index("pub(crate) fn latch_present_result(")
        ]
        self.assertIn("_present: &ValidatedRhiPresent", native_present)
        self.assertIn("self.swap_chain.Present(1, DXGI_PRESENT(0))", native_present)

    # 生产选择与上层单一源码必须不受 Planned adapter 变更影响。
    def test_registry_defaults_and_upper_sources_remain_unchanged(self) -> None:
        registry = WINDOWS_REGISTRY.read_text(encoding="utf-8")
        cargo = CARGO.read_text(encoding="utf-8")
        modules = D3D12_MODULES.read_text(encoding="utf-8")

        self.assertNotIn("GraphicsApi::D3d12", registry)
        self.assertIn('#[cfg(feature = "d3d12")]\npub(crate) mod d3d12;', modules)
        self.assertNotIn('#[cfg(all(test, feature = "d3d12"))]', modules)
        self.assertIn("id: GraphicsApi::D3d11,\n        priority: 30,", registry)
        self.assertIn("id: GraphicsApi::Vulkan,\n        priority: 100,", registry)
        self.assertIn("id: GraphicsApi::OpenGlEs,\n        priority: 10,", registry)
        default_features = next(
            line for line in cargo.splitlines() if line.startswith("default = ")
        )
        self.assertNotIn("d3d12", default_features)
        for upper_root in (ROOT / "src/app", ROOT / "src/ui", ROOT / "src/draw"):
            for path in upper_root.rglob("*.rs"):
                with self.subTest(path=path.relative_to(ROOT)):
                    self.assertNotIn("d3d12", path.read_text(encoding="utf-8").lower())

    # 本阶段所有源码与契约文件均不得超过单文件上限。
    def test_touched_files_stay_below_limit(self) -> None:
        for path in (
            D3D12_CONTEXT,
            D3D12_METHODS,
            D3D12_GRAPHICS,
            D3D12_RHI_SURFACE,
            D3D12_ADAPTER,
            D3D12_MODULES,
            Path(__file__),
        ):
            self.assertLess(len(path.read_text(encoding="utf-8").splitlines()), 1500, path)


# 支持直接执行这一精确契约测试。
if __name__ == "__main__":
    unittest.main()
