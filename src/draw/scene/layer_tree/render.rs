use std::collections::HashSet;

use crate::core::{DirtyRegion, Point};
use crate::draw::geometry::types::Transform;
use crate::draw::painting::{PaintContext, PaintPass, PaintSurfaceConfig};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::image::ImageService;
use crate::draw::scene::picture::{
    blit_picture_cache, rasterize_picture_to_offscreen, LayerRenderEnv,
};
use crate::draw::scene::render_object::RenderObjectTree;
use crate::draw::scene::viewport_transform::{needs_paint, needs_paint_rect};
use crate::draw::scene::{NodeId, ScenePaint};
use crate::draw::{Canvas2D, FontHandle, RenderTarget};

use super::{DebugHover, LayerNode, LayerTree};

impl LayerTree {

    pub fn render(
        &mut self,
        engine: &mut dyn RenderTarget,
        scene: &impl ScenePaint,
        paint_region: &DirtyRegion,
        font: FontHandle,
        font_service: &FontService,
        image_service: &ImageService,
        debug_mode: bool,
        hover_pos: Option<Point>,
        render_objects: Option<&mut RenderObjectTree>,
    ) -> Result<(), crate::core::Error> {
        self.render_scope(
            engine,
            scene,
            paint_region,
            font,
            font_service,
            image_service,
            debug_mode,
            hover_pos,
            render_objects,
            true,
        )
    }

    /// Replays only root-level overlays. ScenePipeline uses this after it has
    /// restored a proven clean retained backdrop for a full overlay frame.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_overlays(
        &mut self,
        engine: &mut dyn RenderTarget,
        scene: &impl ScenePaint,
        paint_region: &DirtyRegion,
        font: FontHandle,
        font_service: &FontService,
        image_service: &ImageService,
        debug_mode: bool,
        hover_pos: Option<Point>,
        render_objects: Option<&mut RenderObjectTree>,
    ) -> Result<(), crate::core::Error> {
        self.render_scope(
            engine,
            scene,
            paint_region,
            font,
            font_service,
            image_service,
            debug_mode,
            hover_pos,
            render_objects,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_scope(
        &mut self,
        engine: &mut dyn RenderTarget,
        scene: &impl ScenePaint,
        paint_region: &DirtyRegion,
        font: FontHandle,
        font_service: &FontService,
        image_service: &ImageService,
        debug_mode: bool,
        hover_pos: Option<Point>,
        mut render_objects: Option<&mut RenderObjectTree>,
        render_root: bool,
    ) -> Result<(), crate::core::Error> {
        let dpi = engine.dpi();
        let dpr = engine.device_pixel_ratio();
        let orientation = engine.orientation();
        let surface_w = engine.canvas_2d().width();
        let surface_h = engine.canvas_2d().height();
        let env = LayerRenderEnv {
            font,
            font_service,
            image_service,
            dpi,
            dpr,
            orientation,
        };

        // 仅 hover 祖先链画边框；leaf = hit_test 最深节点（尺寸标签只贴它）。
        let debug_hover: Option<DebugHover> = if debug_mode {
            hover_pos.and_then(|pos| {
                let leaf = scene.hit_test(pos)?;
                let mut chain = HashSet::new();
                let mut current = leaf;
                chain.insert(current);
                while let Some(pid) = scene.parent(current) {
                    chain.insert(pid);
                    current = pid;
                }
                Some(DebugHover { chain, leaf })
            })
        } else {
            None
        };

        if render_root {
            if let Some(ref mut root) = self.root {
                // The normal tree may establish viewport clips or scroll translations.
                // Root-level overlays must start from the frame's original canvas state,
                // even if a backend retains state after the normal-tree traversal.
                engine.canvas_2d().save();
                let render_result = Self::render_node(
                    root,
                    engine,
                    scene,
                    paint_region,
                    &env,
                    surface_w,
                    surface_h,
                    debug_mode,
                    &debug_hover,
                    0,
                    render_objects.as_deref_mut(),
                );
                engine.canvas_2d().restore();
                render_result?;
                root.mark_clean();
            }
        }
        for overlay in &mut self.overlays {
            // Isolate sibling overlays too: one overlay cannot clip or translate
            // the next one, and neither can inherit normal-tree state.
            engine.canvas_2d().save();
            let render_result = Self::render_node(
                overlay,
                engine,
                scene,
                paint_region,
                &env,
                surface_w,
                surface_h,
                debug_mode,
                &debug_hover,
                0,
                render_objects.as_deref_mut(),
            );
            engine.canvas_2d().restore();
            render_result?;
            overlay.mark_clean();
        }
        Ok(())
    }

    /// 创建短生命周期绘制上下文（避免与 Picture 离屏路径争用 engine 借用）。
    fn paint_context<'a>(
        engine: &'a mut dyn RenderTarget,
        env: &'a LayerRenderEnv<'_>,
        surface_w: i32,
        surface_h: i32,
    ) -> PaintContext<'a> {
        PaintContext::new(
            engine.canvas_2d(),
            env.font,
            env.font_service,
            env.image_service,
            PaintSurfaceConfig {
                dpi: env.dpi,
                device_pixel_ratio: env.dpr,
                orientation: env.orientation,
                surface_w,
                surface_h,
            },
        )
    }


}

