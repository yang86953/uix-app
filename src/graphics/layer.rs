//! Layer Tree —— 受 Flutter/Chrome/gogpu/ui 启发的合成树。
//!
//! 每个 `RepaintBoundary` widget 对应一个 `PictureLayer`，其子树光栅化
//! 到独立离屏缓冲后缓存。干净时直接 blit 到主缓冲。
//! 非 RepaintBoundary 的 widget 通过 `Direct` 节点直接渲染到父级画布。
//! `ClipRect` 节点用于裁剪子节点内容。
//!
//! # 性能设计
//! - 子节点在 build 时按 z_index 预排序，渲染时零排序开销（#97）
//! - Picture 离屏创建失败时启用 retry_count 退避机制（#97）
//! - Picture 支持 children，实现嵌套 RepaintBoundary（#96）

use crate::base::Rect;
use crate::graphics::engine::GraphicsEngine;
use crate::graphics::types::ImageHandle;
use crate::graphics::FontHandle;
use crate::ui::render_context::RenderContext;
use crate::ui::theme::TokenProvider;
use crate::ui::widget::{WidgetCore, WidgetId, WidgetTree};

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
    pub fn sweep_orphaned_offscreens(&mut self, engine: &mut dyn GraphicsEngine) {
        for handle in self.orphaned_handles.drain(..) {
            engine.destroy_offscreen(&handle);
        }
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
    ) {
        let mut rctx = RenderContext::new(engine, font, tokens);
        if let Some(ref mut root) = self.root {
            Self::render_node(root, &mut rctx, tree);
            root.mark_clean();
        }
    }

    /// 渲染 overlay 层（post_render，绘制在所有内容之上）。
    /// 替代旧的 tree_render.rs 中的 post_render_pass 路径。
    ///
    /// 当 `debug_mode` 为 true 时，额外绘制调试边框、标签和坐标信息。
    pub fn render_overlays(
        &self,
        engine: &mut dyn GraphicsEngine,
        tree: &WidgetTree,
        tokens: &dyn TokenProvider,
        font: FontHandle,
        debug_mode: bool,
    ) {
        let mut rctx = RenderContext::new(engine, font, tokens);
        rctx.set_debug_mode(debug_mode);
        if let Some(ref root) = self.root {
            Self::render_overlay_node(root, &mut rctx, tree, 0);
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
                ..
            } => {
                *is_dirty = tree.get(*widget_id).map(|n| n.dirty()).unwrap_or(true);
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
                        *widget_id,
                        bounds,
                        offscreen_handle,
                        children,
                        ctx,
                        tree,
                        w,
                        h,
                        retry_count,
                    );
                } else if let Some(handle) = offscreen_handle {
                    let src = Rect::new(0.0, 0.0, w as f32, h as f32);
                    let dst = Rect::new(bounds.x, bounds.y, w as f32, h as f32);
                    ctx.engine().draw_image(handle, src, dst);
                }
            }
            LayerNode::ClipRect {
                widget_id,
                rect,
                children,
            } => {
                // 先渲染 widget 自身的视觉效果（如背景），再裁剪子节点
                Self::render_widget_self(*widget_id, ctx, tree);
                let r = *rect;
                ctx.engine().push_clip_rect(r);
                // 子节点已预排序，直接遍历
                for child in children.iter_mut() {
                    Self::render_node(child, ctx, tree);
                }
                ctx.engine().pop_clip_rect();
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
            // Direct 节点只用于无 children_clip 的 widget，不需要 clip children
            // 子节点已预排序，直接遍历
            for child in children.iter_mut() {
                Self::render_node(child, ctx, tree);
            }
            ctx.restore();
        }
    }

    /// 离屏创建失败时，回退到在主缓冲直接渲染 widget 及其子树。
    /// 不使用 LayerNode children，而是通过 WidgetTree 直接遍历。
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
            // 直接遍历 WidgetTree 的子节点（跳过 LayerNode 层级）
            for &child_id in node.children() {
                if let Some(child) = tree.get(child_id) {
                    if child.visible() {
                        let child_frame = child.frame();
                        let abs_frame = Rect::new(
                            frame.x + child_frame.x,
                            frame.y + child_frame.y,
                            child_frame.w,
                            child_frame.h,
                        );
                        ctx.save();
                        child.inner().render(abs_frame, ctx, tree);
                        ctx.restore();
                    }
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
            match ctx.engine().create_offscreen(w, h) {
                Ok(h) => {
                    *offscreen_handle = Some(*h);
                    *retry_count = 0;
                }
                Err(e) => {
                    *retry_count += 1;
                    log::warn!(
                        "[LayerTree] 离屏创建失败 ({}x{}), retry={}, err={:?}, 回退到主缓冲渲染",
                        w,
                        h,
                        *retry_count,
                        e,
                    );
                    Self::render_widget_and_children_direct(widget_id, ctx, tree);
                    return;
                }
            }
        }

        if let Some(handle) = offscreen_handle.as_mut() {
            ctx.engine().save();
            ctx.engine().set_opacity(1.0);
            ctx.engine().reset_transform();

            ctx.engine().begin_offscreen(handle);
            ctx.engine()
                .push_clip_rect(Rect::new(0.0, 0.0, w as f32, h as f32));
            ctx.engine().fill_rect(
                Rect::new(0.0, 0.0, w as f32, h as f32),
                crate::graphics::Color::from_rgba(0, 0, 0, 0),
                None,
            );
            // 使用 LayerNode 子节点递归渲染（支持嵌套 Picture #96）
            Self::render_widget_self(widget_id, ctx, tree);
            for child in children.iter_mut() {
                Self::render_node(child, ctx, tree);
            }
            ctx.engine().pop_clip_rect();
            ctx.engine().end_offscreen();

            ctx.engine().restore();

            // 立即将离屏渲染结果回写到主缓冲
            let src = Rect::new(0.0, 0.0, w as f32, h as f32);
            ctx.engine().draw_image(handle, src, *bounds);
        }
    }

    /// 递归渲染 overlay 层（post_render 回调 + 调试覆盖）。
    fn render_overlay_node(
        node: &LayerNode,
        ctx: &mut RenderContext,
        tree: &WidgetTree,
        depth: usize,
    ) {
        let widget_id = node.widget_id();
        // 调用 widget 的 post_render
        if let Some(widget_node) = tree.get(widget_id) {
            if widget_node.visible() {
                let frame = widget_node.frame();
                ctx.save();
                widget_node.inner().post_render(frame, ctx, tree);
                ctx.restore();

                // 调试模式：绘制边框、标签和坐标信息
                if ctx.debug_mode() {
                    ctx.draw_debug_border(frame, depth);
                    ctx.draw_debug_label(widget_id, depth, frame);
                    ctx.draw_debug_frame_info(widget_id, frame);
                }
            }
        }
        // 递归子节点（深度 + 1）
        match node {
            LayerNode::Picture { children, .. }
            | LayerNode::ClipRect { children, .. }
            | LayerNode::Direct { children, .. } => {
                for child in children {
                    Self::render_overlay_node(child, ctx, tree, depth + 1);
                }
            }
        }
    }
}

impl Default for LayerTree {
    fn default() -> Self {
        Self::new()
    }
}
