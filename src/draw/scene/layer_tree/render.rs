use crate::core::{DirtyRegion, Point, Rect};
use crate::draw::geometry::types::Transform;
use crate::draw::painting::{PaintContext, PaintPass, PaintSurfaceConfig};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::image::ImageService;
use crate::draw::scene::picture::{
    LayerRenderEnv, blit_picture_cache, rasterize_picture_to_offscreen,
};
use crate::draw::scene::render_object::RenderObjectTree;
use crate::draw::scene::viewport_transform::{needs_paint_in_viewport, needs_paint_rect};
use crate::draw::scene::{NodeId, ScenePaint};
use crate::draw::{Canvas2D, FontHandle, RenderTarget};

use super::{LayerNode, LayerTree};

impl LayerTree {
    /// 按图层顺序将指定损伤区域渲染到目标表面。
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
            true,
        )
    }

    /// 只重放正常根树，供 ScenePipeline 在同一最终 present 前重建 clean backdrop。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_content(
        // 借用本帧唯一绘制目标。
        &mut self,
        // 接收可录制或真实 retained target。
        engine: &mut dyn RenderTarget,
        // 借用当前场景事实。
        scene: &impl ScenePaint,
        // 使用本次正常树重建区域。
        paint_region: &DirtyRegion,
        // 传入字体句柄。
        font: FontHandle,
        // 传入字体服务。
        font_service: &FontService,
        // 传入图片服务。
        image_service: &ImageService,
        // 保留调试模式语义。
        debug_mode: bool,
        // 保留 hover 调试位置。
        hover_pos: Option<Point>,
        // 可选同步渲染对象树。
        render_objects: Option<&mut RenderObjectTree>,
    ) -> Result<(), crate::core::Error> {
        // 复用唯一遍历器，并明确关闭 overlay 重放。
        self.render_scope(
            // 正常树写入调用方给定 target。
            engine,
            // 读取同一场景。
            scene,
            // 使用调用方指定区域。
            paint_region,
            // 传递字体句柄。
            font,
            // 传递字体服务。
            font_service,
            // 传递图片服务。
            image_service,
            // 传递调试状态。
            debug_mode,
            // 传递 hover 位置。
            hover_pos,
            // 传递渲染对象同步目标。
            render_objects,
            // 正常根树必须重放。
            true,
            // overlay 留到 snapshot/blur 完成后再重放。
            false,
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
            true,
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
        render_overlays: bool,
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

        // 兼容旧 LayerTree 参数；调试事实只由 ScenePipeline 的最终顶层 Pass 消费。
        let _ = (debug_mode, hover_pos);

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
                    render_objects.as_deref_mut(),
                );
                engine.canvas_2d().restore();
                render_result?;
                root.mark_clean();
            }
        }
        // 正常树 refresh 阶段必须在 clean snapshot 前排除全部 overlay。
        if !render_overlays {
            // 根树已完成，直接返回给 ScenePipeline 的中间提交边界。
            return Ok(());
        }
        // overlay 阶段按原有 z-order 重放。
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
        mut render_objects: Option<&mut RenderObjectTree>,
    ) -> Result<(), crate::core::Error> {
        // 克隆小型片段集合，避免在重复渲染期间借用可变节点。
        let clip_regions = node.clip_regions().map(<[Rect]>::to_vec);
        // 只有父布局显式声明片段时才进入逐片重放路径。
        let Some(clip_regions) = clip_regions else {
            // 普通节点保持原有单次绘制路径。
            return Self::render_node_unclipped(
                // 传入当前节点及共享渲染环境。
                node,
                engine,
                scene,
                dirty_region,
                env,
                surface_w,
                surface_h,
                render_objects,
            );
        };
        // 片段为空表示父布局明确隐藏整个节点子树。
        if clip_regions.is_empty() {
            // 不提交任何内容也不穿透为未裁剪绘制。
            return Ok(());
        }
        // 同一有状态节点按父布局提供的每个不连续片段重放。
        for clip_region in clip_regions {
            // 在节点自身变换之前压入父级内容坐标裁剪。
            engine.canvas_2d().push_clip(clip_region);
            // 在当前片段内执行一次完整节点与后代绘制。
            let result = Self::render_node_unclipped(
                // 复用同一节点实例，避免复制 View 状态。
                node,
                engine,
                scene,
                dirty_region,
                env,
                surface_w,
                surface_h,
                // 多片段重放共享同一 DisplayList 缓存索引。
                render_objects.as_deref_mut(),
            );
            // 无论片段内绘制是否成功都恢复父级裁剪栈。
            engine.canvas_2d().pop_clip();
            // 在恢复画布状态后传播绘制错误。
            result?;
        }
        // 所有片段均成功绘制后结束当前节点。
        Ok(())
    }

    // 在不处理父级片段的前提下执行原有节点变换与绘制流程。
    #[allow(clippy::too_many_arguments, reason = "节点渲染环境由合成递归完整传递")]
    fn render_node_unclipped(
        // 接收需要绘制的可变图层节点。
        node: &mut LayerNode,
        // 接收当前渲染目标。
        engine: &mut dyn RenderTarget,
        // 接收只读场景快照。
        scene: &impl ScenePaint,
        // 接收当前脏区域。
        dirty_region: &DirtyRegion,
        // 接收共享字体与图像资源环境。
        env: &LayerRenderEnv<'_>,
        // 接收目标表面宽度。
        surface_w: i32,
        // 接收目标表面高度。
        surface_h: i32,
        // 接收可选的显示列表缓存树。
        render_objects: Option<&mut RenderObjectTree>,
    ) -> Result<(), crate::core::Error> {
        // 节点自身变换必须位于父级片段裁剪之内。
        let transform = node.transform();
        // 读取节点自身透明度。
        let opacity = node.opacity();
        // 保存进入节点前的画布状态。
        engine.canvas_2d().save();
        // 应用节点及其后代共享的视觉变换。
        Self::apply_canvas_transform(engine.canvas_2d(), transform);
        // 读取祖先已经累计的透明度。
        let inherited_opacity = engine.canvas_2d().opacity();
        // 叠乘当前节点透明度。
        engine.canvas_2d().set_opacity(inherited_opacity * opacity);
        // 执行节点类型对应的实际绘制逻辑。
        let result = Self::render_node_inner(
            node,
            engine,
            scene,
            dirty_region,
            env,
            surface_w,
            surface_h,
            render_objects,
        );
        // 恢复进入节点前的画布变换与透明度。
        engine.canvas_2d().restore();
        // 返回节点绘制结果。
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
                // 父级片段已经由外层渲染入口处理。
                ..
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
                        render_objects.as_deref_mut(),
                    )?;
                }
                if let Some((sx, sy)) = Self::get_scroll_offset(scene, *node_id) {
                    Self::apply_canvas_transform(engine.canvas_2d(), Transform::translate(sx, sy));
                }
                engine.canvas_2d().pop_clip();
                if scene.node_paints_after_children(*node_id)
                    && Self::should_paint_node(scene, *node_id, dirty_region)
                {
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
        if pass == PaintPass::Content {
            if let Some(ro) = render_objects {
                ro.paint_content(id, frame, scene, ctx);
                ctx.restore();
                return;
            }
        }
        scene.paint(id, frame, ctx);
        ctx.restore();
    }

    /// 仅渲染 widget 自身的视觉效果（不处理子节点）。
    pub(crate) fn render_widget_self(
        id: NodeId,
        ctx: &mut PaintContext<'_>,
        scene: &impl ScenePaint,
    ) {
        Self::paint_widget(id, ctx, scene, PaintPass::Content, None);
    }

    /// 在子树完成后渲染 widget 自身的覆盖视觉。
    pub(crate) fn render_widget_after_children(
        // 接收当前场景节点身份。
        id: NodeId,
        // 借用当前目标的绘制上下文。
        ctx: &mut PaintContext<'_>,
        // 借用只读场景快照。
        scene: &impl ScenePaint,
    ) {
        // AfterChildren 不进入仅缓存 Content 的 RenderObject。
        Self::paint_widget(id, ctx, scene, PaintPass::AfterChildren, None);
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
                    render_objects.as_deref_mut(),
                )?;
            }
        }
        // 普通直绘节点也必须在全部子节点完成后获得覆盖绘制阶段。
        if scene.node_paints_after_children(id) && Self::should_paint_node(scene, id, dirty_region)
        {
            // 为当前主表面构造一次短生命周期绘制上下文。
            let mut ctx = Self::paint_context(engine, env, surface_w, surface_h);
            // AfterChildren 始终绕过只记录 Content 的显示列表缓存。
            Self::paint_widget(id, &mut ctx, scene, PaintPass::AfterChildren, None);
        }
        // 当前节点与子树全部绘制成功。
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
        scene.node_is_overlay(node_id) || needs_paint_in_viewport(scene, node_id, dirty_region)
    }
}
