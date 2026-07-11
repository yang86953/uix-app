//! LayerTree — 合成树（Phase 3 迁入 draw）。
//!
//! 通过 ScenePaint trait 读取场景结构并下发绘制，不依赖 ui 域。

use std::collections::HashSet;

use crate::core::{Point, Rect};

use crate::core::DirtyRegion;
use crate::draw::compositor::picture::{
    blit_picture_cache, rasterize_picture_to_offscreen, LayerRenderEnv,
};
use crate::draw::compositor::viewport_transform::{needs_paint, needs_paint_rect};
use crate::draw::compositor::{PicturePolicy, ScenePaint};
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::draw::painting::{DisplayList, PaintContext, PaintPass, ThemeSnapshot};
use crate::draw::pipeline::NodeId;
use crate::draw::primitives::types::ImageHandle;
use crate::draw::render_object::RenderObjectTree;
use crate::draw::traits::GraphicsEngine;
use crate::draw::FontHandle;

/// Debug overlay：当前指针下的祖先链 + 最深命中节点。
struct DebugHover {
    chain: HashSet<NodeId>,
    leaf: NodeId,
}

/// 图层节点。
pub enum LayerNode {
    /// 图片图层：缓存被 ScenePaint 边界选中的子树栅格结果。
    /// 包含 children 以支持嵌套 Picture 缓存层（#82、#86、#87）。
    Picture {
        node_id: NodeId,
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
        node_id: NodeId,
        rect: Rect,
        children: Vec<LayerNode>,
    },
    /// 普通节点：直接渲染 widget 及其子树（无特殊图层语义，无离屏缓存）。
    Direct {
        node_id: NodeId,
        children: Vec<LayerNode>,
    },
}

impl std::fmt::Debug for LayerNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerNode::Picture {
                node_id,
                bounds,
                is_dirty,
                offscreen_handle,
                display_list,
                children,
                retry_count,
            } => f
                .debug_struct("PictureLayer")
                .field("node_id", node_id)
                .field("bounds", bounds)
                .field("is_dirty", is_dirty)
                .field("has_offscreen", &offscreen_handle.is_some())
                .field("has_display_list", &display_list.is_some())
                .field("children_count", &children.len())
                .field("retry_count", retry_count)
                .finish(),
            LayerNode::ClipRect {
                node_id,
                rect,
                children,
            } => f
                .debug_struct("ClipRectLayer")
                .field("node_id", node_id)
                .field("rect", rect)
                .field("children_count", &children.len())
                .finish(),
            LayerNode::Direct { node_id, children } => f
                .debug_struct("DirectLayer")
                .field("node_id", node_id)
                .field("children_count", &children.len())
                .finish(),
        }
    }
}

