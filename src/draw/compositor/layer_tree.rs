//! LayerTree — 合成树（Phase 3 迁入 draw）。
//!
//! 通过 ScenePaint trait 读取场景结构并下发绘制，不依赖 ui 域。

use std::collections::HashSet;

use crate::core::{Point, Rect};

use crate::core::DirtyRegion;
use crate::draw::FontHandle;
use crate::draw::compositor::picture::{
    LayerRenderEnv, blit_picture_cache, rasterize_picture_to_offscreen,
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
    pub(crate) fn node_id(&self) -> NodeId {
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
    pub fn sweep_orphaned_offscreens(
        &mut self,
        engine: &mut dyn GraphicsEngine,
    ) -> Result<(), crate::core::Error> {
        let handles = std::mem::take(&mut self.orphaned_handles);
        for (index, handle) in handles.iter().copied().enumerate() {
            if let Err(error) = engine.try_destroy_offscreen(handle) {
                self.orphaned_handles.extend_from_slice(&handles[index..]);
                return Err(error);
            }
        }
        Ok(())
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
                    )?;
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
                )?;
            }
        }
        Ok(())
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
                crate::draw::perf_probe::add_widget_painted();
                return;
            }
        }
        scene.paint(id, frame, ctx);
        ctx.restore();
        crate::draw::perf_probe::add_widget_painted();
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

impl Default for LayerTree {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerTree {
    pub(crate) fn root_node(&self) -> Option<&LayerNode> {
        self.root.as_ref()
    }

    pub(crate) fn overlay_nodes(&self) -> &[LayerNode] {
        &self.overlays
    }

    pub(crate) fn orphaned_handles(&self) -> &[ImageHandle] {
        &self.orphaned_handles
    }

    pub(crate) fn orphaned_handles_mut(&mut self) -> &mut Vec<ImageHandle> {
        &mut self.orphaned_handles
    }
}