impl LayerTree {

    /// 递归渲染单个节点；脏剪枝使用 viewport 坐标变换（Phase 5）。
    fn render_node(
        node: &mut LayerNode,
        engine: &mut dyn RenderTarget,
        scene: &impl ScenePaint,
        dirty_region: &DirtyRegion,
        env: &LayerRenderEnv<'_>,
        surface_w: i32,
        surface_h: i32,
        debug_mode: bool,
        debug_hover: &Option<DebugHover>,
        depth: usize,
        render_objects: Option<&mut RenderObjectTree>,
    ) -> Result<(), crate::core::Error> {
        let transform = node.transform();
        let opacity = node.opacity();
        engine.canvas_2d().save();
        Self::apply_canvas_transform(engine.canvas_2d(), transform);
        let inherited_opacity = engine.canvas_2d().opacity();
        engine.canvas_2d().set_opacity(inherited_opacity * opacity);
        let result = Self::render_node_inner(
            node,
            engine,
            scene,
            dirty_region,
            env,
            surface_w,
            surface_h,
            debug_mode,
            debug_hover,
            depth,
            render_objects,
        );
        engine.canvas_2d().restore();
        result
    }

    fn render_node_inner(
        node: &mut LayerNode,
        engine: &mut dyn RenderTarget,
        scene: &impl ScenePaint,
        dirty_region: &DirtyRegion,
        env: &LayerRenderEnv<'_>,
        surface_w: i32,
        surface_h: i32,
        debug_mode: bool,
        debug_hover: &Option<DebugHover>,
        depth: usize,
        mut render_objects: Option<&mut RenderObjectTree>,
    ) -> Result<(), crate::core::Error> {
        match node {
            LayerNode::Picture {
                node_id,
                bounds,
                is_dirty,
                offscreen_handle,
                display_list,
                children,
                retry_count,
            } => {
                let w = bounds.w.ceil() as i32;
                let h = bounds.h.ceil() as i32;
                if w <= 0 || h <= 0 {
                    return Ok(());
                }

                let needs_blit =
                    *is_dirty || needs_paint_rect(scene, *node_id, *bounds, dirty_region);

                if *is_dirty || offscreen_handle.is_none() {
                    rasterize_picture_to_offscreen(
                        engine,
                        *node_id,
                        bounds,
                        offscreen_handle,
                        display_list,
                        children,
                        is_dirty,
                        scene,
                        dirty_region,
                        w,
                        h,
                        retry_count,
                        env,
                    )?;
                }

                if offscreen_handle.is_none() {
                    Self::render_widget_and_children(
                        engine,
                        *node_id,
                        children,
                        scene,
                        dirty_region,
                        env,
                        surface_w,
                        surface_h,
                        debug_mode,
                        debug_hover,
                        depth,
                        render_objects,
                    )?;
                    return Ok(());
                }

                if needs_blit {
                    if let Some(handle) = offscreen_handle.as_ref() {
                        blit_picture_cache(engine, handle, bounds, w, h)?;
                    }
                }
            }
            LayerNode::ClipRect {
                node_id,
                rect,
                children,
                ..
            } => {
                if Self::should_paint_node(scene, *node_id, dirty_region) {
                    let mut ctx = Self::paint_context(engine, env, surface_w, surface_h);
                    Self::paint_widget(
                        *node_id,
                        &mut ctx,
                        scene,
                        PaintPass::Content,
                        render_objects.as_deref_mut(),
                    );
                    Self::draw_debug_for_widget(
                        &mut ctx,
                        scene,
                        *node_id,
                        debug_mode,
                        debug_hover,
                        depth,
                    );
                }
                engine.canvas_2d().push_clip(*rect);
                if let Some((sx, sy)) = Self::get_scroll_offset(scene, *node_id) {
                    Self::apply_canvas_transform(
                        engine.canvas_2d(),
                        Transform::translate(-sx, -sy),
                    );
                }
                for child in children.iter_mut() {
                    Self::render_node(
                        child,
                        engine,
                        scene,
                        dirty_region,
                        env,
                        surface_w,
                        surface_h,
                        debug_mode,
                        debug_hover,
                        depth + 1,
                        render_objects.as_deref_mut(),
                    )?;
                }
                if let Some((sx, sy)) = Self::get_scroll_offset(scene, *node_id) {
                    Self::apply_canvas_transform(engine.canvas_2d(), Transform::translate(sx, sy));
                }
                engine.canvas_2d().pop_clip();
                if Self::should_paint_node(scene, *node_id, dirty_region) {
                    let mut ctx = Self::paint_context(engine, env, surface_w, surface_h);
                    Self::paint_widget(*node_id, &mut ctx, scene, PaintPass::AfterChildren, None);
                }
            }
            LayerNode::Direct {
                node_id, children, ..
            } => {
                Self::render_widget_and_children(
                    engine,
                    *node_id,
                    children,
                    scene,
                    dirty_region,
                    env,
                    surface_w,
                    surface_h,
                    debug_mode,
                    debug_hover,
                    depth,
                    render_objects,
                )?;
            }
        }
        Ok(())
    }

