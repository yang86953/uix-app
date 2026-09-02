//! Picture 离屏缓存栅格化与合成（Phase 4）。

use crate::core::{Errc, Error, Point, Rect};

use super::layer_tree::{LayerNode, LayerTree};
use crate::core::DirtyRegion;
use crate::draw::FontHandle;
use crate::draw::geometry::spatial::Orientation;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::painting::{DisplayList, FrameEncoder, recorder::CommandRecorder};
use crate::draw::painting::{PaintContext, PaintSurfaceConfig};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::image::ImageService;
use crate::draw::scene::NodeId;
use crate::draw::scene::ScenePaint;
use crate::draw::scene::viewport_transform::needs_paint;
use crate::draw::target::RenderTarget;

/// 离屏创建连续失败上限（超过后放弃离屏、改走直绘；仅 WARN 一次）。
const MAX_OFFSCREEN_RETRY: u8 = 8;

/// 图层渲染环境（绘制资源，供 Picture 离屏路径复用）。
pub(crate) struct LayerRenderEnv<'a> {
    pub font: FontHandle,
    pub font_service: &'a FontService,
    pub image_service: &'a ImageService,
    pub dpi: f32,
    pub dpr: f32,
    pub orientation: Orientation,
}

/// 将 Picture 缓存 blit 到主缓冲。
pub(crate) fn blit_picture_cache(
    engine: &mut dyn RenderTarget,
    handle: &ImageHandle,
    bounds: &Rect,
    w: i32,
    h: i32,
) -> Result<(), Error> {
    let src = Rect::new(0.0, 0.0, w as f32, h as f32);

    engine.try_blit_offscreen_src(handle, src, *bounds)
}

/// 对已栅格化的 Picture 离屏做可分离高斯模糊（GPU RT 或 CPU 像素）。
///
/// 供 Modal / Drawer 等毛玻璃消费方在 blit 前调用；半径语义同
/// `crate::draw::raster::rasterizer::blur::gaussian_blur`（私有实现，纯文本引用）。
pub fn blur_picture_region(
    engine: &mut dyn RenderTarget,
    handle: &ImageHandle,
    region: Rect,
    radius: f32,
) -> Result<(), Error> {
    engine.try_blur_offscreen(handle, region, radius)
}

