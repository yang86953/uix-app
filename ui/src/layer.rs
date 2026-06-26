//! Layer Tree —— 受 Flutter/Chrome/gogpu/ui 启发的合成树。
//!
//! 职责边界：
//!   LayerTree 位于 UI 层（从 Graphics 层迁入），是 Graphics 与 UI 之间的桥接。
//!   - 读取 WidgetTree 结构 → 构建可缓存的图层树（build）
//!   - 遍历图层树 → 通过 RenderContext 下发渲染指令到 GraphicsEngine
//!   - 管理 Picture 节点的离屏缓冲生命周期
//!
//! 设计说明：
//!   LayerTree 引用 Graphics 层的 GraphicsEngine/ImageHandle，
//!   也引用 UI 层的 WidgetTree/RenderContext/TokenProvider。
//!   这是 UI 层对 Graphics 层的"只读消费"——LayerTree 不修改 Graphics 状态。
//!   未来如需进一步拆分 crate，可将 LayerTree 提取到独立 bridge crate。
//!
//! # 性能设计
//! - 子节点在 build 时按 z_index 预排序，渲染时零排序开销（#97）
//! - Picture 离屏创建失败时启用 retry_count 退避机制（#97）
//! - Picture 支持 children，实现嵌套 RepaintBoundary（#96）

use std::collections::HashSet;
use uix_core::{Point, Rect};
use uix_graphics::DirtyRegion;
use uix_graphics::GraphicsEngine;
use uix_graphics::font_service::FontService;
use uix_graphics::types::ImageHandle;
use uix_graphics::FontHandle;
use crate::render_context::RenderContext;
use crate::theme::TokenProvider;
use crate::widget::{WidgetCore, WidgetId, WidgetTree};

/// 离屏创建失败的最大重试次数（超过后跳过直到下一次 rebuild）。
const MAX_OFFSCREEN_RETRY: u8 = 3;

/// 图层节点。
pub enum LayerNode {
    /// 图片图层：缓存 RepaintBoundary 子树的栅格结果。
    /// 包含 children 以支持嵌套 RepaintBoundary（#96）。
    Picture {
        widget_id: WidgetId,
        bounds: Rect,
        is_dirty: bool,
        offscreen_handle: Option<ImageHandle>,
        children: Vec<LayerNode>,
        /// 离屏创建连续失败计数，rebuild 时重置为 0。
        retry_count: u8,
    },
    /// 裁剪图层：将子图层内容限制在矩形区域内。
    ClipRect {
        widget_id: WidgetId,
        rect: Rect,
        children: Vec<LayerNode>,
    },
    /// 普通节点：直接渲染 widget 及其子树（无特殊图层语义，无离屏缓存）。
    Direct {
        widget_id: WidgetId,
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
                children,
                retry_count,
            } => f
                .debug_struct("PictureLayer")
                .field("widget_id", widget_id)
                .field("bounds", bounds)
                .field("is_dirty", is_dirty)
                .field("has_offscreen", &offscreen_handle.is_some())
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
                is_dirty,
                children,
                ..
            } => {
                *is_dirty = true;
                for child in children.iter_mut() {
                    child.mark_dirty();
                }
            }
            LayerNode::ClipRect { children, .. }
            | LayerNode::Direct { children, .. } => {
                for child in children.iter_mut() {
                    child.mark_dirty();
                }
            }
        }
    }

    fn mark_clean(&mut self) {
        match self {
            LayerNode::Picture {
                is_dirty,
                children,
                ..
            } => {
                *is_dirty = false;
                for child in children.iter_mut() {
                    child.mark_clean();
                }
            }
            LayerNode::ClipRect { children, .. }
            | LayerNode::Direct { children, .. } => {
                for child in children.iter_mut() {
                    child.mark_clean();
                }
            }
        }
    }

    /// 获取该节点对应的 widget_id。
    fn widget_id(&self) -> WidgetId {
        match self {
            LayerNode::Picture { widget_id, .. }
            | LayerNode::ClipRect { widget_id, .. }
            | LayerNode::Direct { widget_id, .. } => *widget_id,
        }
    }

}

