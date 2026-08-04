# -*- coding: utf-8 -*-
"""Keep production graphics routing and legacy teardown scope explicit."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
GRAPHICS = ROOT / "src/native/presentation/graphics/mod.rs"
# 读取拆分后的 Vulkan context 组合模块和 shutdown 实现。
VULKAN_CONTEXT = ROOT / "src/native/presentation/graphics/vulkan/platform/context"
VULKAN_DEVICE = ROOT / "src/native/presentation/graphics/vulkan/platform/device.rs"
VULKAN_FAULT = ROOT / "src/native/presentation/graphics/vulkan/platform/fault.rs"
# 读取拆分后的 GFX-R5 support module。
GFX_R5 = ROOT / "src/gfx_r5_support"
REGISTRIES = (
    ROOT / "src/native/factory/registry_linux.rs",
    ROOT / "src/native/factory/registry_macos.rs",
    ROOT / "src/native/factory/registry_windows.rs",
)
# 维护当前所有原生 context 的 checked shutdown 证据位置。
LEGACY_CONTEXTS = {
    "d3d11": (
        ROOT / "src/native/presentation/graphics/d3d11/platform/context",
        False,
    ),
    "d3d12": (
        ROOT / "src/native/presentation/graphics/d3d12/platform/context",
        True,
    ),
    "metal": (
        ROOT / "src/native/presentation/graphics/metal/platform/context.rs",
        False,
    ),
    "vulkan": (
        ROOT / "src/native/presentation/graphics/vulkan/platform/context",
        True,
    ),
    "egl": (
        ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
        True,
    ),
    "wgl": (
        # WGL 的 IGraphicsContext/shutdown 实现已拆到同目录的 forwarding module。
        ROOT / "src/native/presentation/graphics/opengl/platform",
        True,
    ),
}


# 读取单文件或目录 module 的全部 Rust 源码。
def read_rust_module(path: Path) -> str:
    # 单文件模块直接读取 UTF-8 内容。
    if path.is_file():
        return path.read_text(encoding="utf-8")
    # 目录模块按文件名排序后拼接，保证审计稳定。
    return "\n".join(child.read_text(encoding="utf-8") for child in sorted(path.glob("*.rs")))


class GraphicsTeardownContractTests(unittest.TestCase):
    # 校验生产 registry 当前只进入原生 adapter，未残留 wgpu 创建入口。
    def test_production_registry_routes_active_recipes_through_native_adapters(self) -> None:
        # 读取 graphics 模块的 feature/adapter 门控声明。
        graphics = GRAPHICS.read_text(encoding="utf-8")
        # 默认 D3D11 与可选 OpenGL ES 必须进入生产模块。
        self.assertIn('#[cfg(feature = "d3d11")]', graphics)
        # OpenGL ES adapter 由 feature gate 控制并复用薄 RHI。
        self.assertIn('#[cfg(feature = "opengles")]', graphics)
        # 尚未完成生产验收的 API 继续保持测试门控。
        self.assertIn('#[cfg(all(test, feature = "d3d12"))]', graphics)
        # Metal 仍然保持测试门控。
        self.assertIn('#[cfg(all(test, feature = "metal"))]', graphics)
        for registry in REGISTRIES:
            # 读取平台 registry 的当前原生创建入口。
            source = registry.read_text(encoding="utf-8")
            with self.subTest(registry=registry.name):
                # 旧 wgpu factory 不得重新成为生产路由。
                self.assertNotIn("wgpu_backend::create_", source)
                # Windows/Linux 的 OpenGL ES feature 必须有真实 create 入口。
                if registry.name in {"registry_windows.rs", "registry_linux.rs"}:
                    self.assertIn("presentation::graphics::opengl::create", source)
                # macOS 当前明确返回 typed NotImplemented，而不是伪造可用 adapter。
                else:
                    self.assertIn("no production implementation", source)

    def test_legacy_contexts_have_checked_shutdown_and_drop_boundaries(self) -> None:
        for name, (path, has_explicit_drop) in LEGACY_CONTEXTS.items():
            # 目录 context 由拆分后的 Rust module 文件共同构成。
            source = read_rust_module(path)
            with self.subTest(context=name):
                self.assertIn("shutdown_result", source)
                self.assertIn("fn try_shutdown", source)
                if has_explicit_drop:
                    self.assertIn("impl Drop", source)

    def test_thread_bound_wrapper_calls_checked_shutdown(self) -> None:
        source = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        self.assertIn("inner.try_shutdown()", source)
        self.assertIn('self.with_owner("try_shutdown"', source)

    # 校验薄 RHI 在首帧前完成能力探针，并拥有 MSDF atlas 的显式释放边界。
    def test_rhi_probe_and_msdf_atlas_contract(self) -> None:
        # 读取通用 RHI、registry、renderer 和 backend shutdown 的实现。
        rhi = (ROOT / "src/native/present/rhi.rs").read_text(encoding="utf-8")
        registry = (ROOT / "src/native/factory/registry.rs").read_text(encoding="utf-8")
        renderer = (ROOT / "src/draw/backend/rhi_renderer.rs").read_text(encoding="utf-8")
        msdf = (ROOT / "src/draw/backend/rhi_renderer_msdf.rs").read_text(encoding="utf-8")
        mixed = (ROOT / "src/draw/backend/rhi_renderer_mixed.rs").read_text(encoding="utf-8")
        backend = (ROOT / "src/draw/backend/gpu/backend/render_backend.rs").read_text(
            encoding="utf-8"
        )
        # 通用 device 必须声明首帧前固定 pipeline/resource probe。
        self.assertIn("fn probe(&mut self)", rhi)
        # registry 必须在 owner thread 绑定前执行 probe 并记录成功事实。
        self.assertIn("rhi.probe()", registry)
        self.assertIn("thin RHI probe passed", registry)
        # MSDF atlas 必须有 renderer 状态、混合 lowering 入口和 shutdown 释放边界。
        self.assertIn("msdf_atlas_pages", renderer)
        self.assertIn("msdf_atlas_cache", renderer)
        self.assertIn("ensure_msdf_texture", msdf)
        self.assertIn("ensure_msdf_texture(context, quad)", mixed)
        self.assertIn("update_texture_region", rhi)
        self.assertIn("pad_msdf_payload", msdf)
        self.assertIn("release_msdf_atlas", backend)

    # 校验原生扇形已经进入统一 RHI，而不是继续走 legacy draw_sectors。
    def test_native_sector_rhi_contract(self) -> None:
        # 读取扇形 lowering、通用 renderer 和两套 adapter 的固定 ABI。
        rhi = (ROOT / "src/native/present/rhi.rs").read_text(encoding="utf-8")
        lowering = (ROOT / "src/draw/backend/gpu/rhi_lowering.rs").read_text(encoding="utf-8")
        mixed = (ROOT / "src/draw/backend/rhi_renderer_mixed.rs").read_text(encoding="utf-8")
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs").read_text(encoding="utf-8")
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        gl_pipeline = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_pipeline.rs").read_text(encoding="utf-8")
        gl_shaders = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs").read_text(encoding="utf-8")
        d3d_shader = (ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/rhi_sector.rs").read_text(encoding="utf-8")
        # 通用 probe 必须在首帧前创建 sector pipeline。
        self.assertIn("pub(crate) const SECTOR", rhi)
        self.assertIn("pipeline_keys::SECTOR", rhi)
        # lowering 和 mixed executor 必须保留 sector 的 painter order。
        self.assertIn("PendingNativeOp::Sector(sector)", lowering)
        self.assertIn("RhiOp::Sector(RhiSector", lowering)
        self.assertIn("RhiOp::Sector(sector)", mixed)
        self.assertIn("SECTOR_UNIFORM_BYTES", rhi)
        self.assertIn("SECTOR_UNIFORM_BYTES", mixed)
        # 两套 adapter 必须接受同一 64 字节常量 ABI 和 sector shader。
        self.assertIn("SECTOR_UNIFORM_BYTES", d3d11)
        self.assertIn("SECTOR_UNIFORM_BYTES", opengl)
        self.assertIn("pipeline_keys::SECTOR", gl_pipeline)
        self.assertIn("SECTOR_FRAGMENT", gl_shaders)
        self.assertIn("SECTOR_HLSL", d3d_shader)

    # 校验 FrameEncoder 已能整条降低到 RHI，并把最终 present 留给外层。
    def test_frame_encoder_rhi_segment_contract(self) -> None:
        # 读取编码帧 lowering、调用边界和不触发 present 的 FramePlan 执行器。
        lowering = (ROOT / "src/draw/backend/gpu/backend/rhi_frame.rs").read_text(encoding="utf-8")
        backend = (ROOT / "src/draw/backend/gpu/backend/render_backend.rs").read_text(encoding="utf-8")
        mixed = (ROOT / "src/draw/backend/rhi_renderer_mixed.rs").read_text(encoding="utf-8")
        plan = (ROOT / "src/draw/backend/frame_plan_execution.rs").read_text(encoding="utf-8")
        # 读取 shape pipeline 与两套 adapter 的 Additive ABI。
        rhi = (ROOT / "src/native/present/rhi.rs").read_text(encoding="utf-8")
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs").read_text(encoding="utf-8")
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        # 主要 FrameEncoder 命令必须进入同一个 lowering，而不是逐操作兼容门面。
        self.assertIn("FrameCommand::CpuSegment", lowering)
        self.assertIn("FrameCommand::PictureBlit", lowering)
        self.assertIn("FrameRasterOp::BlitGlyphs", lowering)
        self.assertIn("FrameRasterOp::FillRoundedRect", lowering)
        self.assertIn("FrameRasterOp::ScrollCopy", lowering)
        # Additive 矩形必须复用统一 shape lowering，并由 adapter 切换 blend。
        self.assertIn("SHAPE_RECT_ADDITIVE", rhi)
        self.assertIn("RhiOp::AdditiveShape", lowering)
        self.assertIn("RhiOp::AdditiveShape", mixed)
        self.assertIn("SHAPE_RECT_ADDITIVE", d3d11)
        self.assertIn("SHAPE_RECT_ADDITIVE", opengl)
        # 主 surface 与 Picture 都必须经过统一入口，且显式使用无 present 模式。
        self.assertIn("try_execute_frame_encoder_rhi", backend)
        self.assertIn("present: bool", lowering)
        self.assertIn("execute_ops", lowering)
        self.assertIn("execute_surface_segment_on_context", plan)
        self.assertIn("execute_plan_without_present", mixed)

    # 校验变换圆角矩形已回收到共享路径 tessellation，而不是继续静默回退。
    def test_affine_rounded_rect_uses_shared_mesh_lowering(self) -> None:
        # 读取 GPU 几何 helper 与入队实现。
        geometry = (ROOT / "src/draw/backend/gpu/geometry.rs").read_text(encoding="utf-8")
        queue = (ROOT / "src/draw/backend/gpu/queue.rs").read_text(encoding="utf-8")
        # helper 必须构造闭合圆角路径，队列必须走共享 path mesh。
        self.assertIn("fn rounded_rect_path", geometry)
        self.assertIn("rounded_rect_path(rect, radius)", queue)
        self.assertIn("queue_path_mesh(&path", queue)

    # 校验纹理 RHI ABI 已经使用设备空间四角而不是轴对齐矩形。
    def test_affine_textured_quad_contract(self) -> None:
        # 读取通用图片 payload、lowering 和混合顶点编码。
        present = (ROOT / "src/native/present/mod.rs").read_text(encoding="utf-8")
        lowering = (ROOT / "src/draw/backend/gpu/rhi_lowering.rs").read_text(encoding="utf-8")
        mixed = (ROOT / "src/draw/backend/rhi_renderer_mixed.rs").read_text(encoding="utf-8")
        # 图片与 sampled Picture 必须承载任意仿射四角。
        self.assertIn("pub corners: [[f32; 2]; 4]", present)
        self.assertIn("RhiTexturedQuad", lowering)
        self.assertIn("textured_vertices_values", mixed)
        self.assertIn("quad.corners", mixed)

    # 校验阴影 RHI 已经复用真实设备四角和 96 字节跨 adapter ABI。
    def test_affine_shadow_contract(self) -> None:
        # 读取阴影 lowering、renderer 和两套 adapter 的固定布局。
        shadow = (ROOT / "src/draw/backend/rhi_renderer_shadow.rs").read_text(encoding="utf-8")
        lowering = (ROOT / "src/draw/backend/gpu/rhi_lowering.rs").read_text(encoding="utf-8")
        submit = (ROOT / "src/draw/backend/gpu/submit.rs").read_text(encoding="utf-8")
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs").read_text(encoding="utf-8")
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        # lowering 必须缩放并保留 corners，不能恢复轴对齐拒绝门禁。
        self.assertIn("scale_rhi_corners(value.corners", lowering)
        self.assertIn("corners,", lowering)
        self.assertIn("convex_quad_is_valid", submit)
        # renderer 与两套 adapter 必须识别同一 96 字节 shadow ABI。
        self.assertIn("pub(crate) corners: [[f32; 2]; 4]", shadow)
        self.assertIn("uniform.size_bytes != 96", d3d11)
        self.assertIn("uniform.len() != 96", opengl)

    # 校验线性/径向渐变已经从轴对齐 AABB ABI 扩展为共享 affine quad ABI。
    def test_affine_gradient_contract(self) -> None:
        # 读取渐变 payload、queue、lowering、renderer 和两套 adapter。
        present = (ROOT / "src/native/present/mod.rs").read_text(encoding="utf-8")
        queue = (ROOT / "src/draw/backend/gpu/queue.rs").read_text(encoding="utf-8")
        lowering = (ROOT / "src/draw/backend/gpu/rhi_lowering.rs").read_text(encoding="utf-8")
        renderer = (ROOT / "src/draw/backend/rhi_renderer.rs").read_text(encoding="utf-8")
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs").read_text(encoding="utf-8")
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        shaders = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs").read_text(encoding="utf-8")
        # 两类 native gradient DTO 和通用 payload 必须带真实四角。
        self.assertIn("pub corners: [[f32; 2]; 4]", present)
        self.assertIn("pub(crate) corners: [[f32; 2]; 4]", renderer)
        self.assertIn("glyph_device_corners", queue)
        self.assertIn("scale_rhi_corners(value.corners", lowering)
        # 两套 adapter 必须接受同一个 96 字节 ABI，并使用独立 gradient vertex shader。
        self.assertIn("uniform.size_bytes != 96", d3d11)
        self.assertIn("uniform.len() != 96", opengl)
        self.assertIn("GRADIENT_VERTEX", shaders)

    # 校验 D3D11 legacy gradient 已经消费 affine 四角，而不是只使用旧 AABB。
    def test_d3d11_legacy_gradient_consumes_affine_corners(self) -> None:
        # 读取兼容队列的渐变常量和绘制入口。
        pipeline = (ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline/pipeline3.rs").read_text(encoding="utf-8")
        # 常量入口必须接收逻辑尺寸与设备空间四角。
        self.assertIn("local_w: f32", pipeline)
        self.assertIn("corners: [[f32; 2]; 4]", pipeline)
        # legacy shader 常量必须由 TL、TR、BL 恢复两条 affine 边。
        self.assertIn("let edge_x", pipeline)
        self.assertIn("let edge_y", pipeline)
        self.assertIn("origin_edge_x", pipeline)
        # 线性和径向入口都必须把真实四角传入统一常量编码。
        self.assertIn("rect.corners", pipeline)
        self.assertIn("g.corners", pipeline)

    # 校验 Picture 的 RHI/legacy 回退不会继续保留失同步的 RHI source。
    def test_picture_rhi_fallback_downgrades_resource_owner(self) -> None:
        # 读取离屏生命周期实现和资源销毁契约。
        backend = (ROOT / "src/draw/backend/gpu/backend/render_backend.rs").read_text(encoding="utf-8")
        # 回退必须先检查式销毁 RHI texture，再允许 legacy target 接管。
        self.assertIn("fn downgrade_offscreen_rhi_texture", backend)
        self.assertIn("self.downgrade_offscreen_rhi_texture(*handle)?", backend)
        self.assertIn("Picture RHI target cannot switch to legacy rendering after commit", backend)
        self.assertIn("context.destroy_texture(texture)", backend)

    def test_direct_vulkan_uses_owner_shutdown_and_lost_device_generation(self) -> None:
        # 组合读取 Vulkan context 的拆分模块，以保持顺序审计语义。
        context = read_rust_module(VULKAN_CONTEXT)
        device = VULKAN_DEVICE.read_text(encoding="utf-8")
        fault = VULKAN_FAULT.read_text(encoding="utf-8")
        # 读取 GFX-R5 拆分目录。
        gfx_r5 = read_rust_module(GFX_R5)

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