/// 将脏 Picture 栅格化到离屏缓冲。
pub(crate) fn rasterize_picture_to_offscreen<S: ScenePaint>(
    engine: &mut dyn RenderTarget,
    node_id: NodeId,
    bounds: &Rect,
    offscreen_handle: &mut Option<ImageHandle>,
    display_list: &mut Option<DisplayList>,
    children: &mut [LayerNode],
    is_dirty: &mut bool,
    scene: &S,
    paint_region: &DirtyRegion,
    w: i32,
    h: i32,
    retry_count: &mut u8,
    env: &LayerRenderEnv<'_>,
) -> Result<(), Error> {
    // 嵌套 Picture 仍按屏幕脏区决定是否重栅格化；一旦进入栅格化则内部全量重绘。
    prepare_nested_pictures(engine, children, scene, paint_region, env)?;

    // 已放弃：不再重试、不再刷 WARN（rebuild 会重置 retry_count）。
    if *retry_count >= MAX_OFFSCREEN_RETRY {
        return Ok(());
    }
    if !ensure_offscreen(engine, offscreen_handle, w, h)? {
        *retry_count = retry_count.saturating_add(1);
        if *retry_count == MAX_OFFSCREEN_RETRY {
            tracing::warn!(
                "Picture 离屏创建失败已达 {MAX_OFFSCREEN_RETRY} 次，改走直绘，node_id={node_id}"
            );
        }
        return Ok(());
    }
    *retry_count = 0;
    let handle = match offscreen_handle {
        Some(h) => *h,
        None => return Ok(()),
    };

    engine.try_begin_offscreen_paint(&handle)?;

    let nested_blits = collect_picture_blit_info(children, bounds);

    let self_dirty = scene.node_dirty(node_id);
    let mut fresh_list = if self_dirty || display_list.is_none() {
        Some(DisplayList::new())
    } else {
        None
    };
    let mut fresh_list_complete = true;

    let raster_result = (|| -> Result<(), Error> {
        // Cached and fresh paints use the same recorder. Keep cached execution
        // inside the checked paint lifetime so every error still reaches the
        // end/abort boundary below and cannot publish a partial Picture.
        let encoded_cached_picture = if fresh_list.is_none() {
            if let Some(cached) = display_list.as_ref() {
                match encode_cached_picture(
                    cached,
                    w,
                    h,
                    Point::new(bounds.x, bounds.y),
                    env.font,
                    env.font_service,
                    env.image_service,
                ) {
                    Ok(encoder) => matches!(
                        engine.try_execute_encoded_picture(&handle, &encoder)?,
                        crate::draw::painting::EncodedPictureExecution::Executed
                    ),
                    // 编码器构造失败只损失缓存资格（回退直绘），保留一次
                    // debug 级细节供性能问题定位。
                    Err(error) => {
                        tracing::debug!(
                            "picture encoder construction failed; caching skipped: {}",
                            error.short_what()
                        );
                        false
                    }
                }
            } else {
                false
            }
        } else {
            false
        };

        let Some(off_canvas) = engine.offscreen_canvas(&handle) else {
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen target disappeared after bind",
            ));
        };
        // 离屏缓冲已由 checked begin 以 replace 语义清透明；必须完整重绘子树，不能沿用屏幕 dirty_region 剪枝，
        // 否则悬停窄标脏时未相交的兄弟节点（侧栏其它项）会永久消失。
        let full_offscreen = DirtyRegion::full();
        let mut off_ctx = PaintContext::new(
            off_canvas,
            env.font,
            env.font_service,
            env.image_service,
            PaintSurfaceConfig {
                dpi: env.dpi,
                device_pixel_ratio: env.dpr,
                orientation: env.orientation,
                surface_w: w,
                surface_h: h,
            },
        );
        off_ctx.translate(-bounds.x, -bounds.y);
        if !encoded_cached_picture {
            if let Some(list) = fresh_list.as_mut() {
                off_ctx.with_recorder(list, |ctx| {
                    LayerTree::render_widget_self(node_id, ctx, scene);
                });
                fresh_list_complete = off_ctx.recording_complete();
            } else if let Some(cached) = display_list.as_ref() {
                cached.replay(&mut off_ctx);
            }
        }
        if scene.node_visible(node_id) {
            // 屏幕 dirty 仅用于主表面剪枝；离屏已全清，子树必须完整重绘
            render_non_picture_subtree(children, &mut off_ctx, scene, &full_offscreen);
        }
        // 防御性保留显式二阶段契约；正常选择器会拒绝这类 Picture 根。
        if scene.node_paints_after_children(node_id) {
            // 在离屏子树完成后绘制覆盖装饰。
            LayerTree::render_widget_after_children(node_id, &mut off_ctx, scene);
        }
        Ok(())
    })();
    let raster_result = raster_result.and_then(|()| {
        engine.try_flush_offscreen_paint(&handle)?;

        for (child_bounds, child_handle) in nested_blits {
            let local = Rect::new(
                child_bounds.x - bounds.x,
                child_bounds.y - bounds.y,
                child_bounds.w,
                child_bounds.h,
            );
            let src = Rect::new(0.0, 0.0, child_bounds.w, child_bounds.h);
            engine.try_blit_offscreen_src(&child_handle, src, local)?;
        }
        Ok(())
    });
    let restore_result = engine.try_end_offscreen_paint();
    // Target restoration is still attempted above, but the original recording
    // failure is the frame result and leaves this Picture dirty.
    raster_result?;
    restore_result?;

    if let Some(list) = fresh_list {
        *display_list = fresh_list_complete.then_some(list);
    }

    *is_dirty = false;
    Ok(())
}

pub(crate) fn ensure_offscreen(
    engine: &mut dyn RenderTarget,
    handle: &mut Option<ImageHandle>,
    w: i32,
    h: i32,
) -> Result<bool, Error> {
    if w <= 0 || h <= 0 {
        return Ok(false);
    }
    if let Some(hdl) = *handle {
        if let Some(canvas) = engine.offscreen_canvas(&hdl) {
            if canvas.width() == w && canvas.height() == h {
                return Ok(true);
            }
        }
        // 新资源的正常不支持仍允许保持无缓存路径；typed failure 直接传播。
        let Some(replacement) = engine.try_create_offscreen(w, h)? else {
            return Ok(false);
        };
        if let Err(error) = engine.try_destroy_offscreen(hdl) {
            // 旧资源释放失败时补偿释放尚未发布的 replacement。
            if let Err(cleanup_error) = engine.try_destroy_offscreen(replacement) {
                // 双重释放失败保留完整原因链，backend 仍持有两份资源供 shutdown 重试。
                return Err(error.with_appended_source(cleanup_error));
            }
            // 补偿成功后仍返回原始旧资源释放失败。
            return Err(error);
        }
        *handle = Some(replacement);
        return Ok(true);
    }
    // 首次创建同样区分正常无资源与 typed allocation/device failure。
    *handle = engine.try_create_offscreen(w, h)?;
    Ok(handle.is_some())
}

/// Turns a complete cached DisplayList into the same API-neutral command stream
/// used by a fresh Picture paint. Eligible glyph/solid operations retain their
/// shared IR; unsupported states become bounded CPU segments inside the stream.
pub(crate) fn encode_cached_picture(
    list: &DisplayList,
    width: i32,
    height: i32,
    origin: Point,
    font: FontHandle,
    font_service: &FontService,
    image_service: &ImageService,
) -> Result<FrameEncoder, Error> {
    let mut recorder = CommandRecorder::new();
    recorder.initialize(width, height)?;
    recorder.begin_recording(true)?;
    {
        let canvas = recorder.canvas_2d();
        canvas.translate(-origin.x, -origin.y);
        list.replay_canvas(
            canvas,
            font,
            font_service,
            Some(image_service),
            width as f32,
        );
    }
    recorder.finish_recording()
}

