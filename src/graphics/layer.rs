//! Layer Tree —— 受 Flutter/Chrome/gogpu/ui 启发的合成树。
//!
//! 每个 `RepaintBoundary` widget 对应一个 `PictureLayer`，其子树光栅化
//! 到独立离屏缓冲后缓存。干净时直接 blit 到主缓冲。
//! 非 RepaintBoundary 的 widget 通过 `Direct` 节点直接渲染到父级画布。
//! `ClipRect` 节点用于裁剪子节点内容。

use crate::base::{Point, Rect};
use crate::graphics::engine::GraphicsEngine;
use crate::graphics::types::ImageHandle;
use crate::graphics::FontHandle;
use crate::ui::render_context::RenderContext;
use crate::ui::theme::TokenProvider;
use crate::ui::widget::{WidgetCore, WidgetId, WidgetTree};

/// 图层节点。
pub enum LayerNode {
    /// 图片图层：缓存 RepaintBoundary 子树的栅格结果。
    Picture {
        widget_id: WidgetId,
        bounds: Rect,
        is_dirty: bool,
        offscreen_handle: Option<ImageHandle>,
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
            } => f
                .debug_struct("PictureLayer")
                .field("widget_id", widget_id)
                .field("bounds", bounds)
                .field("is_dirty", is_dirty)
                .field("has_offscreen", &offscreen_handle.is_some())
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
            LayerNode::Picture { is_dirty, .. } => *is_dirty = true,
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
            LayerNode::Picture { is_dirty, .. } => *is_dirty = false,
            LayerNode::ClipRect { children, .. }
            | LayerNode::Direct { children, .. } => {
                for child in children.iter_mut() {
                    child.mark_clean();
                }
            }
        }
    }
}

