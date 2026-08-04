//! retained surface 与 Picture texture 的 CPU soft segment 合成。

// 引入共享引用，避免把 soft tile 再复制成第二份可变数组。
use std::sync::Arc;

// 引入统一错误、最终 damage 和透明清理颜色。
use crate::core::{Error, PresentDamage};
// 引入 retained target 的 FramePlan 类型。
use crate::draw::backend::frame_plan::{
    FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef,
};
// 引入通用图片 quad 载荷和 RHI renderer。
use crate::draw::backend::rhi_renderer::{RhiRenderer, RhiTexturedQuad};
// 引入薄 RHI 的 load、target、context 和 viewport 类型。
use crate::native::present::rhi::{
    GraphicsContextRhi, LoadAction, RenderTargetHandle, RhiColor, RhiViewport,
};

// 引入 soft staging 的 owner canvas。
use super::super::canvas::NativeGpuCanvas2D;
// 引入 soft tile 打包器，保持 CPU staging 与 legacy 上传共用可见区域规则。
use super::super::tile::pack_visible_soft_fallback_tile;
// 引入当前 retained surface owner。
use super::GpuBackend;

// 复用 adapter soft blit 的逻辑到物理采样比例，拒绝异常 DPR。
fn supports_soft_upload(scale_x: f32, scale_y: f32) -> bool {
    // RHI textured quad 与 legacy soft tile 都以物理 viewport 进行过滤采样。
    scale_x.is_finite() && scale_y.is_finite() && scale_x > 0.0 && scale_y > 0.0
}

// 为尚无可见 soft 像素的新 retained target 执行透明初始化 pass。
pub(super) fn clear_empty_soft_target(
    context: &mut dyn GraphicsContextRhi,
    viewport: RhiViewport,
    target: RenderTargetHandle,
) -> Result<(), Error> {
    // 使用 Clear load action 使没有可见 tile 的首帧仍拥有确定像素。
    let mut pass = RenderPassPlan::new(
        RenderTargetRef::Texture(target),
        LoadAction::Clear(RhiColor([0.0, 0.0, 0.0, 0.0])),
    );
    // 非空 viewport 命令满足 FramePlan pass 契约并固定 adapter 状态。
    pass.push(FramePlanCommand::SetViewport(viewport));
    // 计划代际必须来自同一个 owner context。
    let mut plan = FramePlan::new(context.token(), PresentDamage::Full);
    // 追加只写 retained texture 的清理 pass。
    plan.push_pass(pass);
    // 只执行离屏 target，不获取或呈现 swapchain。
    plan.execute_offscreen_on_context(context)?;
    // 初始化提交句柄已由 owner context 接收，helper 只返回成功状态。
    Ok(())
}