fn prepare_nested_pictures<S: ScenePaint>(
    engine: &mut dyn RenderTarget,
    children: &mut [LayerNode],
    scene: &S,
    paint_region: &DirtyRegion,
    env: &LayerRenderEnv<'_>,
) -> Result<(), Error> {
    for child in children.iter_mut() {
        if let LayerNode::Picture {
            node_id,
            bounds,
            is_dirty,
            offscreen_handle,
            display_list,
            children: sub,
            retry_count,
            ..
        } = child
        {
            let w = bounds.w.ceil() as i32;
            let h = bounds.h.ceil() as i32;
            if *is_dirty || offscreen_handle.is_none() {
                rasterize_picture_to_offscreen(
                    engine,
                    *node_id,
                    bounds,
                    offscreen_handle,
                    display_list,
                    sub,
                    is_dirty,
                    scene,
                    paint_region,
                    w,
                    h,
                    retry_count,
                    env,
                )?;
            }
        }
        let (LayerNode::Picture { children: sub, .. }
        | LayerNode::ClipRect { children: sub, .. }
        | LayerNode::Direct { children: sub, .. }) = child;
        {
            prepare_nested_pictures(engine, sub, scene, paint_region, env)?;
        }
    }
    Ok(())
}

fn collect_picture_blit_info(
    children: &[LayerNode],
    parent_bounds: &Rect,
) -> Vec<(Rect, ImageHandle)> {
    let mut out = Vec::new();
    collect_picture_blit_info_rec(children, parent_bounds, &mut out);
    out
}

fn collect_picture_blit_info_rec(
    children: &[LayerNode],
    parent_bounds: &Rect,
    out: &mut Vec<(Rect, ImageHandle)>,
) {
    for child in children {
        match child {
            LayerNode::Picture {
                bounds,
                offscreen_handle: Some(handle),
                children: sub,
                ..
            } => {
                if parent_bounds.intersect(bounds).is_some() {
                    out.push((*bounds, *handle));
                }
                collect_picture_blit_info_rec(sub, parent_bounds, out);
            }
            LayerNode::Picture {
                offscreen_handle: None,
                ..
            } => {}
            LayerNode::ClipRect { children: sub, .. } | LayerNode::Direct { children: sub, .. } => {
                collect_picture_blit_info_rec(sub, parent_bounds, out);
            }
        }
    }
}

fn render_non_picture_subtree<S: ScenePaint>(
    children: &mut [LayerNode],
    ctx: &mut PaintContext<'_>,
    scene: &S,
    dirty_region: &DirtyRegion,
) {
    for child in children.iter_mut() {
        match child {
            LayerNode::Picture { .. } => {}
            LayerNode::ClipRect {
                node_id,
                rect,
                children: sub,
                ..
            } => {
                // ClipRect 自身的普通内容不依赖二阶段覆盖 capability。
                if needs_paint(scene, *node_id, dirty_region) {
                    // 先绘制父节点 Content，再进入裁剪后的子树。
                    LayerTree::render_widget_self(*node_id, ctx, scene);
                }
                ctx.push_clip(*rect);
                if let Some((sx, sy)) = LayerTree::get_scroll_offset(scene, *node_id) {
                    ctx.translate(-sx, -sy);
                }
                render_non_picture_subtree(sub, ctx, scene, dirty_region);
                if let Some((sx, sy)) = LayerTree::get_scroll_offset(scene, *node_id) {
                    ctx.translate(sx, sy);
                }
                ctx.pop_clip();
                // 裁剪只约束子树，父节点覆盖视觉在恢复后绘制。
                if scene.node_paints_after_children(*node_id)
                    && needs_paint(scene, *node_id, dirty_region)
                {
                    // 复用统一 AfterChildren 入口保持主表面与 Picture 顺序一致。
                    LayerTree::render_widget_after_children(*node_id, ctx, scene);
                }
            }
            LayerNode::Direct {
                node_id,
                children: sub,
                ..
            } => {
                if !scene.node_visible(*node_id) {
                    continue;
                }
                if scene.node_paints_after_children(*node_id)
                    && needs_paint(scene, *node_id, dirty_region)
                {
                    let frame = scene.node_frame(*node_id);
                    ctx.save();
                    scene.paint(*node_id, frame, ctx);
                    ctx.restore();
                }
                if scene.node_visible(*node_id) {
                    render_non_picture_subtree(sub, ctx, scene, dirty_region);
                }
                // 普通 Picture 子节点也必须在后代完成后绘制覆盖视觉。
                if needs_paint(scene, *node_id, dirty_region) {
                    // 复用统一 AfterChildren 入口，避免只在主表面有效。
                    LayerTree::render_widget_after_children(*node_id, ctx, scene);
                }
            }
        }
    }
}