    /// Keeps the legacy offset fast path for pure translations, but folds an
    /// existing offset into the affine matrix before a scale/shear is applied.
    /// This preserves the order `ancestor * scroll * child_transform`.
    fn apply_canvas_transform(canvas: &mut dyn Canvas2D, transform: Transform) {
        if transform.is_identity() {
            return;
        }
        let [a, b, tx, c, d, ty] = transform.m;
        let translation_only = a == 1.0 && b == 0.0 && c == 0.0 && d == 1.0;
        if canvas.current_transform().is_identity() && translation_only {
            canvas.translate(tx, ty);
            return;
        }

        let mut current = canvas.current_transform();
        let (offset_x, offset_y) = canvas.offset();
        if offset_x != 0.0 || offset_y != 0.0 {
            current = current.concat(Transform::translate(offset_x, offset_y));
            canvas.set_offset(0.0, 0.0);
        }
        canvas.set_transform(current.concat(transform));
    }

    /// 按绘制阶段调用 widget `paint`；Content 阶段可走 RenderObject DisplayList 缓存。
    fn paint_widget(
        id: NodeId,
        ctx: &mut PaintContext<'_>,
        scene: &impl ScenePaint,
        pass: PaintPass,
        render_objects: Option<&mut RenderObjectTree>,
    ) {
        if !scene.node_visible(id) {
            return;
        }
        let frame = scene.node_frame(id);
        ctx.set_paint_pass(pass);
        ctx.save();
        let paint_t0 = std::time::Instant::now();
        if pass == PaintPass::Content {
            if let Some(ro) = render_objects {
                ro.paint_content(id, frame, scene, ctx);
                ctx.restore();
                crate::core::perf_probe::add_direct_paint(paint_t0.elapsed().as_micros());
                crate::core::perf_probe::add_widget_painted();
                return;
            }
        }
        scene.paint(id, frame, ctx);
        ctx.restore();
        crate::core::perf_probe::add_direct_paint(paint_t0.elapsed().as_micros());
        crate::core::perf_probe::add_widget_painted();
    }

