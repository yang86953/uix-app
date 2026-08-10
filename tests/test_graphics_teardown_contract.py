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
        self.assertIn("atomic GPU recipe probe passed", registry)
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
        # 读取 graphics backend 私有图片 payload、lowering 和混合顶点编码。
        primitives = (ROOT / "src/draw/backend/gpu/primitives.rs").read_text(encoding="utf-8")
        lowering = (ROOT / "src/draw/backend/gpu/rhi_lowering.rs").read_text(encoding="utf-8")
        mixed = (ROOT / "src/draw/backend/rhi_renderer_mixed.rs").read_text(encoding="utf-8")
        # 图片与 sampled Picture 必须承载任意仿射四角。
        self.assertIn("pub(crate) corners: [[f32; 2]; 4]", primitives)
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
        # 读取 graphics backend 私有渐变 payload、queue、lowering、renderer 和两套 adapter。
        primitives = (ROOT / "src/draw/backend/gpu/primitives.rs").read_text(encoding="utf-8")
        queue = (ROOT / "src/draw/backend/gpu/queue.rs").read_text(encoding="utf-8")
        lowering = (ROOT / "src/draw/backend/gpu/rhi_lowering.rs").read_text(encoding="utf-8")
        renderer = (ROOT / "src/draw/backend/rhi_renderer.rs").read_text(encoding="utf-8")
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs").read_text(encoding="utf-8")
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        shaders = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs").read_text(encoding="utf-8")
        # 两类 native gradient DTO 和通用 payload 必须带真实四角。
        self.assertIn("pub(crate) corners: [[f32; 2]; 4]", primitives)
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
        # 生产 backend 构造必须以能力快照存在性拒绝缺少组合 thin RHI 的 context。
        self.assertIn(".is_some_and(|capabilities| capabilities.has_gpu_baseline())", lifecycle)
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

    # 校验平台 current 语义只存在于 adapter 私有 RHI host，不再穿透通用 renderer。
    def test_make_current_compatibility_entry_leaves_renderer_and_graphics_context(self) -> None:
        # 读取公共图形上下文 trait。
        graphics_trait = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # 读取 owner-thread context wrapper。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # 读取通用 backend 契约。
        backend_contract = (ROOT / "src/draw/backend/contract.rs").read_text(encoding="utf-8")
        # 读取 RenderSession 的帧入口。
        session = (ROOT / "src/draw/renderer/session.rs").read_text(encoding="utf-8")
        # 读取 Renderer 的 presentation 路由。
        runtime = (ROOT / "src/draw/renderer/runtime.rs").read_text(encoding="utf-8")
        # 读取 GPU backend 的 RHI 准备 helper。
        gpu_lifecycle = (ROOT / "src/draw/backend/gpu/backend/impl_main.rs").read_text(encoding="utf-8")
        # 读取 GPU RenderBackend 实现。
        gpu_backend = (ROOT / "src/draw/backend/gpu/backend/render_backend.rs").read_text(encoding="utf-8")
        # 读取 overlay 无帧 RHI 事务。
        overlay = (ROOT / "src/draw/backend/gpu/backend/render_backend_backdrop.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 的 thin RHI host。
        opengl_host = (ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs").read_text(encoding="utf-8")
        # 收集所有直接实现 IGraphicsContext 的测试与原生 context。
        contexts = (
            # fake context 不得保留调用计数旁路。
            ROOT / "src/native/test_harness/fake_graphics_context.rs",
            # D3D11 context 的 target 绑定只属于低层 RHI helper。
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs",
            # D3D12 context 的 command list 开始只属于 adapter 内部。
            ROOT / "src/native/presentation/graphics/d3d12/platform/context/graphics.rs",
            # Vulkan context 不再提供空 current 实现。
            ROOT / "src/native/presentation/graphics/vulkan/platform/context/graphics.rs",
            # Metal context 不再提供空 current 实现。
            ROOT / "src/native/presentation/graphics/metal/platform/context.rs",
            # WGL trait wrapper 不再暴露 current。
            ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs",
            # EGL trait wrapper 不再暴露 current。
            ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
        )
        # IGraphicsContext 不得重新声明平台 current 方法。
        self.assertNotIn("fn make_current(", graphics_trait)
        # thread-bound wrapper 不得重新转发平台 current 方法。
        self.assertNotIn("forward_result!(make_current", thread_bound)
        # RenderBackend 不得重新泄露平台 current 方法。
        self.assertNotIn("fn make_current(", backend_contract)
        # 通用 backend 只保留语义型帧准备 hook。
        self.assertIn("fn prepare_frame(&mut self)", backend_contract)
        # RenderSession 必须在帧开始前调用统一准备 hook。
        self.assertIn("self.backend.prepare_frame()", session)
        # Renderer 不得再按 presentation 类型直接切换 current。
        self.assertNotIn("make_backend_current", runtime)
        # Renderer 不得再绕过 session 调用 backend current。
        self.assertNotIn("backend_mut().make_current", runtime)
        # GPU helper 必须进入 thin RHI device maintenance。
        self.assertIn("GraphicsDevice::maintain(context)", gpu_lifecycle)
        # GPU backend 的 prepare_frame 必须复用单一 helper。
        self.assertIn("self.prepare_rhi_device()", gpu_backend)
        # overlay 不得再直接调用兼容 context current。
        self.assertNotIn("gpu_ctx.make_current", overlay)
        # snapshot 与 restore 都必须复用 thin RHI 设备准备。
        # snapshot、blur 与 restore 各自必须通过唯一 device maintenance 入口。
        self.assertEqual(overlay.count("self.prepare_rhi_device()"), 3)
        # 定位 OpenGL 设备维护实现。
        maintain = opengl_host.index("fn maintain(&mut self)")
        # current 必须先于设备健康检查执行。
        current = opengl_host.index("self.rhi_make_current()?", maintain)
        # 设备健康检查必须位于同一维护方法内。
        health = opengl_host.index("self.rhi_pipeline_mut().rhi_maintain()", current)
        # 明确锁定 owner context 准备顺序。
        self.assertLess(current, health)
        # 所有 IGraphicsContext wrapper 均不得复活 current 方法。
        for context in contexts:
            # 读取单个 wrapper 文件，避免 adapter 私有 helper 造成误判。
            source = context.read_text(encoding="utf-8")
            # trait 实现中不能出现同名平台入口。
            self.assertNotIn("fn make_current(", source)
        # fake context 不得继续记录已删除的调用事实。
        self.assertNotIn("make_current_calls", contexts[0].read_text(encoding="utf-8"))
        # WGL 原生 helper 必须保留给 OpenGlRhiHost 使用。
        wgl_native = (ROOT / "src/native/presentation/graphics/opengl/platform/wgl.rs").read_text(encoding="utf-8")
        # helper 不进入公共 trait，只在 adapter 内维护。
        self.assertIn("fn make_current_result(&self)", wgl_native)
        # EGL 同样保留 adapter 私有 helper。
        self.assertIn("fn make_current_result(&self)", contexts[6].read_text(encoding="utf-8"))

    # 校验生产 GPU backend 不再保留 hybrid 构造和 native resize 兼容回退。
    def test_gpu_resize_only_uses_factory_prepared_thin_rhi_surface(self) -> None:
        # 读取 GPU backend 的状态字段。
        backend_state = (ROOT / "src/draw/backend/gpu/backend/mod.rs").read_text(encoding="utf-8")
        # 读取 GPU backend 构造与资源生命周期。
        lifecycle = (ROOT / "src/draw/backend/gpu/backend/impl_main.rs").read_text(encoding="utf-8")
        # 读取 GPU canvas 的生产与测试构造边界。
        canvas = (ROOT / "src/draw/backend/gpu/canvas.rs").read_text(encoding="utf-8")
        # 读取 RenderBackend 的 resize、initialize 与 Picture 实现。
        backend = (ROOT / "src/draw/backend/gpu/backend/render_backend.rs").read_text(encoding="utf-8")
        # 读取构造期已验证的 GPU recipe owner。
        gpu_owner = (ROOT / "src/native/present/gpu_recipe_owner.rs").read_text(encoding="utf-8")
        # dormant hybrid 构造入口不得复活。
        self.assertNotIn("pub(crate) fn new(gpu_ctx", lifecycle)
        # 构造器不得重新按模式分叉。
        self.assertNotIn("fn new_with_mode", lifecycle)
        # 生产 backend 不再保存 hybrid/GPU-only 运行时开关。
        self.assertNotIn("gpu_only: bool", backend_state)
        # factory-prepared 是构造不变量，不再保存第二份状态。
        self.assertNotIn("factory_prepared", backend_state + lifecycle + backend)
        # 构造门禁只接受 GPU-only retained RHI baseline。
        self.assertIn("native_caps.has_gpu_only_baseline()", lifecycle)
        # hybrid baseline 不得重新进入生产构造分支。
        self.assertNotIn("native_caps.has_hybrid_baseline()", lifecycle)
        # 主 surface 固定创建 GPU-only canvas。
        self.assertIn("NativeGpuCanvas2D::new_gpu_only(logical_w, logical_h, native_caps)", lifecycle)
        # hybrid canvas 构造只能作为单测 fixture 编译。
        self.assertIn("#[cfg(test)]\n    // 创建允许测试显式进入 hybrid 分支的 canvas。\n    pub(crate) fn new", canvas)
        # 定位显式 resize 实现。
        resize_start = backend.index("fn resize(&mut self, width: i32, height: i32)")
        # 定位 initialize 边界以截取完整 resize 方法。
        initialize_start = backend.index("fn initialize_prepared", resize_start)
        # 提取 resize 方法，避免其它生命周期代码干扰断言。
        resize = backend[resize_start:initialize_start]
        # resize 必须通过已验证 GPU owner 的专用 surface 事务。
        self.assertIn("self.gpu_ctx.resize_surface(logical_w, logical_h)?", resize)
        # backend 不得重新从兼容 context 借用可选生命周期视图。
        self.assertNotIn("self.gpu_ctx.gpu_recipe_context()", resize)
        # 兼容 IGraphicsContext::resize 不得作为 NotImplemented 回退。
        self.assertNotIn("self.gpu_ctx.resize(", resize)
        # 构造后视图缺失必须进入状态恢复，而不是能力回退。
        self.assertNotIn("Errc::NotImplemented", resize)
        # 缺失专用视图必须由 owner 使用稳定的状态错误分类。
        self.assertIn("Errc::InvalidState", gpu_owner)
        # 旧代 retained 资源必须先于 surface generation 推进释放。
        self.assertLess(
            # 定位旧代 retained 资源销毁。
            resize.index("self.destroy_rhi_surface_texture()?"),
            # 定位已验证 owner 的 thin RHI resize 调用。
            resize.index("self.gpu_ctx.resize_surface(logical_w, logical_h)?"),
        )
        # 定位 initialize 方法的结束边界。
        shutdown_start = backend.index("fn try_shutdown", initialize_start)
        # 提取 initialize 方法以锁定无二次 native resize。
        initialize = backend[initialize_start:shutdown_start]
        # factory-prepared 初始化不得触碰任何 resize 入口。
        self.assertNotIn(".resize(", initialize)
        # Picture canvas 同样固定采用 GPU-only 语义。
        self.assertIn("canvas: NativeGpuCanvas2D::new_gpu_only", backend)
        # Picture 创建不得保留 runtime hybrid 分叉。
        self.assertNotIn("if self.gpu_only", backend)

    # 校验逐图元 legacy draw ABI 与平行 capability 表不会重新进入 adapter 门面。
    def test_legacy_draw_methods_leave_the_graphics_context_facade(self) -> None:
        # 读取公共兼容接口与事实型 capability profile。
        present = read_rust_module(ROOT / "src/native/present")
        # 读取 graphics backend 私有的 renderer 能力投影。
        raster_caps = (ROOT / "src/draw/backend/gpu/capabilities.rs").read_text(encoding="utf-8")
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
        self.assertIn("retained_framebuffer: bool", raster_caps)
        # capability profile 必须保留 Additive RHI 事实。
        self.assertIn("rhi_additive_blend: bool", raster_caps)
        # platform presentation 不得重新取得 renderer 能力投影所有权。
        self.assertNotIn("NativeRasterCaps", present)

    # 校验 renderer 能力只从 thin RHI 快照派生，不恢复 adapter 平行声明。
    def test_native_raster_caps_are_derived_from_thin_rhi(self) -> None:
        # 读取迁移期 graphics context trait。
        facade = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # 读取 owner-thread 包装层。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # 读取 graphics backend 拥有的 renderer 能力投影定义。
        raster_caps = (ROOT / "src/draw/backend/gpu/capabilities.rs").read_text(encoding="utf-8")
        # 读取生产 GPU backend 构造门禁。
        backend = (ROOT / "src/draw/backend/gpu/backend/impl_main.rs").read_text(encoding="utf-8")
        # 枚举曾经硬编码 renderer profile 的生产 adapter。
        adapters = (
            # Windows 默认 D3D11 adapter。
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs",
            # Windows OpenGL ES/WGL adapter。
            ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs",
            # Linux OpenGL ES/EGL adapter。
            ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
        )
        # 公共 context 门面不得重新声明 renderer 能力查询。
        self.assertNotIn("fn native_raster_caps(", facade)
        # 线程包装层不得缓存或转发第二份能力真相。
        self.assertNotIn("native_raster_caps", thread_bound)
        # 所有生产 adapter 都必须退出硬编码 profile。
        for adapter in adapters:
            # adapter wrapper 不得重新声明 renderer 能力查询。
            self.assertNotIn("native_raster_caps", adapter.read_text(encoding="utf-8"))
        # renderer 投影必须显式接收薄 RHI capability 快照。
        self.assertIn("from_rhi_capabilities(capabilities: GraphicsCapabilities)", raster_caps)
        # retained 事实必须直接复制自同一薄 RHI 快照。
        self.assertIn("retained_framebuffer: capabilities.retained_framebuffer", raster_caps)
        # Additive 事实必须直接复制自同一薄 RHI 快照。
        self.assertIn("rhi_additive_blend: capabilities.additive_blend", raster_caps)
        # 构造门禁必须从已验证组合 RHI 读取唯一能力来源。
        self.assertIn("Some(gpu_ctx.rhi_context()?.capabilities())", backend)
        # 构造门禁必须验证完整 GPU 原语基线。
        self.assertIn("capabilities.has_gpu_baseline()", backend)
        # 构造门禁必须从同一快照派生 renderer 投影。
        self.assertIn(".map(NativeRasterCaps::from_rhi_capabilities)", backend)

    # 校验 surface readback 已成为事实声明的可选 thin RHI 能力。
    def test_surface_readback_is_an_optional_thin_rhi_capability(self) -> None:
        # 读取兼容 graphics context trait。
        facade = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # 读取 owner-thread 兼容转发门面。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # 读取 thin RHI 契约。
        rhi = (ROOT / "src/native/present/rhi.rs").read_text(encoding="utf-8")
        # 读取 D3D11 surface 与 capability 实现。
        d3d11_surface = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi.rs").read_text(encoding="utf-8")
        # 读取 D3D11 device capability 实现。
        d3d11_device = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs").read_text(encoding="utf-8")
        # 读取共享 OpenGL surface bridge。
        opengl_surface = (ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs").read_text(encoding="utf-8")
        # 读取共享 OpenGL capability profile。
        opengl_rhi = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi.rs").read_text(encoding="utf-8")
        # 读取 GPU backend 的测试诊断入口。
        backend = (ROOT / "src/draw/backend/gpu/backend/impl_main.rs").read_text(encoding="utf-8")
        # 读取 Vulkan 的显式 GFX-R5 诊断辅助。
        vulkan = (ROOT / "src/native/presentation/graphics/vulkan/platform/context/graphics.rs").read_text(encoding="utf-8")
        # 读取 GFX-R5 证据调用点。
        gfx_r5 = (ROOT / "src/gfx_r5_support/evidence.rs").read_text(encoding="utf-8")
        # 兼容 trait 不得重新声明 readback。
        self.assertNotIn("fn read_pixels(", facade)
        # owner-thread 兼容 wrapper 不得转发 readback。
        self.assertNotIn("fn read_pixels(", thread_bound)
        # 逐个核对原生 context wrapper 已退出旧入口。
        for adapter in (
            # D3D11 兼容实现。
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs",
            # D3D12 兼容实现。
            ROOT / "src/native/presentation/graphics/d3d12/platform/context/graphics.rs",
            # Vulkan 兼容实现。
            ROOT / "src/native/presentation/graphics/vulkan/platform/context/graphics.rs",
            # Metal 兼容实现。
            ROOT / "src/native/presentation/graphics/metal/platform/context.rs",
            # WGL 兼容实现。
            ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs",
            # EGL 兼容实现。
            ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
            # 测试 context 兼容实现。
            ROOT / "src/native/test_harness/fake_graphics_context.rs",
        ):
            # 兼容实现不得私自复活旧 trait 方法。
            self.assertNotIn("fn read_pixels(", adapter.read_text(encoding="utf-8"))
        # capability 必须显式陈述可选 surface 回读事实。
        self.assertIn("pub(crate) surface_readback: bool", rhi)
        # 通用 GPU 基线默认不能宣称可回读。
        self.assertIn("surface_readback: false", rhi)
        # surface trait 必须提供 typed 可选操作。
        self.assertIn("fn read_surface_pixels(", rhi)
        # D3D11 必须如实启用 capability。
        self.assertIn("capabilities.surface_readback = true;", d3d11_device)
        # OpenGL 必须如实启用 capability。
        self.assertIn("capabilities.surface_readback = true;", opengl_rhi)
        # D3D11 surface 必须委托私有 staging 实现。
        self.assertIn("self.read_surface_pixels_result(x, y, width, height)", d3d11_surface)
        # 截取 OpenGL surface 回读方法。
        opengl_readback = opengl_surface[opengl_surface.index("fn read_surface_pixels(") : opengl_surface.index("fn present(")]
        # OpenGL 回读必须先恢复 current context。
        self.assertLess(opengl_readback.index("self.rhi_make_current()?"), opengl_readback.index("self.rhi_pipeline_mut().read_pixels"))
        # GPU backend 不得回退兼容 context readback。
        self.assertNotIn("self.gpu_ctx.read_pixels", backend)
        # GPU backend 必须检查 capability 事实。
        self.assertIn("context.capabilities().surface_readback", backend)
        # GPU backend 必须通过 thin RHI surface 执行回读。
        self.assertIn("context.read_surface_pixels(0, 0, width, height)", backend)
        # Vulkan pixel-upload readback 必须保留显式诊断名称。
        self.assertIn("pub(crate) fn readback_pixels(", vulkan)
        # GFX-R5 证据必须调用显式诊断辅助。
        self.assertIn(".readback_pixels(0, 0, 1, 1)", gfx_r5)
        # GFX-R5 不得继续依赖兼容 trait 方法。
        self.assertNotIn(".read_pixels(", gfx_r5)

    # 校验通用 context resize 已拆分为 GPU RHI 与 CPU PixelUpload 两条 typed 契约。
    def test_context_resize_is_split_by_surface_recipe(self) -> None:
        # 读取兼容 context 与 PixelUpload surface trait。
        facade = (ROOT / "src/native/present/traits.rs").read_text(encoding="utf-8")
        # 读取 owner-thread wrapper。
        thread_bound = (ROOT / "src/native/factory/thread_bound.rs").read_text(encoding="utf-8")
        # 读取统一 renderer 的 PixelUpload 生命周期。
        runtime = (ROOT / "src/draw/renderer/runtime.rs").read_text(encoding="utf-8")
        # 读取离开 native factory 前的正交 recipe owner。
        recipe_owner = (ROOT / "src/native/present/recipe_owner.rs").read_text(encoding="utf-8")
        # 读取 Vulkan GFX-R5 显式诊断调用点。
        gfx_r5 = (ROOT / "src/gfx_r5_support/evidence.rs").read_text(encoding="utf-8")
        # 读取 mixed-DPI GFX-R5 场景，避免 feature 隔离代码逃逸契约。
        gfx_r5_mod = (ROOT / "src/gfx_r5_support/mod.rs").read_text(encoding="utf-8")
        # 读取 D3D11 context 私有生命周期实现。
        d3d11_methods = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/methods.rs").read_text(encoding="utf-8")
        # 通用 context trait 不得继续声明无 recipe 区分的 resize。
        self.assertNotIn("fn resize(&mut self", facade)
        # PixelUpload 必须拥有独立的专用 surface trait。
        self.assertIn("pub(crate) trait PixelUploadSurface", facade)
        # 专用 trait 必须使用显式 recipe 名称。
        self.assertIn("fn resize_pixel_upload_surface(", facade)
        # context 只借出可选 PixelUpload surface 视图。
        self.assertIn("fn pixel_upload_surface(&mut self)", facade)
        # thread-bound 不得恢复通用 resize wrapper。
        self.assertNotIn("fn resize(&mut self", thread_bound)
        # thread-bound 必须实现专用契约。
        self.assertIn("impl PixelUploadSurface for ThreadBoundGraphicsContext", thread_bound)
        # 专用 wrapper 必须保持 owner-thread 检查。
        self.assertIn('self.with_owner("resize_pixel_upload_surface"', thread_bound)
        # 专用 wrapper 成功后必须刷新 drawable 元数据。
        self.assertIn("self.refresh_metadata();", thread_bound)
        # 逐个核对 GPU context 与测试 fake 已退出兼容 resize wrapper。
        for adapter in (
            # D3D11 GPU context。
            ROOT / "src/native/presentation/graphics/d3d11/platform/context/graphics.rs",
            # D3D12 测试期 GPU context。
            ROOT / "src/native/presentation/graphics/d3d12/platform/context/graphics.rs",
            # WGL GPU context。
            ROOT / "src/native/presentation/graphics/opengl/platform/wgl_graphics.rs",
            # EGL GPU context。
            ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs",
            # GPU-native test fake。
            ROOT / "src/native/test_harness/fake_graphics_context.rs",
        ):
            # adapter 的 IGraphicsContext 实现不得重新包装通用 resize。
            self.assertNotIn("fn resize(&mut self", adapter.read_text(encoding="utf-8"))
        # D3D11 已无消费者的逻辑兼容 helper 必须物理删除。
        self.assertNotIn("fn resize_surface_logical(", d3d11_methods)
        # 读取 Vulkan PixelUpload context 实现。
        vulkan = (ROOT / "src/native/presentation/graphics/vulkan/platform/context/graphics.rs").read_text(encoding="utf-8")
        # 读取 Metal PixelUpload context 实现。
        metal = (ROOT / "src/native/presentation/graphics/metal/platform/context.rs").read_text(encoding="utf-8")
        # 两个 PixelUpload adapter 必须实现专用 surface trait。
        self.assertIn("impl PixelUploadSurface for VulkanContext", vulkan)
        # Metal 同样必须实现专用 surface trait。
        self.assertIn("impl PixelUploadSurface for MetalPixelUploadContext", metal)
        # 两个 adapter 都必须显式暴露专用 surface 视图。
        self.assertIn("fn pixel_upload_surface(&mut self)", vulkan)
        # Metal 不能依赖 runtime 猜测 recipe。
        self.assertIn("fn pixel_upload_surface(&mut self)", metal)
        # native recipe 出口必须先验证 PixelUpload owner。
        self.assertIn(
            "PixelUploadRecipeOwner::try_new(context).map(Self::PixelUpload)",
            recipe_owner,
        )
        # presentation 只能接收已经通过门禁的 owner。
        self.assertIn("PixelUploadPresentation::new(owner)", runtime)
        # runtime resize 必须通过专用 presentation helper。
        self.assertIn("upload.resize_surface(width, height)?", runtime)
        # runtime 不得调用已经删除的通用 context resize。
        self.assertNotIn("upload.context", runtime)
        # Vulkan GFX-R5 必须显式使用 PixelUpload surface 契约。
        self.assertGreaterEqual(gfx_r5.count(".resize_pixel_upload_surface("), 3)
        # GFX-R5 不得再通过 IGraphicsContext resize 驱动 Vulkan。
        self.assertNotIn("context.resize(", gfx_r5)
        # mixed-DPI 两段转换也必须显式使用 PixelUpload surface 契约。
        self.assertGreaterEqual(gfx_r5_mod.count(".resize_pixel_upload_surface("), 2)
        # feature 隔离场景不得保留无法编译的旧 resize 调用。
        self.assertNotIn(".resize(LOGICAL_EXTENT", gfx_r5_mod)

if __name__ == "__main__":
    unittest.main()