impl LayerNode {
    fn mark_cache_dirty(&mut self) {
        match self {
            LayerNode::Picture {
                is_dirty, children, ..
            } => {
                *is_dirty = true;
                for child in children.iter_mut() {
                    child.mark_cache_dirty();
                }
            }
            LayerNode::ClipRect { children, .. } | LayerNode::Direct { children, .. } => {
                for child in children.iter_mut() {
                    child.mark_cache_dirty();
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

    /// 获取该节点对应的 node_id。
    fn node_id(&self) -> NodeId {
        match self {
            LayerNode::Picture { node_id, .. }
            | LayerNode::ClipRect { node_id, .. }
            | LayerNode::Direct { node_id, .. } => *node_id,
        }
    }
}

/// 图层树 —— 从 ScenePaint 构建的可缓存合成栈。
///
/// 子节点在 build 时按 z_index 预排序，渲染时直接遍历无需额外排序。
pub struct LayerTree {
    root: Option<LayerNode>,
    overlays: Vec<LayerNode>,
    /// build 后未复用的旧离屏句柄（等待 sweep 释放）。
    orphaned_handles: Vec<ImageHandle>,
}

const PICTURE_CACHE_MIN_NODES: usize = 8;
const PICTURE_CACHE_MIN_PIXELS: f32 = 65_536.0;

#[derive(Debug, Clone, Copy)]
struct PictureSubtreeStats {
    node_count: usize,
    estimated_pixels: f32,
    cacheable: bool,
}

impl PictureSubtreeStats {
    fn eligible(self) -> bool {
        self.cacheable
            && self.node_count >= PICTURE_CACHE_MIN_NODES
            && self.estimated_pixels >= PICTURE_CACHE_MIN_PIXELS
    }
}

impl std::fmt::Debug for LayerTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayerTree")
            .field("has_root", &self.root.is_some())
            .field("overlay_count", &self.overlays.len())
            .finish()
    }
}

impl LayerTree {
    pub fn new() -> Self {
        Self {
            root: None,
            overlays: Vec::new(),
            orphaned_handles: Vec::new(),
        }
    }

    /// 从 ScenePaint 构建图层树。
    /// 生成所有可见 widget 的 LayerNode（无遗漏）。
    /// 自动复用已缓存离屏缓冲——按 node_id 匹配旧 Picture 节点。
    /// 子节点按 z_index 预排序，渲染时无需再排序（#97）。
    ///
    /// `supports_offscreen`：引擎无离屏能力时不提升为 Picture（避免每帧 create 失败刷 WARN）。
    ///
    /// 注意：build 后未复用的旧离屏缓冲句柄暂存在 `orphaned_handles` 中，
    /// 调用者需在合适的时机调用 `sweep_orphaned_offscreens` 释放。
    pub fn build(&mut self, scene: &impl ScenePaint, supports_offscreen: bool) {
        // 收集旧 Picture 节点的离屏缓冲与 DisplayList（按 node_id 索引）
        let mut old_cache: std::collections::HashMap<
            NodeId,
            (Rect, Option<ImageHandle>, Option<DisplayList>),
        > = std::collections::HashMap::new();
        if let Some(ref root) = self.root {
            Self::collect_picture_handles(root, &mut old_cache);
        }
        for overlay in &self.overlays {
            Self::collect_picture_handles(overlay, &mut old_cache);
        }

        let mut overlay_ids = Vec::new();
        if let Some(root_id) = scene.root_id() {
            Self::collect_overlay_node_ids(scene, root_id, &mut overlay_ids);
        }

        self.root = scene
            .root_id()
            .filter(|id| !scene.node_is_overlay(*id))
            .and_then(|root_id| {
                Self::build_node_cached(scene, root_id, 0, &old_cache, supports_offscreen)
            });
        self.overlays = overlay_ids
            .into_iter()
            .filter_map(|id| {
                Self::build_overlay_node_cached(scene, id, 0, &old_cache, supports_offscreen)
            })
            .collect();
        self.overlays
            .sort_by_key(|overlay| scene.node_z_index(overlay.node_id()));

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
        for overlay in &mut self.overlays {
            Self::update_dirty_node(overlay, scene);
        }
    }

    /// 单 Pass 渲染：按 z-order 合成 widget 绘制与调试覆盖（Phase 7）。
    ///
    /// 不绘制全局焦点环（#177）；交互反馈由控件自身（如 Button Material ripple）负责。
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
        image_service: &ImageService,
        debug_mode: bool,
        hover_pos: Option<Point>,
        mut render_objects: Option<&mut RenderObjectTree>,
    ) {
        let dpi = engine.dpi();
        let dpr = engine.device_pixel_ratio();
        let orientation = engine.orientation();
        let surface_w = engine.canvas_2d().width();
        let surface_h = engine.canvas_2d().height();
        let env = LayerRenderEnv {
            font,
            font_service,
            image_service,
            tokens: theme.tokens(),
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

        if let Some(ref mut root) = self.root {
            // The normal tree may establish viewport clips or scroll translations.
            // Root-level overlays must start from the frame's original canvas state,
            // even if a backend retains state after the normal-tree traversal.
            engine.canvas_2d().save();
            Self::render_node(
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
            root.mark_clean();
        }
        for overlay in &mut self.overlays {
            // Isolate sibling overlays too: one overlay cannot clip or translate
            // the next one, and neither can inherit normal-tree state.
            engine.canvas_2d().save();
            Self::render_node(
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
            overlay.mark_clean();
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
            env.image_service,
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
            root.mark_cache_dirty();
        }
        for overlay in &mut self.overlays {
            overlay.mark_cache_dirty();
        }
    }

    pub fn is_ready(&self) -> bool {
        self.root.is_some() || !self.overlays.is_empty()
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
                node_id,
                bounds,
                offscreen_handle,
                display_list,
                ..
            } => {
                cache.insert(*node_id, (*bounds, *offscreen_handle, display_list.clone()));
            }
            LayerNode::ClipRect { children, .. } | LayerNode::Direct { children, .. } => {
                for child in children {
                    Self::collect_picture_handles(child, cache);
                }
            }
        }
    }

    fn collect_overlay_node_ids(scene: &impl ScenePaint, id: NodeId, ids: &mut Vec<NodeId>) {
        if !scene.node_visible(id) {
            return;
        }
        if scene.node_is_overlay(id) {
            ids.push(id);
        }
        for child in scene.node_children(id) {
            Self::collect_overlay_node_ids(scene, *child, ids);
        }
    }

    /// 带缓存复用的 build_node。
    /// 如果旧缓存中有相同 node_id 且 bounds 未变的 Picture，复用其离屏句柄。
    ///
    /// 注意：`frame` 是绝对坐标（由 layout 阶段设置），以下所有位置计算直接使用 `frame` 的坐标，
    /// 不再累加父级偏移。
    fn build_node_cached(
        scene: &impl ScenePaint,
        id: NodeId,
        depth: usize,
        cache: &std::collections::HashMap<NodeId, (Rect, Option<ImageHandle>, Option<DisplayList>)>,
        supports_offscreen: bool,
    ) -> Option<LayerNode> {
        if !scene.node_visible(id) {
            return None;
        }
        let frame = scene.node_frame(id);

        let stats = Self::picture_subtree_stats(scene, id);

        if supports_offscreen && stats.eligible() {
            // frame 是绝对坐标，直接用作 bounds
            let bounds = frame;
            // 检查旧缓存：如果 bounds 相同，复用离屏句柄
            let (offscreen_handle, display_list) = cache
                .get(&id)
                .map(|(old_bounds, old_handle, old_list)| {
                    if *old_bounds == bounds {
                        (*old_handle, old_list.clone())
                    } else {
                        (None, None)
                    }
                })
                .unwrap_or((None, None));
            let children = Self::build_children_cached(scene, id, depth, cache, supports_offscreen);
            Some(LayerNode::Picture {
                node_id: id,
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
            let children = Self::build_children_cached(scene, id, depth, cache, supports_offscreen);
            Some(LayerNode::ClipRect {
                node_id: id,
                rect: adj,
                children,
            })
        } else {
            let children = Self::build_children_cached(scene, id, depth, cache, supports_offscreen);
            Some(LayerNode::Direct {
                node_id: id,
                children,
            })
        }
    }

    fn build_overlay_node_cached(
        scene: &impl ScenePaint,
        id: NodeId,
        depth: usize,
        cache: &std::collections::HashMap<NodeId, (Rect, Option<ImageHandle>, Option<DisplayList>)>,
        supports_offscreen: bool,
    ) -> Option<LayerNode> {
        if !scene.node_visible(id) {
            return None;
        }
        Some(LayerNode::Direct {
            node_id: id,
            children: Self::build_children_cached(scene, id, depth, cache, supports_offscreen),
        })
    }

    fn picture_subtree_stats(scene: &impl ScenePaint, id: NodeId) -> PictureSubtreeStats {
        let frame = scene.node_frame(id);
        let mut stats = PictureSubtreeStats {
            node_count: 1,
            estimated_pixels: (frame.w.max(0.0) * frame.h.max(0.0)).max(0.0),
            cacheable: Self::node_allows_picture(scene, id, frame),
        };

        for child in scene.node_children(id) {
            if scene.node_is_overlay(*child) {
                continue;
            }
            let child_stats = Self::picture_subtree_stats(scene, *child);
            stats.node_count += child_stats.node_count;
            stats.estimated_pixels += child_stats.estimated_pixels;
            stats.cacheable &= child_stats.cacheable;
        }

        stats
    }

    fn node_allows_picture(scene: &impl ScenePaint, id: NodeId, frame: Rect) -> bool {
        scene.node_picture_policy(id) == PicturePolicy::Eligible
            && !scene.node_has_semantic_handlers(id)
            && !scene.node_has_dynamic_content(id)
            && !scene.node_has_interactive_state(id)
            && !scene.node_wants_continuous_pointer_move(id)
            && !scene.node_is_overlay(id)
            && !scene.node_focusable(id)
            && scene.children_clip(id, frame).is_none()
            && scene.scroll_offset(id).is_none()
    }

    /// 带缓存复用的子节点构建，按 z_index 预排序（#97：排序缓存）。
    fn build_children_cached(
        scene: &impl ScenePaint,
        id: NodeId,
        depth: usize,
        cache: &std::collections::HashMap<NodeId, (Rect, Option<ImageHandle>, Option<DisplayList>)>,
        supports_offscreen: bool,
    ) -> Vec<LayerNode> {
        let mut children: Vec<LayerNode> = scene
            .node_children(id)
            .iter()
            .copied()
            .filter(|cid| !scene.node_is_overlay(*cid))
            .filter_map(|cid| {
                Self::build_node_cached(scene, cid, depth + 1, cache, supports_offscreen)
            })
            .collect();
        // 预排序：render 时无需再排序
        children.sort_by_key(|child| scene.node_z_index(child.node_id()));
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
                node_id,
                bounds,
                is_dirty,
                children,
                ..
            } => {
                // 窗口放大后若未 rebuild，仍须刷新 bounds，否则离屏/blit 卡在旧几何。
                let frame = scene.node_frame(*node_id);
                if *bounds != frame {
                    *bounds = frame;
                    *is_dirty = true;
                }
                // Picture 自身脏标记 + 子节点传播的脏标记
                let self_dirty = scene.node_dirty(*node_id);
                let child_dirty = children
                    .iter_mut()
                    .any(|child| Self::update_dirty_node(child, scene));
                *is_dirty = *is_dirty || self_dirty || child_dirty;
                *is_dirty
            }
            LayerNode::ClipRect {
                node_id,
                rect,
                children,
            } => {
                let frame = scene.node_frame(*node_id);
                if let Some(clip) = scene.children_clip(*node_id, frame) {
                    if *rect != clip {
                        *rect = clip;
                    }
                }
                let self_dirty = scene.node_dirty(*node_id);
                let child_dirty = children
                    .iter_mut()
                    .any(|child| Self::update_dirty_node(child, scene));
                self_dirty || child_dirty
            }
            LayerNode::Direct {
                node_id, children, ..
            } => {
                // 检查自身 dirty + 子节点传播的脏标记
                let self_dirty = scene.node_dirty(*node_id);
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
        debug_hover: &Option<DebugHover>,
        depth: usize,
        mut render_objects: Option<&mut RenderObjectTree>,
    ) {
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
                    return;
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
                    );
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
                    );
                    return;
                }

                if needs_blit {
                    if let Some(handle) = offscreen_handle.as_ref() {
                        blit_picture_cache(engine, handle, bounds, w, h);
                    }
                }
            }
            LayerNode::ClipRect {
                node_id,
                rect,
                children,
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
                        debug_hover,
                        depth + 1,
                        render_objects.as_deref_mut(),
                    );
                }
                if let Some((sx, sy)) = Self::get_scroll_offset(scene, *node_id) {
                    engine.canvas_2d().translate(sx, sy);
                }
                engine.canvas_2d().pop_clip();
                if Self::should_paint_node(scene, *node_id, dirty_region) {
                    let mut ctx = Self::paint_context(engine, env, surface_w, surface_h);
                    Self::paint_widget(*node_id, &mut ctx, scene, PaintPass::AfterChildren, None);
                }
            }
            LayerNode::Direct { node_id, children } => {
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
        engine: &mut dyn GraphicsEngine,
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
    ) {
        if !scene.node_visible(id) {
            return;
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
                );
            }
        }
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

    pub fn overlay_nodes(&self) -> &[LayerNode] {
        &self.overlays
    }

    pub fn orphaned_handles(&self) -> &[ImageHandle] {
        &self.orphaned_handles
    }

    pub fn orphaned_handles_mut(&mut self) -> &mut Vec<ImageHandle> {
        &mut self.orphaned_handles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestNode {
        id: NodeId,
        frame: Rect,
        children: Vec<NodeId>,
        policy: PicturePolicy,
        has_handler: bool,
        dynamic: bool,
        interactive: bool,
        continuous_pointer: bool,
        overlay: bool,
        focusable: bool,
        clip: bool,
        scroll: bool,
    }

    impl TestNode {
        fn eligible(id: NodeId, children: Vec<NodeId>) -> Self {
            Self {
                id,
                frame: Rect::new(0.0, 0.0, 300.0, 300.0),
                children,
                policy: PicturePolicy::Eligible,
                has_handler: false,
                dynamic: false,
                interactive: false,
                continuous_pointer: false,
                overlay: false,
                focusable: false,
                clip: false,
                scroll: false,
            }
        }
    }

    struct TestScene {
        nodes: Vec<TestNode>,
        dirty_ids: HashSet<NodeId>,
        paint: Option<for<'a> fn(NodeId, &mut PaintContext<'a>)>,
    }

    impl TestScene {
        fn static_tree(node_count: usize) -> Self {
            let children = (2..=node_count).map(NodeId::new).collect();
            let mut nodes = vec![TestNode::eligible(NodeId::new(1), children)];
            for id in 2..=node_count {
                nodes.push(TestNode::eligible(NodeId::new(id), Vec::new()));
            }
            Self {
                nodes,
                dirty_ids: HashSet::new(),
                paint: None,
            }
        }

        fn node_mut(&mut self, id: NodeId) -> &mut TestNode {
            self.nodes.iter_mut().find(|node| node.id == id).unwrap()
        }

        fn node(&self, id: NodeId) -> &TestNode {
            self.nodes.iter().find(|node| node.id == id).unwrap()
        }

        fn build_layer_tree(&self) -> LayerTree {
            let mut tree = LayerTree::new();
            tree.build(self, true);
            tree
        }
    }

    impl ScenePaint for TestScene {
        fn root_id(&self) -> Option<NodeId> {
            Some(NodeId::new(1))
        }

        fn tree_version(&self) -> u64 {
            1
        }

        fn dirty_region(&self) -> DirtyRegion {
            DirtyRegion::empty()
        }

        fn node_visible(&self, id: NodeId) -> bool {
            self.nodes.iter().any(|node| node.id == id)
        }

        fn node_frame(&self, id: NodeId) -> Rect {
            self.node(id).frame
        }

        fn node_dirty(&self, id: NodeId) -> bool {
            self.dirty_ids.contains(&id)
        }

        fn node_z_index(&self, _id: NodeId) -> i32 {
            0
        }

        fn node_children(&self, id: NodeId) -> &[NodeId] {
            &self.node(id).children
        }

        fn node_picture_policy(&self, id: NodeId) -> PicturePolicy {
            self.node(id).policy
        }

        fn node_has_semantic_handlers(&self, id: NodeId) -> bool {
            self.node(id).has_handler
        }

        fn node_has_dynamic_content(&self, id: NodeId) -> bool {
            self.node(id).dynamic
        }

        fn node_has_interactive_state(&self, id: NodeId) -> bool {
            self.node(id).interactive
        }

        fn node_wants_continuous_pointer_move(&self, id: NodeId) -> bool {
            self.node(id).continuous_pointer
        }

        fn node_is_overlay(&self, id: NodeId) -> bool {
            self.node(id).overlay
        }

        fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect> {
            self.node(id).clip.then_some(frame)
        }

        fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
            frame
        }

        fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
            self.node(id).scroll.then_some((1.0, 0.0))
        }

        fn focused_node(&self) -> Option<NodeId> {
            None
        }

        fn node_focusable(&self, id: NodeId) -> bool {
            self.node(id).focusable
        }

        fn hit_test(&self, _pos: Point) -> Option<NodeId> {
            None
        }

        fn parent(&self, _id: NodeId) -> Option<NodeId> {
            None
        }

        fn paint(&self, id: NodeId, _frame: Rect, ctx: &mut PaintContext<'_>) {
            if let Some(paint) = self.paint {
                paint(id, ctx);
            }
        }
    }

    struct TestTokens;

    macro_rules! test_color_tokens {
        ($($name:ident),+ $(,)?) => {
            $(
                fn $name(&self) -> crate::draw::Color {
                    crate::draw::Color::black()
                }
            )+
        };
    }

    impl crate::draw::painting::IColorTokens for TestTokens {
        test_color_tokens!(
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

    impl crate::draw::painting::ITypographyTokens for TestTokens {
        fn font_family(&self) -> &str {
            "sans"
        }
    }

    impl crate::draw::painting::IBoxShadowTokens for TestTokens {
        fn box_shadow(&self) -> crate::draw::painting::ShadowToken {
            crate::draw::painting::ShadowToken::none()
        }

        fn box_shadow_secondary(&self) -> crate::draw::painting::ShadowToken {
            crate::draw::painting::ShadowToken::none()
        }
    }

    impl crate::draw::painting::ISpacingTokens for TestTokens {}
    impl crate::draw::painting::ThemeTokens for TestTokens {}

    #[test]
    fn eligible_subtree_stays_direct_without_offscreen_support() {
        let scene = TestScene::static_tree(8);
        let mut tree = LayerTree::new();
        tree.build(&scene, false);

        assert!(matches!(
            tree.root_node(),
            Some(LayerNode::Direct { node_id, .. }) if *node_id == NodeId::new(1)
        ));
    }

    #[test]
    fn eligible_large_static_subtree_builds_picture_layer() {
        let scene = TestScene::static_tree(8);
        let tree = scene.build_layer_tree();

        assert!(matches!(
            tree.root_node(),
            Some(LayerNode::Picture { node_id, .. }) if *node_id == NodeId::new(1)
        ));
    }

    #[test]
    fn picture_policy_requires_node_count_and_pixel_thresholds() {
        let small_count = TestScene::static_tree(7).build_layer_tree();
        assert!(matches!(
            small_count.root_node(),
            Some(LayerNode::Direct { node_id, .. }) if *node_id == NodeId::new(1)
        ));

        let mut small_pixels = TestScene::static_tree(8);
        for node in &mut small_pixels.nodes {
            node.frame = Rect::new(0.0, 0.0, 10.0, 10.0);
        }
        let small_pixels = small_pixels.build_layer_tree();
        assert!(matches!(
            small_pixels.root_node(),
            Some(LayerNode::Direct { node_id, .. }) if *node_id == NodeId::new(1)
        ));
    }

    #[test]
    fn runtime_signals_force_picture_policy_never_for_subtree() {
        let runtime_signals: [fn(&mut TestNode); 7] = [
            |node: &mut TestNode| node.has_handler = true,
            |node: &mut TestNode| node.dynamic = true,
            |node: &mut TestNode| node.interactive = true,
            |node: &mut TestNode| node.continuous_pointer = true,
            |node: &mut TestNode| node.overlay = true,
            |node: &mut TestNode| node.focusable = true,
            |node: &mut TestNode| node.scroll = true,
        ];

        for mark_runtime_signal in runtime_signals {
            let mut scene = TestScene::static_tree(8);
            mark_runtime_signal(scene.node_mut(NodeId::new(2)));
            let tree = scene.build_layer_tree();

            assert!(matches!(
                tree.root_node(),
                Some(LayerNode::Direct { node_id, .. }) if *node_id == NodeId::new(1)
            ));
        }
    }

    #[test]
    fn clip_nodes_remain_clip_layers_instead_of_picture_layers() {
        let mut scene = TestScene::static_tree(8);
        scene.node_mut(NodeId::new(1)).clip = true;
        let tree = scene.build_layer_tree();

        assert!(matches!(
            tree.root_node(),
            Some(LayerNode::ClipRect { node_id, .. }) if *node_id == NodeId::new(1)
        ));
    }

    #[test]
    fn overlay_children_are_detached_from_clipped_ancestor_layers() {
        let mut scene = TestScene::static_tree(2);
        scene.node_mut(NodeId::new(1)).clip = true;
        scene.node_mut(NodeId::new(2)).overlay = true;
        let tree = scene.build_layer_tree();

        let Some(LayerNode::ClipRect { children, .. }) = tree.root_node() else {
            panic!("root should remain a clip layer");
        };
        assert!(
            children
                .iter()
                .all(|child| child.node_id() != NodeId::new(2)),
            "active overlays must not inherit an ancestor clip layer"
        );
        assert!(matches!(
            tree.overlay_nodes(),
            [LayerNode::Direct { node_id, .. }] if *node_id == NodeId::new(2)
        ));
    }

    /// Simulates a native canvas whose viewport `pop_clip` leaves its scissor in
    /// effect. LayerTree must restore the root canvas state before painting a
    /// detached overlay, otherwise a full-window overlay is cut to the normal
    /// tree's ScrollView viewport.
    struct LeakyClipCanvas {
        pixels: Vec<u32>,
        clip: Rect,
        clip_stack: Vec<Rect>,
        state_stack: Vec<Rect>,
    }

    impl LeakyClipCanvas {
        fn new(width: i32, height: i32) -> Self {
            Self {
                pixels: vec![0; (width * height) as usize],
                clip: Rect::new(0.0, 0.0, width as f32, height as f32),
                clip_stack: Vec::new(),
                state_stack: Vec::new(),
            }
        }
    }

    impl crate::draw::traits::Canvas2D for LeakyClipCanvas {
        fn save(&mut self) {
            self.state_stack.push(self.clip);
        }

        fn restore(&mut self) {
            self.clip = self
                .state_stack
                .pop()
                .expect("restore must follow a matching save");
        }

        fn push_clip(&mut self, rect: Rect) {
            self.clip_stack.push(self.clip);
            self.clip = self.clip.intersect(&rect).unwrap_or_else(Rect::zero);
        }

        fn pop_clip(&mut self) {
            // Deliberately reproduce a backend clip-state leak.
        }

        fn set_opacity(&mut self, _: f32) {}

        fn opacity(&self) -> f32 {
            1.0
        }

        fn set_blend_mode(&mut self, _: crate::draw::BlendMode) {}

        fn push_clip_path(&mut self, _: &crate::draw::Path) {}

        fn pixels_mut(&mut self) -> &mut [u32] {
            &mut self.pixels
        }

        fn surface_size(&self) -> crate::core::Size {
            crate::core::Size::new(256.0, 256.0)
        }

        fn current_clip(&self) -> Rect {
            self.clip
        }
    }

    struct LeakyClipEngine {
        canvas: LeakyClipCanvas,
    }

    impl LeakyClipEngine {
        fn new() -> Self {
            Self {
                canvas: LeakyClipCanvas::new(256, 256),
            }
        }
    }

    impl GraphicsEngine for LeakyClipEngine {
        fn initialize(&mut self, _: i32, _: i32) -> Result<(), crate::core::Error> {
            Ok(())
        }

        fn shutdown(&mut self) {}

        fn resize(&mut self, _: i32, _: i32) {}

        fn begin_frame(
            &mut self,
            _: crate::draw::traits::UpdateStrategy,
        ) -> crate::draw::engine::RenderOutcome {
            crate::draw::engine::RenderOutcome::Present(crate::draw::backend::DamageRegion::full())
        }

        fn end_frame(
            &mut self,
            damage: &crate::draw::backend::DamageRegion,
        ) -> crate::draw::engine::RenderOutcome {
            crate::draw::engine::RenderOutcome::Present(damage.clone())
        }

        fn canvas_2d(&mut self) -> &mut dyn crate::draw::traits::Canvas2D {
            &mut self.canvas
        }
    }

    fn paint_overlay_outside_viewport(id: NodeId, ctx: &mut PaintContext<'_>) {
        if id == NodeId::new(2) {
            ctx.fill_rect(
                Rect::new(180.0, 24.0, 32.0, 32.0),
                crate::draw::Color::red(),
                None,
            );
        }
    }

    #[test]
    fn detached_overlay_restores_canvas_state_after_a_clipped_root() {
        use crate::draw::font::font_service::FontService;
        use crate::draw::image::ImageService;
        use crate::draw::painting::ThemeSnapshot;

        let mut scene = TestScene::static_tree(2);
        scene.node_mut(NodeId::new(1)).frame = Rect::new(0.0, 0.0, 100.0, 100.0);
        scene.node_mut(NodeId::new(1)).clip = true;
        scene.node_mut(NodeId::new(2)).frame = Rect::zero();
        scene.node_mut(NodeId::new(2)).overlay = true;
        scene.dirty_ids.insert(NodeId::new(2));
        scene.paint = Some(paint_overlay_outside_viewport);

        let mut tree = scene.build_layer_tree();
        let mut engine = LeakyClipEngine::new();
        let tokens = TestTokens;
        let theme = ThemeSnapshot::new(&tokens);
        let fonts = FontService::new();
        let images = ImageService::new();

        tree.render(
            &mut engine,
            &scene,
            &DirtyRegion::full(),
            &theme,
            FontHandle::default(),
            &fonts,
            &images,
            false,
            None,
            None,
        );

        assert_ne!(
            engine.canvas.pixels[24 * 256 + 180],
            0,
            "detached overlay must paint outside the normal tree's viewport clip"
        );
    }

    #[test]
    fn picture_partial_dirty_rerasterize_repaints_all_direct_children() {
        use crate::draw::font::font_service::FontService;
        use crate::draw::image::ImageService;
        use crate::draw::painting::ThemeSnapshot;
        use crate::draw::SoftwareEngine;
        use std::cell::RefCell;
        use std::collections::HashSet;

        struct PaintCountScene {
            base: TestScene,
            painted: RefCell<HashSet<NodeId>>,
            dirty_ids: HashSet<NodeId>,
            region: DirtyRegion,
        }

        impl ScenePaint for PaintCountScene {
            fn root_id(&self) -> Option<NodeId> {
                self.base.root_id()
            }
            fn tree_version(&self) -> u64 {
                1
            }
            fn dirty_region(&self) -> DirtyRegion {
                self.region.clone()
            }
            fn node_visible(&self, id: NodeId) -> bool {
                self.base.node_visible(id)
            }
            fn node_frame(&self, id: NodeId) -> Rect {
                // 子节点纵向错开，便于构造「仅一项与 dirty 相交」
                match id {
                    id if id == NodeId::new(1) => Rect::new(0.0, 0.0, 220.0, 400.0),
                    id if id.slot() >= 2 => {
                        let i = (id.slot() - 2) as f32;
                        Rect::new(8.0, 40.0 + i * 40.0, 200.0, 36.0)
                    }
                    _ => Rect::zero(),
                }
            }
            fn node_dirty(&self, id: NodeId) -> bool {
                self.dirty_ids.contains(&id)
            }
            fn node_z_index(&self, id: NodeId) -> i32 {
                self.base.node_z_index(id)
            }
            fn node_children(&self, id: NodeId) -> &[NodeId] {
                self.base.node_children(id)
            }
            fn node_picture_policy(&self, id: NodeId) -> PicturePolicy {
                self.base.node_picture_policy(id)
            }
            fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect> {
                self.base.children_clip(id, frame)
            }
            fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
                frame
            }
            fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
                self.base.scroll_offset(id)
            }
            fn focused_node(&self) -> Option<NodeId> {
                None
            }
            fn node_focusable(&self, id: NodeId) -> bool {
                self.base.node_focusable(id)
            }
            fn hit_test(&self, _: Point) -> Option<NodeId> {
                None
            }
            fn parent(&self, id: NodeId) -> Option<NodeId> {
                if id.slot() >= 2 {
                    Some(NodeId::new(1))
                } else {
                    None
                }
            }
            fn paint(&self, id: NodeId, _: Rect, _: &mut PaintContext<'_>) {
                self.painted.borrow_mut().insert(id);
            }
        }

        // 8 节点：根成 Picture，子项各自未达阈值 → Direct（与侧栏 Column+Label 同构）
        let base = TestScene::static_tree(8);
        let mut tree = base.build_layer_tree();
        assert!(matches!(tree.root_node(), Some(LayerNode::Picture { .. })));

        // 仅标脏第 2 项（y≈80），与第 7 项（y≈280）不相交
        let hover_child = NodeId::new(3);
        let far_child = NodeId::new(8);
        let mut region = DirtyRegion::empty();
        region.add_rect(Rect::new(8.0, 80.0, 200.0, 36.0));
        let scene = PaintCountScene {
            base,
            painted: RefCell::new(HashSet::new()),
            dirty_ids: HashSet::from([hover_child]),
            region: region.clone(),
        };

        let mut engine = SoftwareEngine::new();
        engine.initialize(256, 512).expect("init");
        tree.update_dirty(&scene);
        // 最小 ThemeTokens，供 LayerTree.render 使用
        struct Tok;
        impl crate::draw::painting::IColorTokens for Tok {
            fn color_primary(&self) -> crate::draw::Color {
                crate::draw::Color::blue()
            }
            fn color_primary_hover(&self) -> crate::draw::Color {
                crate::draw::Color::blue()
            }
            fn color_primary_active(&self) -> crate::draw::Color {
                crate::draw::Color::blue()
            }
            fn color_primary_bg(&self) -> crate::draw::Color {
                crate::draw::Color::blue()
            }
            fn color_primary_border(&self) -> crate::draw::Color {
                crate::draw::Color::blue()
            }
            fn color_bg_container(&self) -> crate::draw::Color {
                crate::draw::Color::white()
            }
            fn color_bg_elevated(&self) -> crate::draw::Color {
                crate::draw::Color::white()
            }
            fn color_bg_raised(&self) -> crate::draw::Color {
                crate::draw::Color::white()
            }
            fn color_bg_overlay(&self) -> crate::draw::Color {
                crate::draw::Color::white()
            }
            fn color_bg_layout(&self) -> crate::draw::Color {
                crate::draw::Color::white()
            }
            fn color_bg_spotlight(&self) -> crate::draw::Color {
                crate::draw::Color::white()
            }
            fn color_bg_mask(&self) -> crate::draw::Color {
                crate::draw::Color::white()
            }
            fn color_border(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_border_secondary(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_fill(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_fill_secondary(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_fill_tertiary(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_fill_quaternary(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_text(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_text_secondary(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_text_tertiary(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_text_quaternary(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_white(&self) -> crate::draw::Color {
                crate::draw::Color::white()
            }
            fn color_black(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_shadow(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_shadow_secondary(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
            fn color_success(&self) -> crate::draw::Color {
                crate::draw::Color::green()
            }
            fn color_success_bg(&self) -> crate::draw::Color {
                crate::draw::Color::green()
            }
            fn color_success_border(&self) -> crate::draw::Color {
                crate::draw::Color::green()
            }
            fn color_warning(&self) -> crate::draw::Color {
                crate::draw::Color::from_rgb(255, 200, 0)
            }
            fn color_warning_bg(&self) -> crate::draw::Color {
                crate::draw::Color::from_rgb(255, 200, 0)
            }
            fn color_warning_border(&self) -> crate::draw::Color {
                crate::draw::Color::from_rgb(255, 200, 0)
            }
            fn color_error(&self) -> crate::draw::Color {
                crate::draw::Color::red()
            }
            fn color_error_bg(&self) -> crate::draw::Color {
                crate::draw::Color::red()
            }
            fn color_error_border(&self) -> crate::draw::Color {
                crate::draw::Color::red()
            }
            fn color_info(&self) -> crate::draw::Color {
                crate::draw::Color::blue()
            }
            fn color_info_bg(&self) -> crate::draw::Color {
                crate::draw::Color::blue()
            }
            fn color_info_border(&self) -> crate::draw::Color {
                crate::draw::Color::blue()
            }
            fn color_link(&self) -> crate::draw::Color {
                crate::draw::Color::blue()
            }
            fn color_link_hover(&self) -> crate::draw::Color {
                crate::draw::Color::blue()
            }
            fn color_link_active(&self) -> crate::draw::Color {
                crate::draw::Color::blue()
            }
        }
        impl crate::draw::painting::ITypographyTokens for Tok {
            fn font_family(&self) -> &str {
                "sans"
            }
        }
        impl crate::draw::painting::IBoxShadowTokens for Tok {
            fn box_shadow(&self) -> crate::draw::painting::ShadowToken {
                crate::draw::painting::ShadowToken::none()
            }
            fn box_shadow_secondary(&self) -> crate::draw::painting::ShadowToken {
                crate::draw::painting::ShadowToken::none()
            }
        }
        impl crate::draw::painting::ISpacingTokens for Tok {}
        impl crate::draw::painting::ThemeTokens for Tok {}

        let tok = Tok;
        let theme = ThemeSnapshot::new(&tok);
        let fs = FontService::new();
        let img = ImageService::new();

        engine.begin_frame(crate::draw::traits::UpdateStrategy::FullRedraw);
        tree.render(
            &mut engine,
            &scene,
            &region,
            &theme,
            FontHandle::default(),
            &fs,
            &img,
            false,
            None,
            None,
        );
        engine.end_frame(&crate::draw::backend::DamageRegion::full());

        let painted = scene.painted.borrow().clone();
        assert!(painted.contains(&hover_child), "hovered child must repaint");
        assert!(
            painted.contains(&far_child),
            "sibling outside screen dirty must still repaint after Picture clear"
        );
        assert!(
            painted.contains(&NodeId::new(1)),
            "picture root content must repaint"
        );
    }
}