/// 图层树 —— 从 WidgetTree 构建的可缓存合成栈。
pub struct LayerTree {
    root: Option<LayerNode>,
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
        Self { root: None }
    }

    /// 从 WidgetTree 构建图层树。
    /// 生成所有可见 widget 的 LayerNode（无遗漏）。
    pub fn build(&mut self, tree: &WidgetTree) {
        self.root = tree
            .root_id()
            .and_then(|root_id| Self::build_node(tree, root_id, Point::zero()));
    }

    /// 增量更新脏状态。
    pub fn update_dirty(&mut self, tree: &WidgetTree) {
        if let Some(ref mut root) = self.root {
            Self::update_dirty_node(root, tree);
        }
    }

    /// 渲染图层树。
    /// FontService 通过 engine.font_service() 获取，避免 engine 和 font_service 的借用冲突。
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
    pub fn render_overlays(
        &self,
        engine: &mut dyn GraphicsEngine,
        tree: &WidgetTree,
        tokens: &dyn TokenProvider,
        font: FontHandle,
    ) {
        let mut rctx = RenderContext::new(engine, font, tokens);
        if let Some(ref root) = self.root {
            Self::render_overlay_node(root, &mut rctx, tree);
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

    /// 构建单个 widget 的 LayerNode。
    /// 所有可见 widget 都会生成节点（不跳过任何不可见的 widget）。
    fn build_node(tree: &WidgetTree, id: WidgetId, parent_origin: Point) -> Option<LayerNode> {
        let node = tree.get(id)?;
        if !node.visible() {
            return None;
        }
        let frame = node.frame();
        let origin = Point::new(parent_origin.x + frame.x, parent_origin.y + frame.y);

        if node.inner().is_repaint_boundary() {
            // RepaintBoundary -> 离屏缓存
            Some(LayerNode::Picture {
                widget_id: id,
                bounds: Rect::new(origin.x, origin.y, frame.w, frame.h),
                is_dirty: true,
                offscreen_handle: None,
            })
        } else if let Some(clip) = node.inner().children_clip(frame) {
            // 有子节点裁剪 -> ClipRect
            let adj = Rect::new(
                parent_origin.x + clip.x,
                parent_origin.y + clip.y,
                clip.w,
                clip.h,
            );
            let children = Self::build_children(tree, id, origin);
            Some(LayerNode::ClipRect {
                widget_id: id,
                rect: adj,
                children,
            })
        } else {
            // 其他 widget -> Direct（直接渲染到父级画布）
            let children = Self::build_children(tree, id, origin);
            Some(LayerNode::Direct {
                widget_id: id,
                children,
            })
        }
    }

    fn build_children(tree: &WidgetTree, id: WidgetId, origin: Point) -> Vec<LayerNode> {
        let node = match tree.get(id) {
            Some(n) => n,
            None => return vec![],
        };
        node.children()
            .iter()
            .filter_map(|&cid| Self::build_node(tree, cid, origin))
            .collect()
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

    /// 获取 LayerNode 对应 widget 的 z_index（用于排序）。
    fn layer_node_z_index(node: &LayerNode, tree: &WidgetTree) -> i32 {
        let wid = match node {
            LayerNode::Picture { widget_id, .. }
            | LayerNode::ClipRect { widget_id, .. }
            | LayerNode::Direct { widget_id, .. } => *widget_id,
        };
        tree.get(wid).map(|n| n.z_index()).unwrap_or(0)
    }

    fn render_node(node: &mut LayerNode, ctx: &mut RenderContext, tree: &WidgetTree) {
        match node {
            LayerNode::Picture {
                widget_id,
                bounds,
                is_dirty,
                offscreen_handle,
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
                        ctx,
                        tree,
                        w,
                        h,
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
                // 子节点按 z_index 排序后递归渲染
                children.sort_by_key(|child| Self::layer_node_z_index(child, tree));
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
            // Direct 节点只用于无 children_clip 的 widget，所以这里不需要 clip children
            if !children.is_empty() {
                children.sort_by_key(|child| Self::layer_node_z_index(child, tree));
                for child in children.iter_mut() {
                    Self::render_node(child, ctx, tree);
                }
            }
            ctx.restore();
        }
    }

    fn render_picture_dirty(
        widget_id: WidgetId,
        bounds: &Rect,
        offscreen_handle: &mut Option<ImageHandle>,
        ctx: &mut RenderContext,
        tree: &WidgetTree,
        w: i32,
        h: i32,
    ) {
        let needs_new = offscreen_handle.is_none();
        if needs_new {
            if let Ok(h) = ctx.engine().create_offscreen(w, h) {
                *offscreen_handle = Some(*h);
            } else {
                // 离屏创建失败，直接渲染到主缓冲
                Self::render_widget_and_children_direct(widget_id, ctx, tree);
                return;
            }
        }
        if let Some(handle) = offscreen_handle.as_mut() {
            // 保存主缓冲的引擎状态（opacity、transform、clip 等），
            // 确保离屏渲染从干净的默认状态开始，避免状态泄漏。
            ctx.engine().save();
            ctx.engine().set_opacity(1.0);
            ctx.engine().reset_transform();

            ctx.engine().begin_offscreen(handle);
            // 离屏缓冲重置状态：裁剪到画布、清除为透明
            ctx.engine()
                .push_clip_rect(Rect::new(0.0, 0.0, w as f32, h as f32));
            ctx.engine().fill_rect(
                Rect::new(0.0, 0.0, w as f32, h as f32),
                crate::graphics::Color::from_rgba(0, 0, 0, 0),
                None,
            );
            Self::render_widget_and_children_direct(widget_id, ctx, tree);
            ctx.engine().pop_clip_rect();
            ctx.engine().end_offscreen();

            // 恢复主缓冲的引擎状态
            ctx.engine().restore();

            // 立即将离屏渲染结果回写到主缓冲，避免延迟一帧
            let src = Rect::new(0.0, 0.0, w as f32, h as f32);
            ctx.engine().draw_image(handle, src, *bounds);
        }
    }

    /// 直接渲染 widget 及其全部子孙（不使用 LayerNode，而是遍历 WidgetTree）。
    /// 只在 Picture 节点的离屏渲染中使用。
    fn render_widget_and_children_direct(
        id: WidgetId,
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
            let clip = node.inner().children_clip(frame);
            if let Some(rect) = clip {
                ctx.engine().push_clip_rect(rect);
            }
            let mut sorted: Vec<WidgetId> = node.children().to_vec();
            sorted.sort_by_key(|&cid| tree.get(cid).map_or(0, |c| c.z_index()));
            for &child_id in &sorted {
                Self::render_widget_and_children_direct(child_id, ctx, tree);
            }
            if let Some(_) = clip {
                ctx.engine().pop_clip_rect();
            }
            ctx.restore();
        }
    }

    // ── Overlay 渲染（替代旧的 tree_render.rs post_render_pass 路径）──

    /// 递归渲染 overlay 节点（post_render）。
    /// 替代旧路径中 WidgetTree::post_render_pass。
    fn render_overlay_node(node: &LayerNode, ctx: &mut RenderContext, tree: &WidgetTree) {
        match node {
            LayerNode::Picture { widget_id, .. } => {
                // Picture 内容已在离屏缓冲中缓存，overlay 直接画在主缓冲
                Self::render_widget_post(*widget_id, ctx, tree);
            }
            LayerNode::ClipRect {
                widget_id,
                rect,
                children,
            } => {
                Self::render_widget_post(*widget_id, ctx, tree);
                ctx.engine().push_clip_rect(*rect);
                let mut sorted: Vec<&LayerNode> = children.iter().collect();
                sorted.sort_by_key(|child| Self::layer_node_z_index(child, tree));
                for child in sorted {
                    Self::render_overlay_node(child, ctx, tree);
                }
                ctx.engine().pop_clip_rect();
            }
            LayerNode::Direct {
                widget_id,
                children,
            } => {
                Self::render_widget_post(*widget_id, ctx, tree);
                let mut sorted: Vec<&LayerNode> = children.iter().collect();
                sorted.sort_by_key(|child| Self::layer_node_z_index(child, tree));
                for child in sorted {
                    Self::render_overlay_node(child, ctx, tree);
                }
            }
        }
    }

    /// 仅渲染 widget 的 overlay（post_render）。
    fn render_widget_post(id: WidgetId, ctx: &mut RenderContext, tree: &WidgetTree) {
        if let Some(node) = tree.get(id) {
            if !node.visible() {
                return;
            }
            let frame = node.frame();
            ctx.save();
            node.inner().post_render(frame, ctx, tree);
            ctx.restore();
        }
    }
}

impl Default for LayerTree {
    fn default() -> Self {
        Self::new()
    }
}
