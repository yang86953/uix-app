//! Picture 离屏缓存栅格化与合成（Phase 4）。

use crate::core::{Errc, Error, Point, Rect};

use super::layer_tree::{LayerNode, LayerTree};
use crate::core::DirtyRegion;
use crate::draw::FontHandle;
use crate::draw::backend::cpu::CpuDrawSurface;
use crate::draw::compositor::ScenePaint;
use crate::draw::compositor::viewport_transform::needs_paint;
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::draw::painting::{DisplayList, PaintContext, ThemeTokens};
use crate::draw::pipeline::{FrameEncoder, FrameEncoderError, FrameImage, FrameRect, NodeId};
use crate::draw::primitives::types::ImageHandle;
use crate::draw::spatial::Orientation;
use crate::draw::traits::{Canvas2D, GraphicsEngine};

/// 离屏创建连续失败上限（超过后放弃离屏、改走直绘；仅 WARN 一次）。
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
) -> Result<(), Error> {
    let src = Rect::new(0.0, 0.0, w as f32, h as f32);
    engine.try_blit_offscreen_src(handle, src, *bounds)
}

/// 将脏 Picture 栅格化到离屏缓冲。
pub(crate) fn rasterize_picture_to_offscreen<S: ScenePaint>(
    engine: &mut dyn GraphicsEngine,
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
    if !ensure_offscreen(engine, offscreen_handle, w, h) {
        *retry_count = retry_count.saturating_add(1);
        if *retry_count == MAX_OFFSCREEN_RETRY {
            crate::core::log::warn_fn(format!(
                "Picture 离屏创建失败已达 {MAX_OFFSCREEN_RETRY} 次，改走直绘，node_id={node_id}"
            ));
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

    // A cached Picture always becomes one complete, API-neutral FrameEncoder
    // before it reaches a backend. The CPU raster pass preserves every
    // DisplayList operation as premultiplied pixels; native backends consume
    // that ordered image instead of silently selecting a separate replay path.
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
                    crate::draw::pipeline::EncodedPictureExecution::Executed
                ),
                Err(_) => false,
            }
        } else {
            false
        }
    } else {
        false
    };

    let raster_result = (|| -> Result<(), Error> {
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
            env.tokens,
            env.dpi,
            env.dpr,
            env.orientation,
            w,
            h,
        );
        off_ctx.canvas_2d().translate(-bounds.x, -bounds.y);
        if !encoded_cached_picture {
            if let Some(list) = fresh_list.as_mut() {
                off_ctx.set_recorder(Some(list));
                LayerTree::render_widget_self(node_id, &mut off_ctx, scene);
                off_ctx.set_recorder(None);
                fresh_list_complete = off_ctx.recording_complete();
            } else if let Some(cached) = display_list.as_ref() {
                cached.replay(&mut off_ctx);
            }
        }
        if scene.node_visible(node_id) {
            // 屏幕 dirty 仅用于主表面剪枝；离屏已全清，子树必须完整重绘
            render_non_picture_subtree(children, &mut off_ctx, scene, &full_offscreen);
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

/// Turns a complete cached DisplayList into the only actual Picture submission
/// format. The temporary CPU surface is deliberately private: it is a
/// reference raster step, while CPU and native backends both receive the same
/// `FrameEncoder` and retain one ordered write into the bound Picture target.
pub(crate) fn encode_cached_picture(
    list: &DisplayList,
    width: i32,
    height: i32,
    origin: Point,
    font: FontHandle,
    font_service: &FontService,
    image_service: &ImageService,
) -> Result<FrameEncoder, FrameEncoderError> {
    let mut encoder = FrameEncoder::new(width, height)?;
    let mut source = CpuDrawSurface::new(width, height);
    {
        let canvas = source.canvas_mut();
        canvas.translate(-origin.x, -origin.y);
        list.replay_canvas(
            canvas,
            font,
            font_service,
            Some(image_service),
            width as f32,
        );
    }

    let image = FrameImage::new(width, height, source.surface().pixels().to_vec())?;
    let full = FrameRect::new(0, 0, width, height);
    encoder.clear(crate::draw::Color::transparent());
    encoder.blit_picture(image, full, full);
    Ok(encoder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::SoftwareEngine;
    use crate::draw::backend::DamageRegion;
    use crate::draw::engine::RenderOutcome;
    use crate::draw::image::ImageService;
    use crate::draw::painting::{
        IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, PaintOp, ShadowToken,
        ThemeTokens,
    };
    use crate::draw::traits::UpdateStrategy;

    struct TestTokens;

    macro_rules! transparent_color_tokens {
        ($($name:ident),+ $(,)?) => {
            $(
                fn $name(&self) -> crate::draw::Color {
                    crate::draw::Color::transparent()
                }
            )+
        };
    }

    impl IColorTokens for TestTokens {
        transparent_color_tokens!(
            color_primary,
            color_primary_hover,
            color_primary_active,
            color_primary_bg,
            color_primary_border,
            color_bg_container,
            color_bg_elevated,
            color_bg_raised,
            color_bg_overlay,
            color_bg_layout,
            color_bg_spotlight,
            color_bg_mask,
            color_border,
            color_border_secondary,
            color_fill,
            color_fill_secondary,
            color_fill_tertiary,
            color_fill_quaternary,
            color_text,
            color_text_secondary,
            color_text_tertiary,
            color_text_quaternary,
            color_white,
            color_black,
            color_shadow,
            color_shadow_secondary,
            color_success,
            color_success_bg,
            color_success_border,
            color_warning,
            color_warning_bg,
            color_warning_border,
            color_error,
            color_error_bg,
            color_error_border,
            color_info,
            color_info_bg,
            color_info_border,
            color_link,
            color_link_hover,
            color_link_active,
        );
    }

    impl ITypographyTokens for TestTokens {
        fn font_family(&self) -> &str {
            "test"
        }
    }

    impl ISpacingTokens for TestTokens {}

    impl IBoxShadowTokens for TestTokens {
        fn box_shadow(&self) -> ShadowToken {
            ShadowToken::none()
        }

        fn box_shadow_secondary(&self) -> ShadowToken {
            ShadowToken::none()
        }
    }

    impl ThemeTokens for TestTokens {}

    struct CachedPictureScene;

    impl ScenePaint for CachedPictureScene {
        fn root_id(&self) -> Option<NodeId> {
            Some(NodeId::new(1))
        }

        fn tree_version(&self) -> u64 {
            1
        }

        fn dirty_region(&self) -> DirtyRegion {
            DirtyRegion::full()
        }

        fn node_visible(&self, _id: NodeId) -> bool {
            true
        }

        fn node_frame(&self, _id: NodeId) -> Rect {
            Rect::new(10.0, 10.0, 16.0, 16.0)
        }

        fn node_dirty(&self, _id: NodeId) -> bool {
            false
        }

        fn node_z_index(&self, _id: NodeId) -> i32 {
            0
        }

        fn node_children(&self, _id: NodeId) -> &[NodeId] {
            &[]
        }

        fn children_clip(&self, _id: NodeId, _frame: Rect) -> Option<Rect> {
            None
        }

        fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
            frame
        }

        fn scroll_offset(&self, _id: NodeId) -> Option<(f32, f32)> {
            None
        }

        fn focused_node(&self) -> Option<NodeId> {
            None
        }

        fn node_focusable(&self, _id: NodeId) -> bool {
            false
        }

        fn hit_test(&self, _pos: Point) -> Option<NodeId> {
            None
        }

        fn parent(&self, _id: NodeId) -> Option<NodeId> {
            None
        }

        fn paint(&self, _id: NodeId, _frame: Rect, _ctx: &mut PaintContext<'_>) {}
    }

    struct CountingEngine {
        inner: SoftwareEngine,
        encoded_picture_executions: usize,
    }

    impl CountingEngine {
        fn new() -> Self {
            Self {
                inner: SoftwareEngine::new(),
                encoded_picture_executions: 0,
            }
        }
    }

    impl GraphicsEngine for CountingEngine {
        fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
            self.inner.initialize(width, height)
        }

        fn shutdown(&mut self) {
            self.inner.shutdown();
        }

        fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
            self.inner.resize(width, height)
        }

        fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
            self.inner.begin_frame(strategy)
        }

        fn end_frame(&mut self, damage: &DamageRegion) -> RenderOutcome {
            self.inner.end_frame(damage)
        }

        fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
            self.inner.canvas_2d()
        }

        fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
            self.inner.create_offscreen(width, height)
        }

        fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
            self.inner.offscreen_canvas(handle)
        }

        fn try_execute_encoded_picture(
            &mut self,
            handle: &ImageHandle,
            encoder: &FrameEncoder,
        ) -> Result<crate::draw::pipeline::EncodedPictureExecution, Error> {
            self.encoded_picture_executions += 1;
            self.inner.try_execute_encoded_picture(handle, encoder)
        }

        fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
            self.inner.try_begin_offscreen_paint(handle)
        }

        fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
            self.inner.try_flush_offscreen_paint(handle)
        }

        fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
            self.inner.try_end_offscreen_paint()
        }
    }

    #[test]
    fn cached_picture_frame_encoder_preserves_non_rect_display_list_pixels() {
        let mut list = DisplayList::new();
        list.push(PaintOp::FillCircle {
            cx: 18.0,
            cy: 18.0,
            r: 4.0,
            color: crate::draw::Color::red(),
        });

        let fonts = FontService::new();
        let images = ImageService::new();
        let encoder = encode_cached_picture(
            &list,
            16,
            16,
            Point::new(10.0, 10.0),
            FontHandle::default(),
            &fonts,
            &images,
        )
        .expect("non-rect cached Picture must have an ordered FrameEncoder");
        let frame = encoder.render_reference();

        assert_eq!(
            frame.pixel(8, 8),
            Some(crate::draw::Color::red().premultiplied())
        );
        assert_eq!(
            frame.pixel(1, 1),
            Some(crate::draw::Color::transparent().premultiplied())
        );
    }

    #[test]
    fn cached_complete_picture_is_executed_by_the_production_compositor() {
        let mut engine = CountingEngine::new();
        engine.initialize(16, 16).expect("software engine");

        let mut list = DisplayList::new();
        list.push(PaintOp::FillCircle {
            cx: 18.0,
            cy: 18.0,
            r: 4.0,
            color: crate::draw::Color::red(),
        });
        let fonts = FontService::new();
        let images = ImageService::new();
        let tokens = TestTokens;
        let env = LayerRenderEnv {
            font: FontHandle::default(),
            font_service: &fonts,
            image_service: &images,
            tokens: &tokens,
            dpi: 96.0,
            dpr: 1.0,
            orientation: Orientation::YDown,
        };
        let scene = CachedPictureScene;
        let mut offscreen = None;
        let mut display_list = Some(list);
        let mut children = [];
        let mut is_dirty = true;
        let mut retry_count = 0;

        rasterize_picture_to_offscreen(
            &mut engine,
            NodeId::new(1),
            &Rect::new(10.0, 10.0, 16.0, 16.0),
            &mut offscreen,
            &mut display_list,
            &mut children,
            &mut is_dirty,
            &scene,
            &DirtyRegion::full(),
            16,
            16,
            &mut retry_count,
            &env,
        )
        .expect("cached Picture must rasterize");

        assert_eq!(engine.encoded_picture_executions, 1);
        assert!(!is_dirty);
        let handle = offscreen.expect("Picture target");
        let (pixels, stride) = engine
            .inner
            .copy_offscreen_pixels(&handle)
            .expect("encoded Picture pixels");
        assert_eq!(
            pixels[8 * stride as usize + 8],
            crate::draw::Color::red().premultiplied()
        );
    }
}

