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

    # 校验 soft fallback 只保留 draw 私有 staging，不再进入逐 UI adapter。
    def test_soft_fallback_uses_only_draw_private_rhi_staging(self) -> None:
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
        # 读取 OpenGL ES context 与 raster pipeline 的组合源码。
        opengl = read_rust_module(ROOT / "src/native/presentation/graphics/opengl")
        # 读取通用 renderer 私有的 soft tile 打包模块。
        draw_tile = (ROOT / "src/draw/backend/gpu/tile.rs").read_text(encoding="utf-8")
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
        # 公共接口不得重新声明 compact tile adapter 方法。
        self.assertNotIn("fn blit_soft_fallback_tile(", present)
        # 三个 native adapter 均不得保留 compact tile 高层或私有上传方法。
        for adapter in (d3d11_context, d3d11_pipeline, d3d12_context, d3d12_pipeline, opengl):
            # 防止逐 UI soft tile 分支在任一 adapter 中复活。
            self.assertNotIn("blit_soft_fallback_tile", adapter)
        # platform capability 不得继续宣称已经删除的 soft upload API。
        self.assertNotIn("soft_blit", present)
        # tight tile 只作为 draw backend 的 sampled staging 私有类型存在。
        self.assertIn("pub(crate) struct SoftFallbackTile", draw_tile)
        # 通用 renderer 必须继续保留可见像素的紧边界打包。
        self.assertIn("fn pack_visible_soft_fallback_tile", draw_tile)
        # platform present 门面不得继续持有 renderer staging DTO。
        self.assertNotIn("pub struct SoftFallbackTile", present)
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

    # 校验 legacy clear/bind 只剩 thin RHI 所需的低层 target 恢复。
    def test_legacy_clear_and_bind_leave_the_graphics_context_facade(self) -> None:
        # 读取公共兼容接口与 capability profile。
        present = read_rust_module(ROOT / "src/native/present")
        # 读取 owner-thread 转发门面。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # 读取 D3D11 兼容 trait 实现。
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs").read_text(encoding="utf-8")
        # 读取 D3D12 兼容 trait 实现。
        d3d12 = (ROOT / "src/native/presentation/graphics/d3d12/platform/context/graphics.rs").read_text(encoding="utf-8")
        # 读取 WGL 兼容 trait 实现。
        wgl = (ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs").read_text(encoding="utf-8")
        # 读取 EGL 兼容 trait 实现。
        egl = (ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs").read_text(encoding="utf-8")
        # 逐一检查被删除的高层 clear/bind 方法。
        for method in ("clear_render_target", "clear_rects", "bind_swapchain_target"):
            # 公共门面不得重新声明已退出的逐 UI 方法。
            self.assertNotIn(f"fn {method}(", present)
            # owner-thread 门面不得重新转发已退出的方法。
            self.assertNotIn(f"forward_result!({method}", thread_bound)
            # 四个 adapter 的 trait wrapper 均不得复活。
            for adapter in (d3d11, d3d12, wgl, egl):
                # 兼容实现中不能再出现该方法声明。
                self.assertNotIn(f"fn {method}(", adapter)
        # capability profile 不得继续宣称已删除的完整清理入口。
        self.assertNotIn("pub clear_target:", present)
        # capability profile 不得继续宣称已删除的局部清理入口。
        self.assertNotIn("pub clear_rects:", present)
        # 读取 D3D11 thin RHI 共用的低层 target 恢复 helper。
        d3d11_methods = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/methods.rs").read_text(encoding="utf-8")
        # D3D11 最终 present 与 RHI acquire 仍须能恢复 swapchain RTV。
        self.assertIn("pub(super) fn bind_swapchain_target", d3d11_methods)
        # 读取 OpenGL thin RHI 共用的低层 framebuffer 恢复 helper。
        opengl_pipeline = (ROOT / "src/native/presentation/graphics/opengl/raster/pipeline2.rs").read_text(encoding="utf-8")
        # OpenGL RHI pass 完成后仍须能恢复默认 framebuffer。
        self.assertIn("pub(crate) fn bind_swapchain_target", opengl_pipeline)

    # 校验所有 context present 都经过显式 PresentFrame 载荷边界。
    def test_swap_buffers_compatibility_entry_leaves_graphics_context(self) -> None:
        # 读取 IGraphicsContext 的唯一兼容 present 契约。
        graphics_trait = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # 读取 owner-thread wrapper，确认没有旧交换转发。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # 收集所有直接实现 IGraphicsContext 的测试与原生 context。
        contexts = (
            # fake context 也必须模拟统一载荷，而不是保留测试旁路。
            ROOT / "src/native/test_harness/fake_graphics_context.rs",
            # D3D11 context 只实现显式 present。
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs",
            # D3D12 context 只实现显式 present。
            ROOT / "src/native/presentation/graphics/d3d12/platform/context/graphics.rs",
            # Vulkan PixelUpload context 必须拒绝 Swapchain payload。
            ROOT / "src/native/presentation/graphics/vulkan/platform/context/graphics.rs",
            # Metal PixelUpload context 必须拒绝 Swapchain payload。
            ROOT / "src/native/presentation/graphics/metal/platform/context.rs",
            # WGL context 显式处理 Swapchain payload。
            ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs",
            # EGL context 显式处理 Swapchain payload。
            ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
        )
        # 公共 trait 不得再暴露绕过 PresentFrame 的交换方法。
        self.assertNotIn("fn swap_buffers(", graphics_trait)
        # 公共 present 必须是实现方不可跳过的必需方法。
        self.assertIn("fn present(&mut self, frame: &PresentFrame) -> Result<(), Error>;", graphics_trait)
        # owner-thread wrapper 不得重新生成旧交换入口。
        self.assertNotIn("forward_result!(swap_buffers", thread_bound)
        # 每个直接 context 都必须显式处理统一 present payload。
        for context in contexts:
            # 读取单个实现文件，避免低层原生 swap API 造成误判。
            source = context.read_text(encoding="utf-8")
            # trait wrapper 中不得存在旧交换方法声明。
            self.assertNotIn("fn swap_buffers(", source)
            # 每个实现必须提供自己的 payload 校验与提交边界。
            self.assertIn("fn present(", source)
        # 读取 Wayland 的 IPresenter 桥接实现。
        wayland = (ROOT / "src/native/backends/linux/wayland/gpu_presenter.rs").read_text(encoding="utf-8")
        # Wayland 必须构造统一 Swapchain payload。
        self.assertIn("PresentFrame::Swapchain", wayland)
        # Wayland 必须调用 context 的统一 present 方法。
        self.assertIn("self.gpu_ctx.present(&frame)", wayland)
        # Wayland 不得再直接调用 context 交换旁路。
        self.assertNotIn("self.gpu_ctx.swap_buffers", wayland)

    # 校验 native context 构造成功即就绪，不再保留二阶段初始化门面。
    def test_initialize_compatibility_entry_leaves_graphics_context(self) -> None:
        # 读取公共图形上下文 trait。
        graphics_trait = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # 读取 owner-thread wrapper。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # 读取 registry 的单候选构造路径。
        registry = (ROOT / "src/native/factory/registry.rs").read_text(encoding="utf-8")
        # 收集所有直接实现 IGraphicsContext 的测试与原生 context。
        contexts = (
            # fake context 必须模拟构造即就绪事实。
            ROOT / "src/native/test_harness/fake_graphics_context.rs",
            # D3D11 context 不再保留空初始化实现。
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs",
            # D3D12 context 不再保留空初始化实现。
            ROOT / "src/native/presentation/graphics/d3d12/platform/context/graphics.rs",
            # Vulkan context 不再保留空初始化实现。
            ROOT / "src/native/presentation/graphics/vulkan/platform/context/graphics.rs",
            # Metal context 在构造阶段进入就绪态。
            ROOT / "src/native/presentation/graphics/metal/platform/context.rs",
            # WGL context 由 constructor 完成原生初始化。
            ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs",
            # EGL context 由 constructor 完成原生初始化。
            ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
        )
        # 公共 trait 不得重新声明二阶段初始化入口。
        self.assertNotIn("fn initialize(", graphics_trait)
        # owner-thread wrapper 不得重新转发已经删除的入口。
        self.assertNotIn('with_owner("initialize"', thread_bound)
        # registry 不得在构造成功后再次初始化 context。
        self.assertNotIn("ctx.initialize(", registry)
        # 所有直接 context 实现都不得复活兼容方法。
        for context in contexts:
            # 读取单个实现文件以排除 renderer 的独立初始化语义。
            source = context.read_text(encoding="utf-8")
            # trait wrapper 中不能出现同名方法。
            self.assertNotIn("fn initialize(", source)
        # 读取可观察构造状态的 fake context。
        fake = contexts[0].read_text(encoding="utf-8")
        # fake 构造必须直接标记为就绪。
        self.assertIn("initialized: true", fake)
        # 读取保留 checked shutdown 状态的 Metal context。
        metal = contexts[4].read_text(encoding="utf-8")
        # Metal 构造必须直接标记为就绪。
        self.assertIn("initialized: true", metal)
        # Metal 的失败诊断只描述构造后发生的 checked shutdown。
        self.assertIn("present after shutdown", metal)
        # 陈旧的二阶段初始化诊断不得复活。
        self.assertNotIn("present before initialize", metal)

    # 校验逐图元 legacy draw ABI 与平行 capability 表不会重新进入 adapter 门面。
    def test_legacy_draw_methods_leave_the_graphics_context_facade(self) -> None:
        # 读取公共兼容接口与事实型 capability profile。
        present = read_rust_module(ROOT / "src/native/present")
        # 读取 owner-thread 转发门面。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # 读取四个原生 context 的 trait 实现。
        adapters = (
            # D3D11 context 的 IGraphicsContext 实现。
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs",
            # D3D12 context 的 IGraphicsContext 实现。
            ROOT / "src/native/presentation/graphics/d3d12/platform/context/graphics.rs",
            # WGL context 的 IGraphicsContext 实现。
            ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs",
            # EGL context 的 IGraphicsContext 实现。
            ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
        )
        # 读取 adapter 私有 pipeline 目录，确认无消费者实现也已物理退出。
        private_pipelines = (
            # D3D11 只应保留薄 RHI packet 编码。
            ROOT / "src/native/presentation/graphics/d3d11/platform/pipeline",
            # D3D12 尚无薄 RHI，旧逐 UI pipeline 目录应不存在。
            ROOT / "src/native/presentation/graphics/d3d12/platform/pipeline",
            # OpenGL raster 只应保留 retained RHI owner 与 surface bridge。
            ROOT / "src/native/presentation/graphics/opengl/raster",
        )
        # 列出已经由固定 RHI probe 接管的全部逐图元入口。
        methods = (
            # 实心矩形批次。
            "draw_solid_rects",
            # 描边矩形批次。
            "draw_stroke_rects",
            # 字形批次。
            "draw_glyphs",
            # 线性渐变批次。
            "draw_linear_gradients",
            # 径向渐变批次。
            "draw_radial_gradients",
            # 扇形批次。
            "draw_sectors",
            # 实心网格批次。
            "draw_solid_meshes",
            # 阴影批次。
            "draw_box_shadows",
            # 图片批次。
            "draw_image_blits",
        )
        # 每个高层入口都必须同时退出 trait、线程门面和 adapter wrapper。
        for method in methods:
            # 公共 trait 不得重新声明逐图元方法。
            self.assertNotIn(f"fn {method}(", present)
            # owner-thread wrapper 不得重新生成转发入口。
            self.assertNotIn(f"forward_result!({method}", thread_bound)
            # 逐个读取 adapter trait 实现，避免私有低层 pipeline 干扰断言。
            for adapter in adapters:
                # 原生 context 不得重新包装该高层绘制方法。
                self.assertNotIn(f"fn {method}(", adapter.read_text(encoding="utf-8"))
            # adapter 私有 pipeline 也不得保留无消费者的同名高层实现。
            for pipeline in private_pipelines:
                # 目录不存在时 read_rust_module 返回空源码，仍符合物理删除契约。
                self.assertNotIn(f"fn {method}(", read_rust_module(pipeline))
        # 逐图元布尔字段不得重新形成第二份能力真相。
        for capability in (
            # 实心矩形能力。
            "solid_rects",
            # 描边矩形能力。
            "stroke_rects",
            # 字形能力。
            "glyphs",
            # 线性渐变能力。
            "linear_gradients",
            # 径向渐变能力。
            "radial_gradients",
            # 扇形能力。
            "sectors",
            # 实心网格能力。
            "solid_meshes",
            # 阴影能力。
            "box_shadows",
        ):
            # capability profile 不得重新导出对应布尔字段。
            self.assertNotIn(f"pub {capability}: bool", present)
        # capability profile 必须保留 retained surface 事实。
        self.assertIn("pub retained_framebuffer: bool", present)
        # capability profile 必须保留 Additive RHI 事实。
        self.assertIn("pub rhi_additive_blend: bool", present)

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
