//! LayerTree — 合成树（Phase 3 迁入 draw）。
//!
//! 通过 ScenePaint trait 读取场景结构并下发绘制，不依赖 ui 域。

use std::collections::HashSet;

use crate::native::{Point, Rect};

use crate::draw::compositor::picture::{blit_picture_cache, rasterize_picture_to_offscreen, LayerRenderEnv};
use crate::draw::compositor::viewport_transform::{needs_paint, needs_paint_rect};
use crate::draw::compositor::ScenePaint;
use crate::draw::font::font_service::FontService;
use crate::draw::painting::{DisplayList, PaintContext, PaintPass, ThemeSnapshot};
use crate::draw::pipeline::NodeId;
use crate::draw::render_object::RenderObjectTree;
use crate::draw::traits::GraphicsEngine;
use crate::draw::primitives::types::{DirtyRegion, ImageHandle};
use crate::draw::FontHandle;

/// 图层节点。
pub enum LayerNode {
    /// 图片图层：缓存 RepaintBoundary 子树的栅格结果。
    /// 包含 children 以支持嵌套 RepaintBoundary（#96）。
    Picture {
        widget_id: NodeId,
        bounds: Rect,
        is_dirty: bool,
        offscreen_handle: Option<ImageHandle>,
        /// widget 自身 Content 阶段的 DisplayList（Phase 7 离屏回放缓存）。
        display_list: Option<DisplayList>,
        children: Vec<LayerNode>,
        /// 离屏创建连续失败计数，rebuild 时重置为 0。
        retry_count: u8,
    },
    /// 裁剪图层：将子图层内容限制在矩形区域内。
    ClipRect {
        widget_id: NodeId,
        rect: Rect,
        children: Vec<LayerNode>,
    },
    /// 普通节点：直接渲染 widget 及其子树（无特殊图层语义，无离屏缓存）。
    Direct {
        widget_id: NodeId,
        children: Vec<LayerNode>,
    },
}

impl std::fmt::Debug for LayerNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerNode::Picture {
                widget_id,
                bounds,
                is_dirty,
                offscreen_handle,
                display_list,
                children,
                retry_count,
            } => f
                .debug_struct("PictureLayer")
                .field("widget_id", widget_id)
                .field("bounds", bounds)
                .field("is_dirty", is_dirty)
                .field("has_offscreen", &offscreen_handle.is_some())
                .field("has_display_list", &display_list.is_some())
                .field("children_count", &children.len())
                .field("retry_count", retry_count)
                .finish(),
            LayerNode::ClipRect {
                widget_id,
                rect,
                children,
            } => f
                .debug_struct("ClipRectLayer")
                .field("widget_id", widget_id)
                .field("rect", rect)
                .field("children_count", &children.len())
                .finish(),
            LayerNode::Direct {
                widget_id,
                children,
            } => f
                .debug_struct("DirectLayer")
                .field("widget_id", widget_id)
                .field("children_count", &children.len())
                .finish(),
        }
    }
}

impl LayerNode {
    fn mark_dirty(&mut self) {
        match self {
            LayerNode::Picture {
                is_dirty, children, ..
            } => {
                *is_dirty = true;
                for child in children.iter_mut() {
                    child.mark_dirty();
                }
            }
            LayerNode::ClipRect { children, .. } | LayerNode::Direct { children, .. } => {
                for child in children.iter_mut() {
                    child.mark_dirty();
                }
            }
        }
    }

    fn mark_clean(&mut self) {
        match self {
            LayerNode::Picture {
                is_dirty, children, ..
            } => {
                *is_dirty = false;
                for child in children.iter_mut() {
                    child.mark_clean();
                }
            }
            LayerNode::ClipRect { children, .. } | LayerNode::Direct { children, .. } => {
                for child in children.iter_mut() {
                    child.mark_clean();
                }
            }
        }
    }

    /// 获取该节点对应的 widget_id。
    fn widget_id(&self) -> NodeId {
        match self {
            LayerNode::Picture { widget_id, .. }
            | LayerNode::ClipRect { widget_id, .. }
            | LayerNode::Direct { widget_id, .. } => *widget_id,
        }
    }
}