    fn draw_debug_for_widget(
        ctx: &mut PaintContext<'_>,
        scene: &impl ScenePaint,
        node_id: NodeId,
        debug_mode: bool,
        debug_hover: &Option<DebugHover>,
        depth: usize,
    ) {
        if !debug_mode || !scene.node_visible(node_id) {
            return;
        }
        let Some(hover) = debug_hover.as_ref() else {
            return;
        };
        // 默认不画满屏淡彩框：仅 hover 祖先链。
        if !hover.chain.contains(&node_id) {
            return;
        }
        // PaintContext 默认 debug=false；须显式打开，否则 draw_debug_* 全部 no-op。
        ctx.set_debug_mode(true);
        let frame = scene.node_frame(node_id);
        ctx.draw_debug_border(frame, depth, true);
        // 尺寸标签只贴最深命中节点，避免祖先链叠多块黑条。
        if node_id == hover.leaf {
            ctx.draw_debug_frame_info(node_id.slot(), frame);
        }
    }

    /// 仅渲染 widget 自身的视觉效果（不处理子节点）。
    pub(crate) fn render_widget_self(
        id: NodeId,
        ctx: &mut PaintContext<'_>,
        scene: &impl ScenePaint,
    ) {
        Self::paint_widget(id, ctx, scene, PaintPass::Content, None);
    }

    /// 渲染 widget 自身及其子节点（直接遍历 children LayerNodes）。
    fn render_widget_and_children(
        engine: &mut dyn RenderTarget,
        id: NodeId,
        children: &mut [LayerNode],
        scene: &impl ScenePaint,
        dirty_region: &DirtyRegion,
        env: &LayerRenderEnv<'_>,
        surface_w: i32,
        surface_h: i32,
        debug_mode: bool,
        debug_hover: &Option<DebugHover>,
        depth: usize,
        mut render_objects: Option<&mut RenderObjectTree>,
    ) -> Result<(), crate::core::Error> {
        if !scene.node_visible(id) {
            return Ok(());
        }
        if Self::should_paint_node(scene, id, dirty_region) {
            let mut ctx = Self::paint_context(engine, env, surface_w, surface_h);
            Self::paint_widget(
                id,
                &mut ctx,
                scene,
                PaintPass::Content,
                render_objects.as_deref_mut(),
            );
            Self::draw_debug_for_widget(&mut ctx, scene, id, debug_mode, debug_hover, depth);
        }

        if scene.node_visible(id) {
            for child in children.iter_mut() {
                Self::render_node(
                    child,
                    engine,
                    scene,
                    dirty_region,
                    env,
                    surface_w,
                    surface_h,
                    debug_mode,
                    debug_hover,
                    depth + 1,
                    render_objects.as_deref_mut(),
                )?;
            }
        }
        Ok(())
    }

    /// 获取滚动容器的 content 偏移（viewport → content）。
    pub(crate) fn get_scroll_offset(
        scene: &impl ScenePaint,
        node_id: NodeId,
    ) -> Option<(f32, f32)> {
        scene.scroll_offset(node_id)
    }

    /// A root-level overlay may intentionally have a zero layout slot (for
    /// example, a masked Drawer). Its visual bounds live in overlay space, so
    /// normal frame-based dirty culling would incorrectly skip it after the
    /// opening animation settles or another root widget repaints.
    fn should_paint_node(
        scene: &impl ScenePaint,
        node_id: NodeId,
        dirty_region: &DirtyRegion,
    ) -> bool {
        scene.node_is_overlay(node_id) || needs_paint(scene, node_id, dirty_region)
    }

}

