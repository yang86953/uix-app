//! Picture 离屏缓存栅格化与合成（Phase 4）。

use crate::core::Rect;

use super::layer_tree::{LayerNode, LayerTree};
use crate::core::DirtyRegion;
use crate::draw::compositor::viewport_transform::needs_paint;
use crate::draw::compositor::ScenePaint;
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::draw::painting::{DisplayList, PaintContext, ThemeTokens};
use crate::draw::pipeline::NodeId;
use crate::draw::primitives::color::Color;
use crate::draw::primitives::types::ImageHandle;
use crate::draw::spatial::Orientation;
use crate::draw::traits::GraphicsEngine;
use crate::draw::FontHandle;

/// 离屏创建连续失败上限（超过后本帧跳过栅格化，下帧 rebuild 重置）。
const MAX_OFFSCREEN_RETRY: u8 = 8;

/// 图层渲染环境（绘制资源，供 Picture 离屏路径复用）。
pub(crate) struct LayerRenderEnv<'a> {
    pub font: FontHandle,
    pub font_service: &'a FontService,
    pub image_service: &'a ImageService,
    pub tokens: &'a dyn ThemeTokens,
    pub dpi: f32,
    pub dpr: f32,
    pub orientation: Orientation,
}

/// 将 Picture 缓存 blit 到主缓冲。
pub(crate) fn blit_picture_cache(
    engine: &mut dyn GraphicsEngine,
    handle: &ImageHandle,
    bounds: &Rect,
    w: i32,
    h: i32,
) {
    let src = Rect::new(0.0, 0.0, w as f32, h as f32);
    if let Some((pixels, pw)) = engine.copy_offscreen_pixels(handle) {
        engine.canvas_2d().blit_image(&pixels, pw, src, *bounds);
    }
}

/// 将脏 Picture 栅格化到离屏缓冲。
pub(crate) fn rasterize_picture_to_offscreen<S: ScenePaint>(
    engine: &mut dyn GraphicsEngine,
    widget_id: NodeId,
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
) {
    prepare_nested_pictures(engine, children, scene, paint_region, env);

    if !ensure_offscreen(engine, offscreen_handle, w, h) {
        *retry_count = retry_count.saturating_add(1);
        if *retry_count >= MAX_OFFSCREEN_RETRY {
            crate::core::log::warn_fn(format!(
                "Picture 离屏创建失败已达 {MAX_OFFSCREEN_RETRY} 次，widget_id={widget_id}"
            ));
        }
        return;
    }
    *retry_count = 0;
    let handle = match offscreen_handle {
        Some(h) => *h,
        None => return,
    };

    let nested_blits = collect_picture_blit_info(children, bounds);
    let nested_pixels: Vec<(Rect, Vec<u32>, i32)> = nested_blits
        .iter()
        .filter_map(|(child_bounds, child_handle)| {
            let (pixels, pw) = engine.copy_offscreen_pixels(child_handle)?;
            Some((*child_bounds, pixels, pw))
        })
        .collect();

    let self_dirty = scene.node_dirty(widget_id);
    let mut fresh_list = if self_dirty || display_list.is_none() {
        Some(DisplayList::new())
    } else {
        None
    };

    {
        let Some(off_canvas) = engine.offscreen_canvas(&handle) else {
            return;
        };
        off_canvas.fill_rect(
            Rect::new(0.0, 0.0, w as f32, h as f32),
            Color::transparent(),
            None,
        );
        let mut off_ctx = PaintContext::new(
            off_canvas,
            env.font,
            env.font_service,
            env.image_service,
            env.tokens,
            env.dpi,
            env.dpr,
            env.orientation,
            w,
            h,
        );
        off_ctx.canvas_2d().translate(-bounds.x, -bounds.y);
        if let Some(list) = fresh_list.as_mut() {
            off_ctx.set_recorder(Some(list));
            LayerTree::render_widget_self(widget_id, &mut off_ctx, scene);
            off_ctx.set_recorder(None);
        } else if let Some(cached) = display_list.as_ref() {
            cached.replay(&mut off_ctx);
        }
        if scene.node_visible(widget_id) {
            render_non_picture_subtree(children, &mut off_ctx, scene, paint_region);
        }
        for (child_bounds, pixels, pw) in nested_pixels {
            let local = Rect::new(
                child_bounds.x - bounds.x,
                child_bounds.y - bounds.y,
                child_bounds.w,
                child_bounds.h,
            );
            let src = Rect::new(0.0, 0.0, child_bounds.w, child_bounds.h);
            off_ctx.canvas_2d().blit_image(&pixels, pw, src, local);
        }
    }

    if let Some(list) = fresh_list {
        *display_list = Some(list);
    }

    *is_dirty = false;
}

fn ensure_offscreen(
    engine: &mut dyn GraphicsEngine,
    handle: &mut Option<ImageHandle>,
    w: i32,
    h: i32,
) -> bool {
    if w <= 0 || h <= 0 {
        return false;
    }
    if let Some(hdl) = *handle {
        if let Some(canvas) = engine.offscreen_canvas(&hdl) {
            if canvas.width() == w && canvas.height() == h {
                return true;
            }
        }
        engine.destroy_offscreen(hdl);
        *handle = None;
    }
    *handle = engine.create_offscreen(w, h);
    handle.is_some()
}

fn prepare_nested_pictures<S: ScenePaint>(
    engine: &mut dyn GraphicsEngine,
    children: &mut [LayerNode],
    scene: &S,
    paint_region: &DirtyRegion,
    env: &LayerRenderEnv<'_>,
) {
    for child in children.iter_mut() {
        if let LayerNode::Picture {
            widget_id,
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
                    *widget_id,
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
                );
            }
        }
        let (LayerNode::Picture { children: sub, .. }
        | LayerNode::ClipRect { children: sub, .. }
        | LayerNode::Direct { children: sub, .. }) = child;
        {
            prepare_nested_pictures(engine, sub, scene, paint_region, env);
        }
    }
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
                widget_id,
                rect,
                children: sub,
            } => {
                if needs_paint(scene, *widget_id, dirty_region) {
                    LayerTree::render_widget_self(*widget_id, ctx, scene);
                }
                ctx.canvas_2d().push_clip(*rect);
                if let Some((sx, sy)) = LayerTree::get_scroll_offset(scene, *widget_id) {
                    ctx.canvas_2d().translate(-sx, -sy);
                }
                render_non_picture_subtree(sub, ctx, scene, dirty_region);
                if let Some((sx, sy)) = LayerTree::get_scroll_offset(scene, *widget_id) {
                    ctx.canvas_2d().translate(sx, sy);
                }
                ctx.canvas_2d().pop_clip();
            }
            LayerNode::Direct {
                widget_id,
                children: sub,
            } => {
                if !scene.node_visible(*widget_id) {
                    continue;
                }
                if needs_paint(scene, *widget_id, dirty_region) {
                    let frame = scene.node_frame(*widget_id);
                    ctx.save();
                    scene.paint(*widget_id, frame, ctx);
                    ctx.restore();
                }
                if scene.node_visible(*widget_id) {
                    render_non_picture_subtree(sub, ctx, scene, dirty_region);
                }
            }
        }
    }
}