/// 图层树 —— 从 ScenePaint 构建的可缓存合成栈。
///
/// 子节点在 build 时按 z_index 预排序，渲染时直接遍历无需额外排序。
pub struct LayerTree {
    root: Option<LayerNode>,
    /// build 后未复用的旧离屏句柄（等待 sweep 释放）。
    orphaned_handles: Vec<ImageHandle>,
}

impl std::fmt::Debug for LayerTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayerTree")
            .field("has_root", &self.root.is_some())
            .finish()
    }
}

impl LayerTree {
    pub fn new() -> Self {
        Self {
            root: None,
            orphaned_handles: Vec::new(),
        }
    }

    /// 从 ScenePaint 构建图层树。
    /// 生成所有可见 widget 的 LayerNode（无遗漏）。
    /// 自动复用已缓存离屏缓冲——按 widget_id 匹配旧 Picture 节点。
    /// 子节点按 z_index 预排序，渲染时无需再排序（#97）。
    ///
    /// 注意：build 后未复用的旧离屏缓冲句柄暂存在 `orphaned_handles` 中，
    /// 调用者需在合适的时机调用 `sweep_orphaned_offscreens` 释放。
    pub fn build(&mut self, scene: &impl ScenePaint) {
        // 收集旧 Picture 节点的离屏缓冲与 DisplayList（按 widget_id 索引）
        let mut old_cache: std::collections::HashMap<
            NodeId,
            (Rect, Option<ImageHandle>, Option<DisplayList>),
        > = std::collections::HashMap::new();
        if let Some(ref root) = self.root {
            Self::collect_picture_handles(root, &mut old_cache);
        }

        self.root = scene
            .root_id()
            .and_then(|root_id| Self::build_node_cached(scene, root_id, &old_cache));

        // 收集未复用的旧句柄（需要在引擎上下文中释放）
        self.orphaned_handles.clear();
        for (_, (_, handle_opt, _)) in old_cache {
            if let Some(h) = handle_opt {
                self.orphaned_handles.push(h);
            }
        }
    }

    /// 释放 build 后未复用的旧离屏缓冲。
    /// 必须在 build 之后、下一帧渲染之前调用。
    pub fn sweep_orphaned_offscreens(&mut self, engine: &mut dyn GraphicsEngine) {
        for handle in self.orphaned_handles.drain(..) {
            engine.destroy_offscreen(handle);
        }
    }

    /// 增量更新脏状态。
    pub fn update_dirty(&mut self, scene: &impl ScenePaint) {
        if let Some(ref mut root) = self.root {
            Self::update_dirty_node(root, scene);
        }
    }

    /// 单 Pass 渲染：按 z-order 合成 widget 绘制、调试覆盖与焦点环（Phase 7）。
    ///
    /// `paint_region` 为实际绘制剪枝区域（可与 `scene.dirty_region()` 不同，
    /// 例如首帧强制全帧重绘时由 FrameRenderer 传入 `DirtyRegion::full()`）。
    pub fn render(
        &mut self,
        engine: &mut dyn GraphicsEngine,
        scene: &impl ScenePaint,
        paint_region: &DirtyRegion,
        theme: &ThemeSnapshot<'_>,
        font: FontHandle,
        font_service: &FontService,
        debug_mode: bool,
        hover_pos: Option<Point>,
        render_objects: Option<&mut RenderObjectTree>,
    ) {
        let dpi = engine.dpi();
        let dpr = engine.device_pixel_ratio();
        let orientation = engine.orientation();
        let surface_w = engine.canvas_2d().width();
        let surface_h = engine.canvas_2d().height();
        let env = LayerRenderEnv {
            font,
            font_service,
            tokens: theme.tokens(),
            dpi,
            dpr,
            orientation,
        };

        let hovered_chain: Option<HashSet<NodeId>> = if debug_mode {
            hover_pos.and_then(|pos| {
                let deepest = scene.hit_test(pos)?;
                let mut chain = HashSet::new();
                let mut current = deepest;
                chain.insert(current);
                while let Some(pid) = scene.parent(current) {
                    chain.insert(pid);
                    current = pid;
                }
                Some(chain)
            })
        } else {
            None
        };

        if let Some(ref mut root) = self.root {
            Self::render_node(
                root,
                engine,
                scene,
                paint_region,
                &env,
                surface_w,
                surface_h,
                debug_mode,
                &hovered_chain,
                0,
                render_objects,
            );
            root.mark_clean();
        }

        // 焦点环
        if let Some(focused_id) = scene.focused_node() {
            if scene.node_visible(focused_id) && scene.node_focusable(focused_id) {
                let frame = scene.node_frame(focused_id);
                let focus_color = theme.tokens().color_primary();
                let mut ctx = Self::paint_context(engine, &env, surface_w, surface_h);
                ctx.canvas_2d().stroke_rect(frame, focus_color, 2.0, None);
            }
        }
    }

