//! Layer Tree —— 受 Flutter/Chrome/gogpu/ui 启发的合成树。
//!
//! 每个 `RepaintBoundary` widget 对应一个 `PictureLayer`，其子树光栅化
//! 到独立离屏缓冲后缓存。干净时直接 blit 到主缓冲。

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
        rect: Rect,
        children: Vec<LayerNode>,
    },
}

impl std::fmt::Debug for LayerNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerNode::Picture { widget_id, bounds, is_dirty, offscreen_handle } => {
                f.debug_struct("PictureLayer")
                    .field("widget_id", widget_id)
                    .field("bounds", bounds)
                    .field("is_dirty", is_dirty)
                    .field("has_offscreen", &offscreen_handle.is_some())
                    .finish()
            }
            LayerNode::ClipRect { rect, children } => {
                f.debug_struct("ClipRectLayer")
                    .field("rect", rect)
                    .field("children_count", &children.len())
                    .finish()
            }
        }
    }
}

impl LayerNode {
    fn mark_dirty(&mut self) {
        match self {
            LayerNode::Picture { is_dirty, .. } => *is_dirty = true,
            LayerNode::ClipRect { children, .. } => {
                for child in children.iter_mut() { child.mark_dirty(); }
            }
        }
    }
    fn mark_clean(&mut self) {
        match self {
            LayerNode::Picture { is_dirty, .. } => *is_dirty = false,
            LayerNode::ClipRect { children, .. } => {
                for child in children.iter_mut() { child.mark_clean(); }
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
    pub fn new() -> Self { Self { root: None } }

    /// 从 WidgetTree 构建图层树。
    pub fn build(&mut self, tree: &WidgetTree) {
        self.root = tree.root_id().and_then(|root_id| {
            Self::build_node(tree, root_id, Point::zero())
        });
    }

    /// 增量更新脏状态。
    pub fn update_dirty(&mut self, tree: &WidgetTree) {
        if let Some(ref mut root) = self.root {
            Self::update_dirty_node(root, tree);
        }
    }

    /// 渲染图层树。
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

    pub fn invalidate(&mut self) {
        if let Some(ref mut root) = self.root { root.mark_dirty(); }
    }
    pub fn is_ready(&self) -> bool { self.root.is_some() }

    // ── 内部 ──

    fn build_node(tree: &WidgetTree, id: WidgetId, parent_origin: Point) -> Option<LayerNode> {
        let node = tree.get(id)?;
        if !node.visible() { return None; }
        let frame = node.frame();
        let origin = Point::new(parent_origin.x + frame.x, parent_origin.y + frame.y);

        if node.inner().is_repaint_boundary() {
            Some(LayerNode::Picture {
                widget_id: id,
                bounds: Rect::new(origin.x, origin.y, frame.w, frame.h),
                is_dirty: true,
                offscreen_handle: None,
            })
        } else if let Some(clip) = node.inner().children_clip(frame) {
            let adj = Rect::new(
                parent_origin.x + clip.x, parent_origin.y + clip.y,
                clip.w, clip.h,
            );
            let children = Self::build_children(tree, id, origin);
            if children.is_empty() { None }
            else { Some(LayerNode::ClipRect { rect: adj, children }) }
        } else if node.children().is_empty() {
            None
        } else {
            let children = Self::build_children(tree, id, origin);
            if children.is_empty() { None }
            else if children.len() == 1 { Some(children.into_iter().next().unwrap()) }
            else {
                Some(LayerNode::ClipRect {
                    rect: Rect::new(origin.x, origin.y, frame.w, frame.h),
                    children,
                })
            }
        }
    }

    fn build_children(tree: &WidgetTree, id: WidgetId, origin: Point) -> Vec<LayerNode> {
        let node = match tree.get(id) { Some(n) => n, None => return vec![] };
        node.children().iter()
            .filter_map(|&cid| Self::build_node(tree, cid, origin))
            .collect()
    }

    fn update_dirty_node(node: &mut LayerNode, tree: &WidgetTree) {
        match node {
            LayerNode::Picture { widget_id, is_dirty, .. } => {
                *is_dirty = tree.get(*widget_id).map(|n| n.dirty()).unwrap_or(true);
            }
            LayerNode::ClipRect { children, .. } => {
                for child in children.iter_mut() { Self::update_dirty_node(child, tree); }
            }
        }
    }

    fn render_node(node: &mut LayerNode, ctx: &mut RenderContext, tree: &WidgetTree) {
        match node {
            LayerNode::Picture { widget_id, bounds, is_dirty, offscreen_handle } => {
                let w = bounds.w.ceil() as i32;
                let h = bounds.h.ceil() as i32;
                if w <= 0 || h <= 0 { return; }

                if *is_dirty || offscreen_handle.is_none() {
                    Self::render_picture_dirty(*widget_id, bounds, offscreen_handle, ctx, tree, w, h);
                } else if let Some(handle) = offscreen_handle {
                    let src = Rect::new(0.0, 0.0, w as f32, h as f32);
                    let dst = Rect::new(bounds.x, bounds.y, w as f32, h as f32);
                    ctx.engine().draw_image(handle, src, dst);
                }
            }
            LayerNode::ClipRect { rect, children } => {
                let r = *rect;
                ctx.engine().push_clip_rect(r);
                for child in children.iter_mut() {
                    Self::render_node(child, ctx, tree);
                }
                ctx.engine().pop_clip_rect();
            }
        }
    }

    fn render_picture_dirty(
        widget_id: WidgetId,
        _bounds: &Rect,
        offscreen_handle: &mut Option<ImageHandle>,
        ctx: &mut RenderContext,
        tree: &WidgetTree,
        w: i32, h: i32,
    ) {
        let needs_new = offscreen_handle.is_none();
        if needs_new {
            if let Ok(h) = ctx.engine().create_offscreen(w, h) {
                *offscreen_handle = Some(*h);
            } else {
                Self::render_widget_subtree(widget_id, ctx, tree);
                return;
            }
        }
        if let Some(handle) = offscreen_handle.as_mut() {
            ctx.engine().begin_offscreen(handle);
            ctx.engine().push_clip_rect(Rect::new(0.0, 0.0, w as f32, h as f32));
            ctx.engine().fill_rect(
                Rect::new(0.0, 0.0, w as f32, h as f32),
                crate::graphics::Color::from_rgba(0, 0, 0, 0),
                None,
            );
            Self::render_widget_subtree(widget_id, ctx, tree);
            ctx.engine().pop_clip_rect();
            ctx.engine().end_offscreen();
        }
    }

    fn render_widget_subtree(id: WidgetId, ctx: &mut RenderContext, tree: &WidgetTree) {
        if let Some(node) = tree.get(id) {
            if !node.visible() { return; }
            let frame = node.frame();
            ctx.save();
            node.inner().render(frame, ctx, tree);
            let clip = node.inner().children_clip(frame);
            if let Some(rect) = clip { ctx.engine().push_clip_rect(rect); }
            let mut sorted: Vec<WidgetId> = node.children().to_vec();
            sorted.sort_by_key(|&cid| tree.get(cid).map_or(0, |c| c.z_index()));
            for &child_id in &sorted { Self::render_widget_subtree(child_id, ctx, tree); }
            if let Some(_) = clip { ctx.engine().pop_clip_rect(); }
            ctx.restore();
        }
    }
}

impl Default for LayerTree {
    fn default() -> Self { Self::new() }
}
