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

    # 校验 Picture 离屏只保留一个 RHI owner，旧资源族不会复活。
    def test_picture_offscreen_has_one_rhi_owner_without_legacy_family(self) -> None:
        # 读取通用 backend 的离屏生命周期实现。
        backend = (ROOT / "src/draw/backend/gpu/backend/render_backend.rs").read_text(encoding="utf-8")
        # 读取 Picture slot 的唯一资源字段。
        backend_owner = (ROOT / "src/draw/backend/gpu/backend/mod.rs").read_text(encoding="utf-8")
        # 读取公共兼容接口和句柄声明。
        present = read_rust_module(ROOT / "src/native/present/mod.rs")
        # 读取 D3D11 adapter context 的完整拆分模块。
        d3d11 = read_rust_module(ROOT / "src/native/presentation/graphics/d3d11/platform/context/mod.rs")
        # 读取 OpenGL ES raster 的完整拆分模块。
        opengl = read_rust_module(ROOT / "src/native/presentation/graphics/opengl/raster/mod.rs")
        # Picture slot 必须直接保存必需的单一 RHI texture。
        self.assertIn("rhi_texture: TextureHandle", backend_owner)
        # slot 不得再保存平行的原生 offscreen target。
        self.assertNotIn("target: OffscreenTargetId", backend_owner)
        # 未覆盖的 Picture 或主帧 lowering 必须在 adapter 高层回退前返回 typed failure。
        self.assertIn("fn require_lossless_rhi_submission", backend)
        # 旧降级 helper 不得重新进入生产实现。
        self.assertNotIn("fn downgrade_offscreen_rhi_texture", backend)
        # 公共 presenter 不得重新导出 legacy offscreen 句柄。
        self.assertNotIn("pub struct OffscreenTargetId", present)
        # 公共 IGraphicsContext 不得重新声明高层离屏创建入口。
        self.assertNotIn("fn create_offscreen_target", present)
        # D3D11 adapter 不得重新保存或绑定平行 offscreen 槽位。
        self.assertNotIn("offscreens:", d3d11)
        # D3D11 adapter 不得重新实现高层离屏采样 blit。
        self.assertNotIn("fn blit_offscreen_target", d3d11)
        # OpenGL ES adapter 不得重新保存平行 FBO 槽位。
        self.assertNotIn("offscreens:", opengl)
        # OpenGL ES adapter 不得重新实现高层离屏采样 blit。
        self.assertNotIn("fn blit_offscreen_target", opengl)

    # 校验整面 soft fallback 透明混合入口不会绕过有边界 tile 协议。
    def test_soft_fallback_only_exposes_bounded_tile_protocol(self) -> None:
        # 读取公共兼容接口与同目录声明。
        present = read_rust_module(ROOT / "src/native/present")
        # 读取 D3D11 adapter 的 context 与 pipeline 拆分模块。
        d3d11_context = read_rust_module(ROOT / "src/native/presentation/graphics/d3d11/platform/context")
        # 读取 D3D11 adapter 的底层 pipeline 拆分模块。
        d3d11_pipeline = read_rust_module(ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline")
        # 读取 D3D12 adapter 的 context 拆分模块。
        d3d12_context = read_rust_module(ROOT / "src/native/presentation/graphics/d3d12/platform/context")
        # 读取 D3D12 adapter 的底层 pipeline 拆分模块。
        d3d12_pipeline = read_rust_module(ROOT / "src/native/presentation/graphics/d3d12/platform/pipeline")
        # 公共门面不得重新声明整面透明混合入口。
        self.assertNotIn("fn blit_soft_fallback(", present)
        # D3D11 context 不得保留同名高层入口。
        self.assertNotIn("fn blit_soft_fallback(", d3d11_context)
        # D3D11 pipeline 不得保留同名底层入口。
        self.assertNotIn("fn blit_soft_fallback(", d3d11_pipeline)
        # D3D12 context 不得保留同名高层入口。
        self.assertNotIn("fn blit_soft_fallback(", d3d12_context)
        # D3D12 pipeline 不得保留同名底层入口。
        self.assertNotIn("fn blit_soft_fallback(", d3d12_pipeline)
        # 有边界 tile 协议仍必须留在公共接口。
        self.assertIn("fn blit_soft_fallback_tile(", present)
        # 整面 replace 上传也不得重新进入兼容门面。
        self.assertNotIn("fn upload_surface_pixels(", present)

    # 校验主 FrameEncoder 与 Picture 一样只能走无损 retained RHI。
    def test_main_frame_encoder_has_no_legacy_replace_upload_fallback(self) -> None:
        # 读取主 RenderBackend 的编码帧执行状态机。
        backend = (ROOT / "src/draw/backend/gpu/backend/render_backend.rs").read_text(encoding="utf-8")
        # 定位主 FrameEncoder 执行函数的起点。
        start = backend.index("fn try_execute_encoded_frame")
        # 定位紧随其后的 Picture 生命周期入口。
        end = backend.index("fn begin_offscreen_paint", start)
        # 截取主帧状态机，避免其它兼容路径干扰断言。
        main_frame = backend[start:end]
        # pending damage 必须先通过 retained texture ClearRect lowering。
        self.assertIn("try_clear_rhi_surface_rects", main_frame)
        # encoder 与前置清理都必须经过统一无损门禁。
        self.assertIn("require_lossless_rhi_submission", main_frame)
        # 主帧不得再进入旧的逐命令 adapter 执行器。
        self.assertNotIn("self.execute_frame_encoder(", main_frame)
        # 主帧不得因 lowering 缺口放弃 retained target 并切回 swapchain。
        self.assertNotIn("abandon_rhi_surface_texture_for_legacy", main_frame)
        # 旧 FrameEncoder adapter 执行模块必须从源码树删除。
        self.assertFalse((ROOT / "src/draw/backend/gpu/backend/impl_frame.rs").exists())

    # 校验主 surface 最终提交和有序边界不再进入 direct swapchain legacy 分支。
    def test_main_surface_has_no_direct_swapchain_legacy_present(self) -> None:
        # 读取最终 present 状态机。
        present = (ROOT / "src/draw/backend/gpu/backend/render_present.rs").read_text(encoding="utf-8")
        # 读取 Picture/effect 前的主 surface 有序边界。
        lifecycle = (ROOT / "src/draw/backend/gpu/backend/impl_main.rs").read_text(encoding="utf-8")
        # 读取 canvas 提交模块，确认旧消费者已经物理删除。
        submit = (ROOT / "src/draw/backend/gpu/submit.rs").read_text(encoding="utf-8")
        # 读取 retained surface 生命周期，确认 legacy 放弃 helper 不会复活。
        retained = (ROOT / "src/draw/backend/gpu/backend/rhi_surface.rs").read_text(encoding="utf-8")
        # 生产 backend 构造本身也必须拒绝缺少组合 thin RHI 的 context。
        self.assertIn("|| !has_rhi_context", lifecycle)
        # 最终状态机必须以统一 typed 门禁拒绝未覆盖语义。
        self.assertIn("require_lossless_main_surface_submission", present)
        # 读取 retained RHI 主 surface 提交状态机。
        rhi_submit = (ROOT / "src/draw/backend/gpu/backend/rhi_submit.rs").read_text(encoding="utf-8")
        # 空新帧必须在 retained texture 内执行透明初始化。
        self.assertIn("submit_rhi_clear_only", rhi_submit)
        # 最终状态机不得自行构造兼容 Swapchain present 帧。
        self.assertNotIn("PresentFrame", present)
        # 最终状态机不得直接调用 graphics context 的 present。
        self.assertNotIn("self.gpu_ctx.present(", present)
        # 两个主 surface 状态机均不得调用逐 UI adapter 清空或提交入口。
        for source in (present, lifecycle):
            # 禁止 direct render-target clear。
            self.assertNotIn(".clear_render_target(", source)
            # 禁止 adapter 局部清理分叉。
            self.assertNotIn(".clear_rects(", source)
            # 禁止旧 native queue 消费者。
            self.assertNotIn(".submit_native(", source)
            # 禁止旧 soft tile 消费者。
            self.assertNotIn(".submit_soft(", source)
        # canvas 提交模块只保留通用 RHI lowering，不再导出 legacy native 消费者。
        self.assertNotIn("pub(crate) fn submit_native", submit)
        # canvas 提交模块不再导出 adapter soft upload 消费者。
        self.assertNotIn("pub(crate) fn submit_soft", submit)
        # retained surface 生命周期不得再提供切回 direct swapchain 的入口。
        self.assertNotIn("abandon_rhi_surface_texture_for_legacy", retained)

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