fn prepare_nested_pictures<S: ScenePaint>(
    engine: &mut dyn GraphicsEngine,
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
            } => {
                if needs_paint(scene, *node_id, dirty_region) {
                    LayerTree::render_widget_self(*node_id, ctx, scene);
                }
                ctx.canvas_2d().push_clip(*rect);
                if let Some((sx, sy)) = LayerTree::get_scroll_offset(scene, *node_id) {
                    ctx.canvas_2d().translate(-sx, -sy);
                }
                render_non_picture_subtree(sub, ctx, scene, dirty_region);
                if let Some((sx, sy)) = LayerTree::get_scroll_offset(scene, *node_id) {
                    ctx.canvas_2d().translate(sx, sy);
                }
                ctx.canvas_2d().pop_clip();
            }
            LayerNode::Direct {
                node_id,
                children: sub,
            } => {
                if !scene.node_visible(*node_id) {
                    continue;
                }
                if needs_paint(scene, *node_id, dirty_region) {
                    let frame = scene.node_frame(*node_id);
                    ctx.save();
                    scene.paint(*node_id, frame, ctx);
                    ctx.restore();
                }
                if scene.node_visible(*node_id) {
                    render_non_picture_subtree(sub, ctx, scene, dirty_region);
                }
            }
        }
    }
}