// 把任意 GPU canvas 的 soft staging 合成到一个明确的 RHI texture target。
pub(super) fn try_upload_rhi_canvas_soft(
    canvas: &mut NativeGpuCanvas2D,
    logical_width: i32,
    logical_height: i32,
    viewport: RhiViewport,
    scale_x: f32,
    scale_y: f32,
    target: RenderTargetHandle,
    load: LoadAction,
    context: &mut dyn GraphicsContextRhi,
    renderer: &mut RhiRenderer,
) -> Result<bool, Error> {
    // 当前实现只接受有限正比例，避免把异常 DPR 转换为无效顶点。
    if !supports_soft_upload(scale_x, scale_y) {
        return Ok(false);
    }
    // 读取 soft canvas 的可见 tile，不扩大上传范围。
    let packed = canvas.soft_fallback.as_ref().and_then(|soft| {
        pack_visible_soft_fallback_tile(
            soft.surface().pixels(),
            logical_width,
            logical_height,
        )
    });
    // 没有可见像素时只在首个 Clear pass 执行透明初始化。
    let Some((pixels, tile)) = packed else {
        if matches!(load, LoadAction::Clear(_)) {
            clear_empty_soft_target(context, viewport, target)?;
        }
        // 没有可上传像素时不制造临时纹理资源。
        canvas.last_soft_upload_bytes = 0;
        return Ok(true);
    };
    // 把逻辑 tile 的左上角换算为 target 的物理坐标。
    let destination_x = tile.dst_x as f32 * scale_x;
    // 保存 soft tile 的物理顶部。
    let destination_y = tile.dst_y as f32 * scale_y;
    // 保存 soft tile 的物理宽度。
    let destination_w = tile.width as f32 * scale_x;
    // 保存 soft tile 的物理高度。
    let destination_h = tile.height as f32 * scale_y;
    // 上传区域必须完整落在当前 target viewport 内。
    if tile.dst_x < 0
        || tile.dst_y < 0
        || tile.width <= 0
        || tile.height <= 0
        || !destination_x.is_finite()
        || !destination_y.is_finite()
        || !destination_w.is_finite()
        || !destination_h.is_finite()
        || destination_x < 0.0
        || destination_y < 0.0
        || destination_w <= 0.0
        || destination_h <= 0.0
        || destination_x + destination_w > viewport.width
        || destination_y + destination_h > viewport.height
    {
        return Ok(false);
    }
    // 构造与 legacy soft blit 相同的左上原点 premultiplied quad。
    let quad = RhiTexturedQuad {
        // 目标左边界使用物理坐标。
        x: destination_x,
        // 目标上边界使用物理坐标。
        y: destination_y,
        // 目标宽度使用同一物理缩放。
        w: destination_w,
        // 目标高度使用同一物理缩放。
        h: destination_h,
        // soft tile 是轴对齐的 SrcOver 采样四边形。
        corners: crate::native::present::GpuGlyphBlit::axis_aligned_corners(
            destination_x,
            destination_y,
            destination_w,
            destination_h,
        ),
        // staging 已经是 premultiplied 颜色，不再额外 tint。
        rgba: [1.0; 4],
        // destination-dependent blend 不允许进入该 helper。
        additive: false,
        // 将紧密 tile 作为共享 RHI 上传 payload。
        pixels: Arc::from(pixels),
        // 记录 tile 的源宽度。
        pixel_w: tile.width as u32,
        // 记录 tile 的源高度。
        pixel_h: tile.height as u32,
        // 可见 tile 已经通过 packer 裁剪完成。
        scissor: None,
    };
    // 在同一个 owner-thread context 上创建临时纹理并采样到 target。
    let result = renderer.execute_textured_quads(
        context,
        PresentDamage::Full,
        viewport,
        load,
        RenderTargetRef::Texture(target),
        &[quad],
    );
    // 只有真实提交成功才更新诊断计数。
    if result.is_ok() {
        canvas.last_soft_upload_bytes = (tile.width as usize)
            .saturating_mul(tile.height as usize)
            .saturating_mul(std::mem::size_of::<u32>());
    }
    // 返回 adapter 的真实执行结果，失败时由 owner 保留 staging。
    result.map(|()| true)
}

// 为 GpuBackend 提供主 surface soft CPU segment 到 retained texture 的合成。
impl GpuBackend {
    // 把主 surface 的透明 soft staging 作为 SrcOver tile 写入 retained texture。
    pub(super) fn try_upload_rhi_surface_soft(
        &mut self,
        target: RenderTargetHandle,
        load: LoadAction,
    ) -> Result<bool, Error> {
        // 读取当前组合 context 的物理 viewport 与逻辑到物理比例。
        let (viewport, scale_x, scale_y) = {
            // 缺少组合 RHI 时不能让 soft staging 跳过 retained owner。
            let Some(context) = self.gpu_ctx.rhi_context() else {
                return Ok(false);
            };
            // 复用 native queue 使用的 surface geometry 证明。
            super::super::submit::rhi_physical_geometry(
                context,
                self.surface.width,
                self.surface.height,
            )
        };
        // 在 owner-thread 借用边界内复用通用 canvas soft 上传 helper。
        let (canvas, gpu_ctx, rhi_renderer) =
            (&mut self.surface.canvas, &mut self.gpu_ctx, &mut self.rhi_renderer);
        // retained soft 合成必须具备通用 renderer cache。
        let Some(renderer) = rhi_renderer.as_mut() else {
            return Ok(false);
        };
        // 当前 adapter 必须暴露组合 RHI context。
        let Some(context) = gpu_ctx.rhi_context() else {
            return Ok(false);
        };
        // 主 surface 按自身逻辑尺寸与物理 viewport 执行 SrcOver 上传。
        try_upload_rhi_canvas_soft(
            canvas,
            self.surface.width,
            self.surface.height,
            viewport,
            scale_x,
            scale_y,
            target,
            load,
            context,
            renderer,
        )
    }
}