/// 图层树 —— 从 WidgetTree 构建的可缓存合成栈。
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

    /// 从 WidgetTree 构建图层树。
    /// 生成所有可见 widget 的 LayerNode（无遗漏）。
    /// 自动复用已缓存离屏缓冲——按 widget_id 匹配旧 Picture 节点。
    /// 子节点按 z_index 预排序，渲染时无需再排序（#97）。
    ///
    /// 注意：build 后未复用的旧离屏缓冲句柄暂存在 `orphaned_handles` 中，
    /// 调用者需在合适的时机调用 `sweep_orphaned_offscreens` 释放。
    pub fn build(&mut self, tree: &WidgetTree) {
        // 收集旧 Picture 节点的离屏缓冲（按 widget_id 索引）
        let mut old_cache: std::collections::HashMap<
            WidgetId,
            (Rect, Option<ImageHandle>),
        > = std::collections::HashMap::new();
        if let Some(ref root) = self.root {
            Self::collect_picture_handles(root, &mut old_cache);
        }

        self.root = tree.root_id().and_then(|root_id| {
            Self::build_node_cached(tree, root_id, &old_cache)
        });

        // 收集未复用的旧句柄（需要在引擎上下文中释放）
        self.orphaned_handles.clear();
        for (_, (_, handle_opt)) in old_cache {
            if let Some(h) = handle_opt {
                self.orphaned_handles.push(h);
            }
        }
    }

    /// 释放 build 后未复用的旧离屏缓冲。
    /// 必须在 build 之后、下一帧渲染之前调用。
    pub fn sweep_orphaned_offscreens(&mut self, _engine: &mut dyn GraphicsEngine) {
        // TODO(v2): offscreen destroy API 在新的 GraphicsEngine trait 中不存在。
        // 需要重新设计离屏缓冲生命周期管理。
        self.orphaned_handles.clear();
    }

    /// 增量更新脏状态。
    pub fn update_dirty(&mut self, tree: &WidgetTree) {
        if let Some(ref mut root) = self.root {
            Self::update_dirty_node(root, tree);
        }
    }

    /// 渲染图层树。
    /// 子节点已在 build 时预排序，渲染时直接遍历。
    pub fn render(
        &mut self,
        engine: &mut dyn GraphicsEngine,
        tree: &WidgetTree,
        tokens: &dyn TokenProvider,
        font: FontHandle,
        font_service: &FontService,
    ) {
        let mut rctx = RenderContext::new(engine.canvas_2d(), font, font_service, tokens);
        if let Some(ref mut root) = self.root {
            Self::render_node(root, &mut rctx, tree);
            root.mark_clean();
        }
    }

    /// 渲染 overlay 层（post_render，绘制在所有内容之上）。
    /// 替代旧的 tree_render.rs 中的 post_render_pass 路径。
    ///
    /// 当 `debug_mode` 为 true 时：
    /// - 所有 widget 绘制极淡彩色边框（alpha=30）
    /// - 光标下的 widget 及其父链绘制高亮边框 + ID/深度标签 + 坐标信息
    /// `hover_pos` 为当前光标位置，用于调试模式的悬浮高亮。
    /// 渲染 overlay 层（post_render 回调 + 调试覆盖）。
    ///
    /// `dirty_region` 用于跳过脏区域之外 widget 的 `post_render` 调用——
    /// 当 widget 的 frame 与 dirty_region 无交集时，其 overlay 内容未变化，
    /// 无需重新绘制。仅跳过 `post_render` 调用，子树递归仍继续，
    /// 确保子树内可能有脏 widget 时不被遗漏。
    /// 调试模式下始终渲染所有 widget 的调试边框。
    pub fn render_overlays(
        &self,
        engine: &mut dyn GraphicsEngine,
        tree: &WidgetTree,
        tokens: &dyn TokenProvider,
        font: FontHandle,
        font_service: &FontService,
        debug_mode: bool,
        hover_pos: Option<Point>,
        dirty_region: &DirtyRegion,
    ) {
        let mut rctx = RenderContext::new(engine.canvas_2d(), font, font_service, tokens);
        rctx.set_debug_mode(debug_mode);

        // 预计算悬浮链：光标所在 widget + 所有父节点
        let hovered_chain: Option<HashSet<WidgetId>> = if debug_mode {
            hover_pos.and_then(|pos| {
                let deepest = tree.hit_test(pos)?;
                let mut chain = HashSet::new();
                let mut current = deepest;
                chain.insert(current);
                while let Some(pid) = tree.get(current).and_then(|n| n.parent()) {
                    chain.insert(pid);
                    current = pid;
                }
                Some(chain)
            })
        } else {
            None
        };

        if let Some(ref root) = self.root {
            Self::render_overlay_node(root, &mut rctx, tree, 0, &hovered_chain, debug_mode, dirty_region);
        }
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
            WidgetId,
            (Rect, Option<ImageHandle>),
        >,
    ) {
        match node {
            LayerNode::Picture {
                widget_id,
                bounds,
                offscreen_handle,
                ..
            } => {
                cache.insert(*widget_id, (*bounds, *offscreen_handle));
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
        tree: &WidgetTree,
        id: WidgetId,
        cache: &std::collections::HashMap<WidgetId, (Rect, Option<ImageHandle>)>,
    ) -> Option<LayerNode> {
        let node = tree.get(id)?;
        if !node.visible() {
            return None;
        }
        let frame = node.frame();

        if node.inner().is_repaint_boundary() {
            // frame 是绝对坐标，直接用作 bounds
            let bounds = frame;
            // 检查旧缓存：如果 bounds 相同，复用离屏句柄
            let offscreen_handle = cache.get(&id).and_then(|(old_bounds, old_handle)| {
                if *old_bounds == bounds {
                    *old_handle
                } else {
                    None
                }
            });
            let children = Self::build_children_cached(tree, id, cache);
            Some(LayerNode::Picture {
                widget_id: id,
                bounds,
                is_dirty: offscreen_handle.is_none() || node.dirty(),
                offscreen_handle,
                children,
                retry_count: 0,
            })
        } else if let Some(clip) = node.inner().children_clip(frame) {
            // clip 由 children_clip 基于 frame（绝对坐标）计算，直接使用
            let adj = Rect::new(clip.x, clip.y, clip.w, clip.h);
            let children = Self::build_children_cached(tree, id, cache);
            Some(LayerNode::ClipRect {
                widget_id: id,
                rect: adj,
                children,
            })
        } else {
            let children = Self::build_children_cached(tree, id, cache);
            Some(LayerNode::Direct {
                widget_id: id,
                children,
            })
        }
    }

    /// 带缓存复用的子节点构建，按 z_index 预排序（#97：排序缓存）。
    fn build_children_cached(
        tree: &WidgetTree,
        id: WidgetId,
        cache: &std::collections::HashMap<WidgetId, (Rect, Option<ImageHandle>)>,
    ) -> Vec<LayerNode> {
        let node = match tree.get(id) {
            Some(n) => n,
            None => return vec![],
        };
        let mut children: Vec<LayerNode> = node
            .children()
            .iter()
            .filter_map(|&cid| Self::build_node_cached(tree, cid, cache))
            .collect();
        // 预排序：render 时无需再排序
        children.sort_by_key(|child| {
            tree.get(child.widget_id())
                .map(|n| n.z_index())
                .unwrap_or(0)
        });
        children
    }

    fn update_dirty_node(node: &mut LayerNode, tree: &WidgetTree) {
        match node {
            LayerNode::Picture {
                widget_id,
                is_dirty,
                children,
                ..
            } => {
                *is_dirty = tree.get(*widget_id).map(|n| n.dirty()).unwrap_or(true);
                // 递归更新子节点脏状态，支持嵌套 RepaintBoundary（#96）
                for child in children.iter_mut() {
                    Self::update_dirty_node(child, tree);
                }
            }
            LayerNode::ClipRect { children, .. } | LayerNode::Direct { children, .. } => {
                for child in children.iter_mut() {
                    Self::update_dirty_node(child, tree);
                }
            }
        }
    }

    /// 递归渲染单个节点。
    /// 子节点已在 build 时预排序，直接遍历无需再次排序。
    fn render_node(node: &mut LayerNode, ctx: &mut RenderContext, tree: &WidgetTree) {
        match node {
            LayerNode::Picture {
                widget_id,
                bounds,
                is_dirty,
                offscreen_handle,
                children,
                retry_count,
            } => {
                let w = bounds.w.ceil() as i32;
                let h = bounds.h.ceil() as i32;
                if w <= 0 || h <= 0 {
                    return;
                }

                if *is_dirty || offscreen_handle.is_none() {
                    Self::render_picture_dirty(
                        *widget_id, bounds, offscreen_handle, children, ctx, tree, w, h, retry_count,
                    );
                } else if let Some(handle) = offscreen_handle {
                    // TODO(v2): blit_image 需要像素数据，待重建 ImageManager
                    // let src = Rect::new(0.0, 0.0, w as f32, h as f32);
                    // let dst = Rect::new(bounds.x, bounds.y, w as f32, h as f32);
                    // ctx.canvas_2d().blit_image(handle, src, dst);
                }
            }
            LayerNode::ClipRect {
                widget_id,
                rect,
                children,
            } => {
                Self::render_widget_self(*widget_id, ctx, tree);
                let r = *rect;
                ctx.canvas_2d().push_clip(r);
                for child in children.iter_mut() {
                    Self::render_node(child, ctx, tree);
                }
                ctx.canvas_2d().pop_clip();
            }
            LayerNode::Direct {
                widget_id,
                children,
            } => {
                Self::render_widget_and_children(*widget_id, children, ctx, tree);
            }
        }
    }

    /// 仅渲染 widget 自身的视觉效果（不处理子节点）。
    fn render_widget_self(id: WidgetId, ctx: &mut RenderContext, tree: &WidgetTree) {
        if let Some(node) = tree.get(id) {
            if !node.visible() {
                return;
            }
            let frame = node.frame();
            ctx.save();
            node.inner().render(frame, ctx, tree);
            ctx.restore();
        }
    }

    /// 渲染 widget 自身及其子节点（直接遍历 children LayerNodes）。
    /// 子节点已预排序，直接遍历无需再次排序。
    /// 注意：先渲染 widget 自身（由其 render() 自行判断内部可见性），
    /// 再通过 inner().visible() 决定是否渲染子节点。这样模态框等组件
    /// 的 render() 可以控制自身绘制，同时阻止子节点在隐藏时渲染。
    fn render_widget_and_children(
        id: WidgetId,
        children: &mut [LayerNode],
        ctx: &mut RenderContext,
        tree: &WidgetTree,
    ) {
        if let Some(node) = tree.get(id) {
            if !node.visible() {
                return;
            }
            let frame = node.frame();
            ctx.save();
            node.inner().render(frame, ctx, tree);
            // 仅在 widget 内部可见时才渲染子节点。
            // 支持 Modal 等组件：内部 visible 为 false 时 render() 已返回，
            // 同时阻止子节点在隐藏位渲染（子节点 frame 可能仍在屏幕内）。
            if node.inner().visible() {
                for child in children.iter_mut() {
                    Self::render_node(child, ctx, tree);
                }
            }
            ctx.restore();
        }
    }

    /// 离屏创建失败时，回退到在主缓冲直接渲染 widget 及其子树。
    /// 不使用 LayerNode children，而是通过 WidgetTree 递归遍历整个子树。
    ///
    /// 注意：widget 的 frame 是绝对坐标（由 layout 阶段设置），
    /// 子节点 frame 也是绝对坐标，因此直接使用子节点的 frame 即可，
    /// 无需也不能累加父级偏移。
    fn render_widget_and_children_direct(
        widget_id: WidgetId,
        ctx: &mut RenderContext,
        tree: &WidgetTree,
    ) {
        if let Some(node) = tree.get(widget_id) {
            if !node.visible() {
                return;
            }
            let frame = node.frame();
            ctx.save();
            node.inner().render(frame, ctx, tree);
            // 仅在 widget 内部可见时才递归渲染子节点
            if node.inner().visible() {
                for &child_id in node.children() {
                    Self::render_widget_and_children_direct(child_id, ctx, tree);
                }
            }
            ctx.restore();
        }
    }

    /// 渲染脏 Picture 节点：离屏光栅化子节点后再 blit 到主缓冲。
    /// #97：离屏创建失败时递增 retry_count，超过阈值后跳过渲染。
    fn render_picture_dirty(
        widget_id: WidgetId,
        bounds: &Rect,
        offscreen_handle: &mut Option<ImageHandle>,
        children: &mut [LayerNode],
        ctx: &mut RenderContext,
        tree: &WidgetTree,
        w: i32,
        h: i32,
        retry_count: &mut u8,
    ) {
        // 尝试创建或复用离屏缓冲
        let needs_new = offscreen_handle.is_none();
        if needs_new {
            if *retry_count >= MAX_OFFSCREEN_RETRY {
                log::warn!(
                    "[LayerTree] 离屏创建连续失败 {} 次 ({}x{}), 跳过渲染",
                    *retry_count,
                    w,
                    h,
                );
                return;
            }
            // TODO(v2): offscreen API 重新设计中。
            // Picture 离屏渲染暂时回退到主缓冲直接渲染。
            Self::render_widget_and_children_direct(widget_id, ctx, tree);
        }

        // TODO(v2): 离屏渲染回写代码待 redesign
    }

    /// 递归渲染 overlay 层（post_render 回调 + 调试覆盖）。
    ///
    /// 当 `debug_mode` 为 false 时，跳过与 `dirty_region` 无交集的 widget
    /// 的 `post_render` 调用，因为其 overlay 内容未发生变化。
    /// 子树始终递归（子节点可能位于脏区域内）。
    fn render_overlay_node(
        node: &LayerNode,
        ctx: &mut RenderContext,
        tree: &WidgetTree,
        depth: usize,
        hovered_chain: &Option<HashSet<WidgetId>>,
        debug_mode: bool,
        dirty_region: &DirtyRegion,
    ) {
        let widget_id = node.widget_id();

        // 判断此 widget 是否需要重新绘制 overlay
        let needs_overlay = if debug_mode {
            // 调试模式下始终绘制边框，不跳过
            true
        } else if let Some(widget_node) = tree.get(widget_id) {
            if widget_node.visible() {
                let frame = widget_node.frame();
                // 如果 frame 与脏区域有交集，说明内容可能变化，需重绘 overlay
                dirty_region.intersects(frame)
            } else {
                false
            }
        } else {
            false
        };

        // 调用 widget 的 post_render（仅当需要时）
        if needs_overlay {
            if let Some(widget_node) = tree.get(widget_id) {
                if widget_node.visible() {
                    let frame = widget_node.frame();
                    ctx.save();
                    widget_node.inner().post_render(frame, ctx, tree);
                    ctx.restore();
                }
            }
        }

        // 调试模式：悬浮 widget 及其父链显示坐标信息，其余仅极淡边框
        if debug_mode {
            if let Some(widget_node) = tree.get(widget_id) {
                if widget_node.visible() {
                    let frame = widget_node.frame();
                    let hovered = hovered_chain.as_ref().map_or(false, |c| c.contains(&widget_id));
                    ctx.draw_debug_border(frame, depth, hovered);
                    if hovered {
                        ctx.draw_debug_frame_info(widget_id, frame);
                    }
                }
            }
        }

        // 递归子节点（深度 + 1）
        match node {
            LayerNode::Picture { children, .. }
            | LayerNode::Direct { children, .. } => {
                for child in children {
                    Self::render_overlay_node(child, ctx, tree, depth + 1, hovered_chain, debug_mode, dirty_region);
                }
            }
            LayerNode::ClipRect { rect, children, .. } => {
                ctx.canvas_2d().push_clip(*rect);
                for child in children {
                    Self::render_overlay_node(child, ctx, tree, depth + 1, hovered_chain, debug_mode, dirty_region);
                }
                ctx.canvas_2d().pop_clip();
            }
        }
    }
}

impl Default for LayerTree {
    fn default() -> Self {
        Self::new()
    }
}