    /// 创建短生命周期绘制上下文（避免与 Picture 离屏路径争用 engine 借用）。
    fn paint_context<'a>(
        engine: &'a mut dyn GraphicsEngine,
        env: &'a LayerRenderEnv<'_>,
        surface_w: i32,
        surface_h: i32,
    ) -> PaintContext<'a> {
        PaintContext::new(
            engine.canvas_2d(),
            env.font,
            env.font_service,
            env.tokens,
            env.dpi,
            env.dpr,
            env.orientation,
            surface_w,
            surface_h,
        )
    }

    pub fn invalidate(&mut self) {
        if let Some(ref mut root) = self.root {
            root.mark_dirty();
        }
    }

    pub fn is_ready(&self) -> bool {
        self.root.is_some()
    }

    // ── 内部 ──

    /// 从旧 LayerTree 中收集所有 Picture 节点的离屏缓冲句柄。
    /// 同时重置 retry_count 为 0（rebuild 意味着新的尝试机会）。
    fn collect_picture_handles(
        node: &LayerNode,
        cache: &mut std::collections::HashMap<
            NodeId,
            (Rect, Option<ImageHandle>, Option<DisplayList>),
        >,
    ) {
        match node {
            LayerNode::Picture {
                widget_id,
                bounds,
                offscreen_handle,
                display_list,
                ..
            } => {
                cache.insert(
                    *widget_id,
                    (*bounds, *offscreen_handle, display_list.clone()),
                );
            }
            LayerNode::ClipRect { children, .. } | LayerNode::Direct { children, .. } => {
                for child in children {
                    Self::collect_picture_handles(child, cache);
                }
            }
        }
    }

    /// 带缓存复用的 build_node。
    /// 如果旧缓存中有相同 widget_id 且 bounds 未变的 Picture，复用其离屏句柄。
    ///
    /// 注意：`frame` 是绝对坐标（由 layout 阶段设置），以下所有位置计算直接使用 `frame` 的坐标，
    /// 不再累加父级偏移。
    fn build_node_cached(
        scene: &impl ScenePaint,
        id: NodeId,
        cache: &std::collections::HashMap<
            NodeId,
            (Rect, Option<ImageHandle>, Option<DisplayList>),
        >,
    ) -> Option<LayerNode> {
        if !scene.node_visible(id) {
            return None;
        }
        let frame = scene.node_frame(id);

        if scene.is_repaint_boundary(id) {
            // frame 是绝对坐标，直接用作 bounds
            let bounds = frame;
            // 检查旧缓存：如果 bounds 相同，复用离屏句柄
            let (offscreen_handle, display_list) =
                cache.get(&id).map(|(old_bounds, old_handle, old_list)| {
                    if *old_bounds == bounds {
                        (*old_handle, old_list.clone())
                    } else {
                        (None, None)
                    }
                }).unwrap_or((None, None));
            let children = Self::build_children_cached(scene, id, cache);
            Some(LayerNode::Picture {
                widget_id: id,
                bounds,
                is_dirty: offscreen_handle.is_none()
                    || display_list.is_none()
                    || scene.node_dirty(id),
                offscreen_handle,
                display_list,
                children,
                retry_count: 0,
            })
        } else if let Some(clip) = scene.children_clip(id, frame) {
            // clip 由 children_clip 基于 frame（绝对坐标）计算，直接使用
            let adj = Rect::new(clip.x, clip.y, clip.w, clip.h);
            let children = Self::build_children_cached(scene, id, cache);
            Some(LayerNode::ClipRect {
                widget_id: id,
                rect: adj,
                children,
            })
        } else {
            let children = Self::build_children_cached(scene, id, cache);
            Some(LayerNode::Direct {
                widget_id: id,
                children,
            })
        }
    }

    /// 带缓存复用的子节点构建，按 z_index 预排序（#97：排序缓存）。
    fn build_children_cached(
        scene: &impl ScenePaint,
        id: NodeId,
        cache: &std::collections::HashMap<
            NodeId,
            (Rect, Option<ImageHandle>, Option<DisplayList>),
        >,
    ) -> Vec<LayerNode> {
        let mut children: Vec<LayerNode> = scene
            .node_children(id)
            .iter()
            .copied()
            .filter_map(|cid| Self::build_node_cached(scene, cid, cache))
            .collect();
        // 预排序：render 时无需再排序
        children.sort_by_key(|child| scene.node_z_index(child.widget_id()));
        children
    }

    /// 递归更新脏状态。返回 true 表示该节点或其子树有脏节点。
    ///
    /// 关键语义：子节点脏 → 父 Picture 必须重新栅格化。
    /// 这是离屏缓存一致性的核心保证——没有这个传播，
    /// Picture 子树的动画变化会被缓存的旧内容覆盖，产生视觉残留。
    fn update_dirty_node(node: &mut LayerNode, scene: &impl ScenePaint) -> bool {
        match node {
            LayerNode::Picture {
                widget_id,
                is_dirty,
                children,
                ..
            } => {
                // Picture 自身脏标记 + 子节点传播的脏标记
                let self_dirty = scene.node_dirty(*widget_id);
                let child_dirty = children
                    .iter_mut()
                    .any(|child| Self::update_dirty_node(child, scene));
                *is_dirty = self_dirty || child_dirty;
                *is_dirty
            }
            LayerNode::ClipRect {
                widget_id,
                children,
                ..
            }
            | LayerNode::Direct {
                widget_id,
                children,
                ..
            } => {
                // 检查自身 dirty + 子节点传播的脏标记
                let self_dirty = scene.node_dirty(*widget_id);
                let child_dirty = children
                    .iter_mut()
                    .any(|child| Self::update_dirty_node(child, scene));
                self_dirty || child_dirty
            }
        }
    }

    /// 递归渲染单个节点；脏剪枝使用 viewport 坐标变换（Phase 5）。
    fn render_node(
        node: &mut LayerNode,
        engine: &mut dyn GraphicsEngine,
        scene: &impl ScenePaint,
        dirty_region: &DirtyRegion,
        env: &LayerRenderEnv<'_>,
        surface_w: i32,
        surface_h: i32,
        debug_mode: bool,
        hovered_chain: &Option<HashSet<NodeId>>,
        depth: usize,
        mut render_objects: Option<&mut RenderObjectTree>,
    ) {
        match node {
            LayerNode::Picture {
                widget_id,
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
                    return;
                }

                let needs_blit =
                    *is_dirty || needs_paint_rect(scene, *widget_id, *bounds, dirty_region);

                if *is_dirty || offscreen_handle.is_none() {
                    rasterize_picture_to_offscreen(
                        engine,
                        *widget_id,
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
                    );
                }

                if needs_blit {
                    if let Some(handle) = offscreen_handle.as_ref() {
                        blit_picture_cache(engine, handle, bounds, w, h);
                    }
                }
            }
            LayerNode::ClipRect {
                widget_id,
                rect,
                children,
            } => {
                if needs_paint(scene, *widget_id, dirty_region) {
                    let mut ctx = Self::paint_context(engine, env, surface_w, surface_h);
                    Self::paint_widget(
                        *widget_id,
                        &mut ctx,
                        scene,
                        PaintPass::Content,
                        render_objects.as_deref_mut(),
                    );
                    Self::draw_debug_for_widget(
                        &mut ctx,
                        scene,
                        *widget_id,
                        debug_mode,
                        hovered_chain,
                        depth,
                    );
                }
                engine.canvas_2d().push_clip(*rect);
                if let Some((sx, sy)) = Self::get_scroll_offset(scene, *widget_id) {
                    engine.canvas_2d().translate(-sx, -sy);
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
                        hovered_chain,
                        depth + 1,
                        render_objects.as_deref_mut(),
                    );
                }
                if let Some((sx, sy)) = Self::get_scroll_offset(scene, *widget_id) {
                    engine.canvas_2d().translate(sx, sy);
                }
                engine.canvas_2d().pop_clip();
                if needs_paint(scene, *widget_id, dirty_region) {
                    let mut ctx = Self::paint_context(engine, env, surface_w, surface_h);
                    Self::paint_widget(
                        *widget_id,
                        &mut ctx,
                        scene,
                        PaintPass::AfterChildren,
                        None,
                    );
                }
            }
            LayerNode::Direct {
                widget_id,
                children,
            } => {
                Self::render_widget_and_children(
                    engine,
                    *widget_id,
                    children,
                    scene,
                    dirty_region,
                    env,
                    surface_w,
                    surface_h,
                    debug_mode,
                    hovered_chain,
                    depth,
                    render_objects,
                );
            }
        }
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

    fn draw_debug_for_widget(
        ctx: &mut PaintContext<'_>,
        scene: &impl ScenePaint,
        widget_id: NodeId,
        debug_mode: bool,
        hovered_chain: &Option<HashSet<NodeId>>,
        depth: usize,
    ) {
        if !debug_mode || !scene.node_visible(widget_id) {
            return;
        }
        let frame = scene.node_frame(widget_id);
        let hovered = hovered_chain
            .as_ref()
            .is_some_and(|c| c.contains(&widget_id));
        ctx.draw_debug_border(frame, depth, hovered);
        if hovered {
            ctx.draw_debug_frame_info(widget_id, frame);
        }
    }

    /// 仅渲染 widget 自身的视觉效果（不处理子节点）。
    pub(crate) fn render_widget_self(id: NodeId, ctx: &mut PaintContext<'_>, scene: &impl ScenePaint) {
        Self::paint_widget(id, ctx, scene, PaintPass::Content, None);
    }

    /// 渲染 widget 自身及其子节点（直接遍历 children LayerNodes）。
    fn render_widget_and_children(
        engine: &mut dyn GraphicsEngine,
        id: NodeId,
        children: &mut [LayerNode],
        scene: &impl ScenePaint,
        dirty_region: &DirtyRegion,
        env: &LayerRenderEnv<'_>,
        surface_w: i32,
        surface_h: i32,
        debug_mode: bool,
        hovered_chain: &Option<HashSet<NodeId>>,
        depth: usize,
        mut render_objects: Option<&mut RenderObjectTree>,
    ) {
        if !scene.node_visible(id) {
            return;
        }
        if needs_paint(scene, id, dirty_region) {
            let mut ctx = Self::paint_context(engine, env, surface_w, surface_h);
            Self::paint_widget(
                id,
                &mut ctx,
                scene,
                PaintPass::Content,
                render_objects.as_deref_mut(),
            );
            Self::draw_debug_for_widget(&mut ctx, scene, id, debug_mode, hovered_chain, depth);
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
                    hovered_chain,
                    depth + 1,
                    render_objects.as_deref_mut(),
                );
            }
        }
    }

    /// 获取滚动容器的 content 偏移（viewport → content）。
    pub(crate) fn get_scroll_offset(scene: &impl ScenePaint, widget_id: NodeId) -> Option<(f32, f32)> {
        scene.scroll_offset(widget_id)
    }
}

impl Default for LayerTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl LayerTree {
    pub fn root_node(&self) -> Option<&LayerNode> {
        self.root.as_ref()
    }

    pub fn orphaned_handles(&self) -> &[ImageHandle] {
        &self.orphaned_handles
    }

    pub fn orphaned_handles_mut(&mut self) -> &mut Vec<ImageHandle> {
        &mut self.orphaned_handles
    }
}
