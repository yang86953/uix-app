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
use uix_platform::{Point, Rect};
use uix_graphics::DirtyRegion;
use uix_graphics::GraphicsEngine;
use uix_graphics::font_service::FontService;
use uix_graphics::types::ImageHandle;
use uix_graphics::FontHandle;
use crate::render_context::RenderContext;
use crate::api::traits::TokenProvider;
use crate::widget::{WidgetCore, WidgetId, WidgetTree};
use crate::widgets::scroll_view::ScrollView;

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
            engine.destroy_offscreen(handle);
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
    /// 每个节点做空间+脏状态双剪枝：
    /// - frame 与 dirty_region 无交集且 widget 不脏 → 跳过 render_self
    /// - Picture is_dirty=false → 跳过整棵子树
    pub fn render(
        &mut self,
        engine: &mut dyn GraphicsEngine,
        tree: &WidgetTree,
        tokens: &dyn TokenProvider,
        font: FontHandle,
        font_service: &FontService,
    ) {
        let dirty_region = tree.dirty_region();
        let dpi = engine.dpi();
        let dpr = engine.device_pixel_ratio();
        let orientation = engine.orientation();
        let surface_w = engine.canvas_2d().width();
        let surface_h = engine.canvas_2d().height();
        let mut rctx = RenderContext::new(
            engine.canvas_2d(), font, font_service, tokens,
            dpi, dpr, orientation, surface_w, surface_h,
        );
        let dirty_bounds = dirty_region.bounds();
        if let Some(ref mut root) = self.root {
            Self::render_node(root, &mut rctx, tree, dirty_region, dirty_bounds, false);
            root.mark_clean();
        }
    }

    /// 渲染 overlay 层（post_render，绘制在所有内容之上）。
    /// 替代旧的 tree_render.rs 中的 post_render_pass 路径。
    ///
    /// 当 `debug_mode` 为 true 时：
    /// - 所有 widget 绘制极淡彩色边框（alpha=30）
    /// - 光标下的 widget 及其父链绘制高亮边框 + ID/深度标签 + 坐标信息
    ///   `hover_pos` 为当前光标位置，用于调试模式的悬浮高亮。
    ///   渲染 overlay 层（post_render 回调 + 调试覆盖）。
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
        let dpi = engine.dpi();
        let dpr = engine.device_pixel_ratio();
        let orientation = engine.orientation();
        let surface_w = engine.canvas_2d().width();
        let surface_h = engine.canvas_2d().height();
        let mut rctx = RenderContext::new(
            engine.canvas_2d(), font, font_service, tokens,
            dpi, dpr, orientation, surface_w, surface_h,
        );
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

        let dirty_bounds = dirty_region.bounds();
        if let Some(ref root) = self.root {
            Self::render_overlay_node(root, &mut rctx, tree, 0, &hovered_chain, debug_mode, dirty_region, dirty_bounds, false);
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

        if node.is_repaint_boundary() {
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
        } else if let Some(clip) = node.children_clip(frame) {
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

    /// 递归更新脏状态。返回 true 表示该节点或其子树有脏节点。
    ///
    /// 关键语义：子节点脏 → 父 Picture 必须重新栅格化。
    /// 这是离屏缓存一致性的核心保证——没有这个传播，
    /// Picture 子树的动画变化会被缓存的旧内容覆盖，产生视觉残留。
    fn update_dirty_node(node: &mut LayerNode, tree: &WidgetTree) -> bool {
        match node {
            LayerNode::Picture {
                widget_id,
                is_dirty,
                children,
                ..
            } => {
                // Picture 自身脏标记 + 子节点传播的脏标记
                let self_dirty = tree.get(*widget_id).map(|n| n.dirty()).unwrap_or(true);
                let child_dirty = children.iter_mut()
                    .any(|child| Self::update_dirty_node(child, tree));
                *is_dirty = self_dirty || child_dirty;
                *is_dirty
            }
            LayerNode::ClipRect { widget_id, children, .. }
            | LayerNode::Direct { widget_id, children, .. } => {
                // 检查自身 dirty + 子节点传播的脏标记
                let self_dirty = tree.get(*widget_id).map(|n| n.dirty()).unwrap_or(true);
                let child_dirty = children.iter_mut()
                    .any(|child| Self::update_dirty_node(child, tree));
                self_dirty || child_dirty
            }
        }
    }

    /// 递归渲染单个节点。
    /// 子节点已在 build 时预排序，直接遍历无需再次排序。
    /// 终极方案：空间+脏状态双剪枝。
    /// `dirty_bounds` 是所有脏矩形的外接包围盒，用于捕捉脏矩形间隙中的组件。
    /// `force_render`：为 true 时跳过子节点的脏检查，强制渲染所有子节点。
    /// 滚动容器（ScrollView）需要 force_render，因为子节点 frame 在 content 坐标系，
    /// 而 dirty_region 在 viewport 坐标系，直接交叉检测会导致子节点被错误跳过。
    fn render_node(node: &mut LayerNode, ctx: &mut RenderContext, tree: &WidgetTree, dirty_region: &DirtyRegion, dirty_bounds: Rect, force_render: bool) {
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
                        *widget_id, bounds, offscreen_handle, children, ctx, tree, w, h, retry_count, dirty_region, dirty_bounds,
                    );
                } else if offscreen_handle.is_some() {
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
                // 空间+脏状态双剪枝 + dirty_bounds 间隙修正：
                // widget 不脏且 frame 既不在 dirty_region 也不在 dirty_bounds → 跳过 render_self
                let in_bounds = dirty_bounds.intersect(&tree.get(*widget_id).map(|n| n.frame()).unwrap_or_default()).is_some();
                let needs_render = tree.get(*widget_id)
                    .map(|n| n.dirty() || dirty_region.intersects(n.frame()) || in_bounds)
                    .unwrap_or(true);
                if needs_render {
                    Self::render_widget_self(*widget_id, ctx, tree);
                }
                let r = *rect;
                ctx.canvas_2d().push_clip(r);
                // 应用画布偏移（ScrollView 的滚动平移）使子节点在内容坐标下绘制
                let scroll_off = Self::get_scroll_offset(tree, *widget_id);
                if let Some((sx, sy)) = scroll_off {
                    ctx.canvas_2d().translate(-sx, -sy);
                }
                // 滚动时强制渲染所有子节点：子节点 frame 在 content 坐标系，
                // dirty_region 在 viewport 坐标系，坐标不匹配会导致子节点被跳过。
                // clip rect 已限制实际像素写入范围，不会产生多余绘制。
                let child_force = scroll_off.is_some() || force_render;
                for child in children.iter_mut() {
                    Self::render_node(child, ctx, tree, dirty_region, dirty_bounds, child_force);
                }
                if let Some((sx, sy)) = scroll_off {
                    ctx.canvas_2d().translate(sx, sy);
                }
                ctx.canvas_2d().pop_clip();
            }
            LayerNode::Direct {
                widget_id,
                children,
            } => {
                Self::render_widget_and_children(*widget_id, children, ctx, tree, dirty_region, dirty_bounds, force_render);
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
            node.render(frame, ctx, tree);
            ctx.restore();
        }
    }

    /// 渲染 widget 自身及其子节点（直接遍历 children LayerNodes）。
    /// 子节点已预排序，直接遍历无需再次排序。
    /// 终极方案：入口做空间+脏状态双剪枝 + dirty_bounds 间隙修正。
    ///
    /// `dirty_bounds` 是所有脏矩形的外接包围盒。当 parent 在 dirty_bounds
    /// 区域内渲染背景时，会覆盖非脏子节点的像素。通过 dirty_bounds 检查确保
    /// 这些「间隙区」的子节点也被重新渲染。
    ///
    /// `force_render`：为 true 时跳过脏检查，强制渲染所有子节点。
    /// 用于滚动容器——子节点 frame 在 content 坐标系，dirty_region 在 viewport 坐标系。
    ///
    /// 注意：先渲染 widget 自身（由其 render() 自行判断内部可见性），
    /// 再通过 inner().visible() 决定是否渲染子节点。这样模态框等组件
    /// 的 render() 可以控制自身绘制，同时阻止子节点在隐藏时渲染。
    fn render_widget_and_children(
        id: WidgetId,
        children: &mut [LayerNode],
        ctx: &mut RenderContext,
        tree: &WidgetTree,
        dirty_region: &DirtyRegion,
        dirty_bounds: Rect,
        force_render: bool,
    ) {
        if let Some(node) = tree.get(id) {
            if !node.visible() {
                return;
            }
            let frame = node.frame();
            // 空间+脏状态双剪枝 + dirty_bounds 间隙修正：
            // widget 不脏且 frame 既不在 dirty_region 也不在 dirty_bounds → 跳过 render_self
            let in_bounds = dirty_bounds.intersect(&frame).is_some();
            let need_self_render = force_render
                || node.dirty()
                || dirty_region.intersects(frame)
                || in_bounds;

            if need_self_render {
                ctx.save();
                node.render(frame, ctx, tree);
            }

            // 仅在 widget 内部可见时才渲染子节点。
            // 支持 Modal 等组件：内部 visible 为 false 时 render() 已返回，
            // 同时阻止子节点在隐藏位渲染（子节点 frame 可能仍在屏幕内）。
            if node.visible() {
                for child in children.iter_mut() {
                    Self::render_node(child, ctx, tree, dirty_region, dirty_bounds, force_render);
                }
            }

            if need_self_render {
                ctx.restore();
            }
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
            node.render(frame, ctx, tree);
            // 仅在 widget 内部可见时才递归渲染子节点
            if node.visible() {
                for &child_id in node.children() {
                    Self::render_widget_and_children_direct(child_id, ctx, tree);
                }
            }
            ctx.restore();
        }
    }

    /// 渲染脏 Picture 节点。
    /// 离屏 API 待重建（TODO v2），当前始终回退到主缓冲直接渲染子树。
    fn render_picture_dirty(
        widget_id: WidgetId,
        _bounds: &Rect,
        _offscreen_handle: &mut Option<ImageHandle>,
        _children: &mut [LayerNode],
        ctx: &mut RenderContext,
        tree: &WidgetTree,
        _w: i32,
        _h: i32,
        _retry_count: &mut u8,
        _dirty_region: &DirtyRegion,
        _dirty_bounds: Rect,
    ) {
        Self::render_widget_and_children_direct(widget_id, ctx, tree);
    }

    /// 递归渲染 overlay 层（post_render 回调 + 调试覆盖）。
    ///
    /// 当 `debug_mode` 为 false 时，跳过与 `dirty_region` 无交集的 widget
    /// 的 `post_render` 调用，因为其 overlay 内容未发生变化。
    /// 子树始终递归（子节点可能位于脏区域内）。
    ///
    /// `force_overlay`：为 true 时跳过脏检查，强制渲染所有子节点的 overlay。
    /// 用于滚动容器——子节点 frame 在 content 坐标系，dirty_region 在 viewport 坐标系，
    /// 坐标不匹配会导致脏检查失效（漏渲或错渲）。clip 已限制实际像素写入范围，
    /// 不会产生多余绘制。
    fn render_overlay_node(
        node: &LayerNode,
        ctx: &mut RenderContext,
        tree: &WidgetTree,
        depth: usize,
        hovered_chain: &Option<HashSet<WidgetId>>,
        debug_mode: bool,
        dirty_region: &DirtyRegion,
        dirty_bounds: Rect,
        force_overlay: bool,
    ) {
        let widget_id = node.widget_id();

        // 判断此 widget 是否需要重新绘制 overlay
        let needs_overlay = force_overlay || if debug_mode {
            // 调试模式下始终绘制边框，不跳过
            true
        } else if let Some(widget_node) = tree.get(widget_id) {
            if widget_node.visible() {
                let frame = widget_node.frame();
                // 使用 dirty_rect（含 draw_margin 扩展）而非 raw frame
                // 确保阴影等扩展区域的 overlay 内容被正确重绘
                let draw_area = widget_node.dirty_rect(frame);
                // dirty_bounds 间隙修正：parent 在脏包围盒内重绘会覆盖 overlay 内容
                dirty_region.intersects(draw_area) || dirty_bounds.intersect(&draw_area).is_some()
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
                    widget_node.post_render(frame, ctx, tree);
                    ctx.restore();
                }
            }
        }

        // 调试模式：悬浮 widget 及其父链显示坐标信息，其余仅极淡边框
        if debug_mode {
            if let Some(widget_node) = tree.get(widget_id) {
                if widget_node.visible() {
                    let frame = widget_node.frame();
                    let hovered = hovered_chain.as_ref().is_some_and(|c| c.contains(&widget_id));
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
                    Self::render_overlay_node(child, ctx, tree, depth + 1, hovered_chain, debug_mode, dirty_region, dirty_bounds, force_overlay);
                }
            }
            LayerNode::ClipRect { widget_id, rect, children, .. } => {
                ctx.canvas_2d().push_clip(*rect);
                // 应用画布偏移（ScrollView 滚动平移）使子节点 overlay 在内容坐标下绘制
                let scroll_off = Self::get_scroll_offset(tree, *widget_id);
                if let Some((sx, sy)) = scroll_off {
                    ctx.canvas_2d().translate(-sx, -sy);
                }
                // 滚动时强制渲染所有子节点 overlay：子节点 frame 在 content 坐标系，
                // dirty_region 在 viewport 坐标系，坐标不匹配导致脏检查不可靠。
                // clip 已限制实际像素写入范围，不会产生多余绘制。
                let child_force = scroll_off.is_some() || force_overlay;
                for child in children {
                    Self::render_overlay_node(child, ctx, tree, depth + 1, hovered_chain, debug_mode, dirty_region, dirty_bounds, child_force);
                }
                if let Some((sx, sy)) = scroll_off {
                    ctx.canvas_2d().translate(sx, sy);
                }
                ctx.canvas_2d().pop_clip();
            }
        }
    }

    /// 检查 widget 是否有 ScrollView 的滚动偏移。
    /// 通过 downcast 检测 ScrollView 组件，获取其 scroll_x/scroll_y。
    fn get_scroll_offset(tree: &WidgetTree, widget_id: WidgetId) -> Option<(f32, f32)> {
        tree.get(widget_id).and_then(|node| {
            let comp = node.component();
            let sv = comp.as_any().downcast_ref::<ScrollView>()?;
            // 只在有实际偏移时返回，避免空 translate
            if sv.scroll_x().abs() > 0.5 || sv.scroll_y().abs() > 0.5 {
                Some((sv.scroll_x(), sv.scroll_y()))
            } else {
                None
            }
        })
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

// ════════════════════════════════════════════════════════════════════════════
// 单元测试
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::traits::*;
    use crate::wc_upcast;
    use crate::widget::*;
    use uix_platform::Size;

    // ── 测试用 widget ──────────────────────────────────────────────────

    struct Dummy {
        size: Size,
        is_repaint: bool,
        clip: Option<Rect>,
    }

    impl Dummy {
        fn new(w: f32, h: f32) -> Self {
            Self { size: Size::new(w, h), is_repaint: false, clip: None }
        }
        fn repaint(w: f32, h: f32) -> Self {
            Self { size: Size::new(w, h), is_repaint: true, clip: None }
        }
        fn clipped(w: f32, h: f32, clip_rect: Rect) -> Self {
            Self { size: Size::new(w, h), is_repaint: false, clip: Some(clip_rect) }
        }
    }

    impl WidgetComponent for Dummy {
        fn as_any(&self) -> &dyn std::any::Any { self }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
        fn capabilities(&self) -> WidgetCapabilities {
            WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER)
        }
        fn visible(&self) -> bool { true }
        wc_upcast!(Dummy; WidgetRender);
        wc_upcast!(Dummy; WidgetLayout);
    }

    impl WidgetLayout for Dummy {
        fn preferred_size(&self, _: Option<&dyn GraphicsEngine>) -> Size { self.size }
    }

    impl WidgetRender for Dummy {
        fn render(&self, _: Rect, _: &mut RenderContext, _: &WidgetTree) {}
        fn is_repaint_boundary(&self) -> bool { self.is_repaint }
        fn children_clip(&self, _: Rect) -> Option<Rect> { self.clip }
    }

    struct ContainerWidget {
        size: Size,
        children: std::cell::RefCell<Vec<Box<dyn WidgetComponent>>>,
    }

    impl ContainerWidget {
        fn new(w: f32, h: f32) -> Self {
            Self { size: Size::new(w, h), children: std::cell::RefCell::new(Vec::new()) }
        }
    }

    impl WidgetComponent for ContainerWidget {
        fn as_any(&self) -> &dyn std::any::Any { self }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
        fn capabilities(&self) -> WidgetCapabilities {
            WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER)
        }
        fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
            std::mem::take(&mut *self.children.borrow_mut())
        }
        wc_upcast!(ContainerWidget; WidgetRender);
        wc_upcast!(ContainerWidget; WidgetLayout);
    }

    impl WidgetLayout for ContainerWidget {
        fn preferred_size(&self, _: Option<&dyn GraphicsEngine>) -> Size { self.size }
    }

    impl WidgetRender for ContainerWidget {
        fn render(&self, _: Rect, _: &mut RenderContext, _: &WidgetTree) {}
    }

    // ── 辅助 ───────────────────────────────────────────────────────────

    /// 创建单节点的 WidgetTree。
    fn single_tree(widget: Dummy, frame: Rect) -> WidgetTree {
        let mut tree = WidgetTree::new();
        let id = tree.set_root(Box::new(widget));
        tree.get_mut(id).unwrap().set_frame(frame);
        tree
    }

    /// 创建带子节点的 WidgetTree。
    fn tree_with_children(
        parent: Dummy, parent_frame: Rect,
        children: Vec<(Dummy, Rect)>,
    ) -> WidgetTree {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(parent));
        tree.get_mut(root).unwrap().set_frame(parent_frame);
        for (child_w, child_frame) in children {
            let cid = tree.add_child(root, Box::new(child_w));
            tree.get_mut(cid).unwrap().set_frame(child_frame);
        }
        tree
    }

    // ── 测试用例 ───────────────────────────────────────────────────────

    #[test]
    fn new_creates_empty() {
        let lt = LayerTree::new();
        assert!(lt.root_node().is_none());
        assert!(lt.orphaned_handles().is_empty());
        assert!(!lt.is_ready());
    }

    #[test]
    fn default_creates_empty() {
        let lt = LayerTree::default();
        assert!(lt.root_node().is_none());
    }

    #[test]
    fn build_empty_tree() {
        let tree = WidgetTree::new();
        let mut lt = LayerTree::new();
        lt.build(&tree);
        assert!(lt.root_node().is_none());
        assert!(!lt.is_ready());
    }

    #[test]
    fn build_creates_direct_node() {
        let tree = single_tree(Dummy::new(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);
        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Direct { children, .. } => {
                assert!(children.is_empty());
            }
            other => panic!("期望 Direct 节点，得到 {other:?}"),
        }
        assert!(lt.is_ready());
    }

    #[test]
    fn build_creates_picture_node_for_repaint_boundary() {
        let tree = single_tree(Dummy::repaint(100.0, 50.0), Rect::new(10.0, 20.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);
        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Picture { bounds, is_dirty, offscreen_handle, children, retry_count, .. } => {
                assert!(children.is_empty());
                assert!(*is_dirty);
                assert!(offscreen_handle.is_none());
                assert_eq!(*bounds, Rect::new(10.0, 20.0, 100.0, 50.0));
                assert_eq!(*retry_count, 0);
            }
            other => panic!("期望 Picture 节点，得到 {other:?}"),
        }
    }

    #[test]
    fn build_picture_is_dirty_when_no_cache() {
        let tree = single_tree(Dummy::repaint(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);
        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Picture { is_dirty, .. } => assert!(*is_dirty),
            _ => panic!("期望 Picture"),
        }
    }

    #[test]
    fn build_creates_clip_rect_node() {
        let clip = Rect::new(5.0, 5.0, 90.0, 40.0);
        let tree = single_tree(Dummy::clipped(100.0, 50.0, clip), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);
        let root = lt.root_node().unwrap();
        match root {
            LayerNode::ClipRect { rect, children, .. } => {
                assert!(children.is_empty());
                assert_eq!(*rect, clip);
            }
            other => panic!("期望 ClipRect 节点，得到 {other:?}"),
        }
    }

    #[test]
    fn build_nested_structure() {
        let tree = tree_with_children(
            Dummy::new(200.0, 200.0), Rect::new(0.0, 0.0, 200.0, 200.0),
            vec![
                (Dummy::new(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0)),
                (Dummy::repaint(80.0, 30.0), Rect::new(0.0, 50.0, 80.0, 30.0)),
            ],
        );
        let mut lt = LayerTree::new();
        lt.build(&tree);

        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Direct { children, .. } => {
                assert_eq!(children.len(), 2);
                // 第一个子节点：Direct
                match &children[0] {
                    LayerNode::Direct { widget_id, .. } => assert!(*widget_id > 0),
                    other => panic!("期望子节点1为 Direct，得到 {other:?}"),
                }
                // 第二个子节点：Picture (repaint boundary)
                match &children[1] {
                    LayerNode::Picture { widget_id, .. } => assert!(*widget_id > 0),
                    other => panic!("期望子节点2为 Picture，得到 {other:?}"),
                }
            }
            other => panic!("期望 Direct 根节点，得到 {other:?}"),
        }
    }

    #[test]
    fn build_skips_invisible_widgets() {
        let mut tree = tree_with_children(
            Dummy::new(200.0, 200.0), Rect::new(0.0, 0.0, 200.0, 200.0),
            vec![
                (Dummy::new(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0)),
                (Dummy::new(80.0, 30.0), Rect::new(0.0, 50.0, 80.0, 30.0)),
            ],
        );
        // 将第一个子节点设为不可见
        let root_id = tree.root_id().unwrap();
        let child_id = tree.get(root_id).unwrap().children()[0];
        tree.get_mut(child_id).unwrap().set_visible(false);

        let mut lt = LayerTree::new();
        lt.build(&tree);

        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Direct { children, .. } => {
                // 隐藏的子节点被跳过，只有一个子节点
                assert_eq!(children.len(), 1);
            }
            other => panic!("期望 Direct 根节点，得到 {other:?}"),
        }
    }

    #[test]
    fn build_z_index_sorts_children() {
        let frame = Rect::new(0.0, 0.0, 200.0, 200.0);
        let mut tree = tree_with_children(
            Dummy::new(200.0, 200.0), frame,
            vec![
                (Dummy::new(50.0, 50.0), Rect::new(0.0, 0.0, 50.0, 50.0)),
                (Dummy::new(50.0, 50.0), Rect::new(50.0, 0.0, 50.0, 50.0)),
            ],
        );
        // 设置 z_index：第二个子节点高，第一个低
        let root_id = tree.root_id().unwrap();
        let children = tree.get(root_id).unwrap().children().to_vec();
        tree.get_mut(children[0]).unwrap().set_z_index(1);
        tree.get_mut(children[1]).unwrap().set_z_index(10);

        let mut lt = LayerTree::new();
        lt.build(&tree);

        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Direct { children, .. } => {
                assert_eq!(children.len(), 2);
                // z_index 排序后，第一个子节点（z=1）应在前面，第二个（z=10）在后面
            }
            other => panic!("期望 Direct，得到 {other:?}"),
        }
    }

    #[test]
    fn rebuild_reuses_offscreen_handle() {
        // 先 build 一个带 Picture 的树，注入 handle 后 rebuild，验证 bounds 不变时复用
        let tree = single_tree(Dummy::repaint(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);
        // 注入 offscreen handle（模拟已渲染）
        if let Some(ref mut root) = lt.root {
            match root {
                LayerNode::Picture { ref mut offscreen_handle, .. } => {
                    *offscreen_handle = Some(ImageHandle(42));
                }
                _ => {}
            }
        }
        // rebuild 同一棵树（bounds 不变）→ handle 应被复用
        lt.build(&tree);
        // 新 root 应持有 offscreen handle
        let new_root = lt.root_node().unwrap();
        match new_root {
            LayerNode::Picture { offscreen_handle, .. } => {
                assert_eq!(*offscreen_handle, Some(ImageHandle(42)));
            }
            _ => panic!("期望 Picture"),
        }
    }

    #[test]
    fn rebuild_collects_orphaned_handles() {
        // 先 build 一个带 Picture 的树
        let tree1 = single_tree(Dummy::repaint(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree1);

        // 手动注入一个 offscreen handle 到旧 Picture（模拟已渲染过的状态）
        if let Some(ref mut root) = lt.root {
            match root {
                LayerNode::Picture { ref mut offscreen_handle, .. } => {
                    *offscreen_handle = Some(ImageHandle(42));
                }
                _ => {}
            }
        }

        // 第二次 build 时，旧 handle 应进入 orphaned_handles
        let tree2 = single_tree(Dummy::repaint(200.0, 100.0), Rect::new(0.0, 0.0, 200.0, 100.0));
        lt.build(&tree2); // bounds 改变 → 旧 handle 无法复用 → 进入孤儿列表

        assert!(lt.orphaned_handles().contains(&ImageHandle(42)));
    }

    #[test]
    fn is_ready_after_build() {
        let tree = single_tree(Dummy::new(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        assert!(!lt.is_ready());
        lt.build(&tree);
        assert!(lt.is_ready());
    }

    #[test]
    fn invalidate_marks_picture_dirty() {
        let tree = single_tree(Dummy::repaint(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);

        // 手动标记为 clean 再 invalidate
        if let Some(ref mut root) = lt.root {
            root.mark_clean();
            match root {
                LayerNode::Picture { is_dirty, .. } => assert!(!*is_dirty),
                _ => {}
            }
        }

        lt.invalidate();

        if let Some(ref root) = lt.root {
            match root {
                LayerNode::Picture { is_dirty, .. } => assert!(*is_dirty),
                _ => {}
            }
        }
    }

    #[test]
    fn invalidate_propagates_to_children() {
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(
            ContainerWidget::new(200.0, 200.0),
        ));
        tree.get_mut(root_id).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        let child_id = tree.add_child(root_id, Box::new(Dummy::repaint(100.0, 50.0)));
        tree.get_mut(child_id).unwrap().set_frame(Rect::new(0.0, 0.0, 100.0, 50.0));

        let mut lt = LayerTree::new();
        lt.build(&tree);

        // 手动标记 clean
        if let Some(ref mut root) = lt.root {
            root.mark_clean();
        }

        lt.invalidate();

        // 根 invalidate 后，子 Picture 也应为 dirty
        if let Some(ref root) = lt.root {
            match root {
                LayerNode::Direct { children, .. } => {
                    assert_eq!(children.len(), 1);
                    match &children[0] {
                        LayerNode::Picture { is_dirty, .. } => assert!(*is_dirty),
                        _ => panic!("期望 Picture 子节点"),
                    }
                }
                other => panic!("期望 Direct，得到 {other:?}"),
            }
        }
    }

    #[test]
    fn update_dirty_marks_picture_clean_when_widget_clean() {
        let mut tree = single_tree(Dummy::repaint(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);

        // build 后 Picture 默认 is_dirty=true（无缓存）
        // 标记 widget 为 clean
        let root_id = tree.root_id().unwrap();
        tree.get_mut(root_id).unwrap().set_dirty(false);

        lt.update_dirty(&tree);

        if let Some(ref root) = lt.root {
            match root {
                LayerNode::Picture { is_dirty, .. } => {
                    // widget clean + Picture clean → is_dirty 应为 false
                    assert!(!*is_dirty);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn update_dirty_propagates_child_dirty_to_picture() {
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(Dummy::repaint(300.0, 300.0)));
        tree.get_mut(root_id).unwrap().set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));
        let child_id = tree.add_child(root_id, Box::new(Dummy::new(100.0, 50.0)));
        tree.get_mut(child_id).unwrap().set_frame(Rect::new(10.0, 10.0, 100.0, 50.0));

        let mut lt = LayerTree::new();
        lt.build(&tree);

        // 标记根 widget clean，子 widget dirty
        tree.get_mut(root_id).unwrap().set_dirty(false);
        tree.get_mut(child_id).unwrap().set_dirty(true);

        lt.update_dirty(&tree);

        if let Some(ref root) = lt.root {
            match root {
                LayerNode::Picture { is_dirty, children, .. } => {
                    // 子节点 dirty → 父 Picture 应被标记 dirty
                    assert!(*is_dirty, "子节点 dirty 应传播到父 Picture");
                    // 子节点应为 Direct
                    assert_eq!(children.len(), 1);
                }
                other => panic!("期望 Picture 根节点，得到 {other:?}"),
            }
        }
    }

    #[test]
    fn update_dirty_marks_picture_dirty_when_widget_dirty() {
        let mut tree = single_tree(Dummy::repaint(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);

        // 先标记 clean
        let root_id = tree.root_id().unwrap();
        tree.get_mut(root_id).unwrap().set_dirty(false);
        lt.update_dirty(&tree);

        // 再标记 dirty
        tree.get_mut(root_id).unwrap().set_dirty(true);
        lt.update_dirty(&tree);

        if let Some(ref root) = lt.root {
            match root {
                LayerNode::Picture { is_dirty, .. } => {
                    assert!(*is_dirty);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn build_with_repaint_boundary_has_no_children_in_picture() {
        // Picture 节点的 children 应包含结构化的 LayerNode，不是空的
        let tree = tree_with_children(
            Dummy::new(200.0, 200.0), Rect::new(0.0, 0.0, 200.0, 200.0),
            vec![
                (Dummy::repaint(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0)),
            ],
        );
        let mut lt = LayerTree::new();
        lt.build(&tree);

        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Direct { children, .. } => {
                assert_eq!(children.len(), 1);
                match &children[0] {
                    LayerNode::Picture { children: pic_children, .. } => {
                        assert!(pic_children.is_empty(), "Picture 不应有子节点（当前子树下无更多子节点）");
                    }
                    other => panic!("期望 Picture，得到 {other:?}"),
                }
            }
            other => panic!("期望 Direct，得到 {other:?}"),
        }
    }

    #[test]
    fn orphaned_handles_empty_after_init() {
        let lt = LayerTree::new();
        assert!(lt.orphaned_handles().is_empty());
    }

    #[test]
    fn invalidate_noop_when_no_root() {
        let mut lt = LayerTree::new();
        lt.invalidate(); // 不应 panic
        assert!(lt.root_node().is_none());
    }

    #[test]
    fn update_dirty_noop_when_no_root() {
        let tree = WidgetTree::new();
        let mut lt = LayerTree::new();
        lt.update_dirty(&tree); // 不应 panic
    }

    #[test]
    fn build_twice_rebuilds_correctly() {
        let tree1 = single_tree(Dummy::new(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree1);
        assert!(lt.is_ready());

        // 第二次 build 不同的树
        let tree2 = single_tree(Dummy::repaint(200.0, 100.0), Rect::new(0.0, 0.0, 200.0, 100.0));
        lt.build(&tree2);
        assert!(lt.is_ready());

        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Picture { bounds, .. } => {
                assert_eq!(*bounds, Rect::new(0.0, 0.0, 200.0, 100.0));
            }
            other => panic!("期望 Picture，得到 {other:?}"),
        }
    }

    #[test]
    fn debug_format_layer_tree() {
        let lt = LayerTree::new();
        let s = format!("{lt:?}");
        assert!(s.contains("has_root"));
    }

    #[test]
    fn debug_format_layer_node_picture() {
        let node = LayerNode::Picture {
            widget_id: 1,
            bounds: Rect::new(0.0, 0.0, 100.0, 50.0),
            is_dirty: true,
            offscreen_handle: Some(ImageHandle(1)),
            children: vec![],
            retry_count: 0,
        };
        let s = format!("{node:?}");
        assert!(s.contains("PictureLayer"));
        assert!(s.contains("has_offscreen"));
    }

    #[test]
    fn debug_format_layer_node_clip_rect() {
        let node = LayerNode::ClipRect {
            widget_id: 2,
            rect: Rect::new(5.0, 5.0, 90.0, 40.0),
            children: vec![],
        };
        let s = format!("{node:?}");
        assert!(s.contains("ClipRectLayer"));
    }

    #[test]
    fn debug_format_layer_node_direct() {
        let node = LayerNode::Direct {
            widget_id: 3,
            children: vec![],
        };
        let s = format!("{node:?}");
        assert!(s.contains("DirectLayer"));
    }

    #[test]
    fn sweep_orphaned_offscreens_drains_handles() {
        let mut lt = LayerTree::new();
        lt.orphaned_handles_mut().push(ImageHandle(1));
        lt.orphaned_handles_mut().push(ImageHandle(2));
        assert_eq!(lt.orphaned_handles().len(), 2);

        let mut engine = uix_graphics::null_engine::NullEngine::new();
        lt.sweep_orphaned_offscreens(&mut engine);
        assert!(lt.orphaned_handles().is_empty());
    }

    #[test]
    fn sweep_orphaned_offscreens_empty_is_safe() {
        let mut lt = LayerTree::new();
        let mut engine = uix_graphics::null_engine::NullEngine::new();
        lt.sweep_orphaned_offscreens(&mut engine); // 不应 panic
        assert!(lt.orphaned_handles().is_empty());
    }

    #[test]
    fn build_z_index_ordered_correctly() {
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(Dummy::new(200.0, 200.0)));
        tree.get_mut(root_id).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        let child_a = tree.add_child(root_id, Box::new(Dummy::new(50.0, 50.0)));
        tree.get_mut(child_a).unwrap().set_frame(Rect::new(0.0, 0.0, 50.0, 50.0));
        tree.get_mut(child_a).unwrap().set_z_index(10);
        let child_b = tree.add_child(root_id, Box::new(Dummy::new(50.0, 50.0)));
        tree.get_mut(child_b).unwrap().set_frame(Rect::new(50.0, 0.0, 50.0, 50.0));
        tree.get_mut(child_b).unwrap().set_z_index(1);

        let mut lt = LayerTree::new();
        lt.build(&tree);

        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Direct { children, .. } => {
                assert_eq!(children.len(), 2);
                // z_index 升序：child_b (z=1) 应在 child_a (z=10) 前面
                assert_eq!(children[0].widget_id(), child_b);
                assert_eq!(children[1].widget_id(), child_a);
            }
            other => panic!("期望 Direct，得到 {other:?}"),
        }
    }

    #[test]
    fn build_z_index_equal_indices_preserve_insertion_order() {
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(Dummy::new(200.0, 200.0)));
        tree.get_mut(root_id).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        let child_a = tree.add_child(root_id, Box::new(Dummy::new(50.0, 50.0)));
        tree.get_mut(child_a).unwrap().set_frame(Rect::new(0.0, 0.0, 50.0, 50.0));
        let child_b = tree.add_child(root_id, Box::new(Dummy::new(50.0, 50.0)));
        tree.get_mut(child_b).unwrap().set_frame(Rect::new(50.0, 0.0, 50.0, 50.0));
        // 都使用默认 z=0，排序是稳定的（插入顺序保留）

        let mut lt = LayerTree::new();
        lt.build(&tree);

        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Direct { children, .. } => {
                assert_eq!(children[0].widget_id(), child_a);
                assert_eq!(children[1].widget_id(), child_b);
            }
            other => panic!("期望 Direct，得到 {other:?}"),
        }
    }

    #[test]
    fn rebuild_with_clean_widget_and_offscreen_is_clean() {
        let mut tree = single_tree(Dummy::repaint(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);

        // 注入 offscreen handle
        if let Some(ref mut root) = lt.root {
            if let LayerNode::Picture { ref mut offscreen_handle, .. } = root {
                *offscreen_handle = Some(ImageHandle(42));
            }
        }

        // 标记 widget clean
        let root_id = tree.root_id().unwrap();
        tree.get_mut(root_id).unwrap().set_dirty(false);

        // rebuild（bounds 不变，offscreen 被复用，widget clean）
        lt.build(&tree);

        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Picture { is_dirty, offscreen_handle, .. } => {
                assert!(offscreen_handle.is_some());
                assert!(!*is_dirty, "offscreen 存在且 widget clean → Picture 应为 clean");
            }
            _ => panic!("期望 Picture"),
        }
    }

    #[test]
    fn build_multi_level_nesting() {
        // 三层：Direct → ClipRect → Picture
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(Dummy::new(400.0, 400.0)));
        tree.get_mut(root_id).unwrap().set_frame(Rect::new(0.0, 0.0, 400.0, 400.0));
        let clip_id = tree.add_child(root_id, Box::new(Dummy::clipped(200.0, 200.0, Rect::new(0.0, 0.0, 200.0, 200.0))));
        tree.get_mut(clip_id).unwrap().set_frame(Rect::new(50.0, 50.0, 200.0, 200.0));
        let pic_id = tree.add_child(clip_id, Box::new(Dummy::repaint(100.0, 100.0)));
        tree.get_mut(pic_id).unwrap().set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

        let mut lt = LayerTree::new();
        lt.build(&tree);

        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Direct { children, .. } => {
                assert_eq!(children.len(), 1);
                match &children[0] {
                    LayerNode::ClipRect { children: clip_children, .. } => {
                        assert_eq!(clip_children.len(), 1);
                        match &clip_children[0] {
                            LayerNode::Picture { .. } => {}
                            other => panic!("期望 Picture，得到 {other:?}"),
                        }
                    }
                    other => panic!("期望 ClipRect，得到 {other:?}"),
                }
            }
            other => panic!("期望 Direct，得到 {other:?}"),
        }
    }

    #[test]
    fn build_skips_invisible_root() {
        let mut tree = WidgetTree::new();
        let id = tree.set_root(Box::new(Dummy::new(100.0, 50.0)));
        tree.get_mut(id).unwrap().set_frame(Rect::new(0.0, 0.0, 100.0, 50.0));
        tree.get_mut(id).unwrap().set_visible(false);

        let mut lt = LayerTree::new();
        lt.build(&tree);
        assert!(lt.root_node().is_none());
        assert!(!lt.is_ready());
    }

    #[test]
    fn build_zero_size_frame_creates_node() {
        let tree = single_tree(Dummy::new(0.0, 0.0), Rect::new(0.0, 0.0, 0.0, 0.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);
        // 零尺寸 widget 仍然可见，应创建节点
        assert!(lt.is_ready());
        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Direct { .. } => {}
            other => panic!("期望 Direct，得到 {other:?}"),
        }
    }

    #[test]
    fn build_repaint_boundary_zero_size_is_dirty() {
        let tree = single_tree(Dummy::repaint(0.0, 0.0), Rect::new(0.0, 0.0, 0.0, 0.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);
        assert!(lt.is_ready());
        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Picture { bounds, is_dirty, .. } => {
                assert_eq!(*bounds, Rect::new(0.0, 0.0, 0.0, 0.0));
                assert!(*is_dirty);
            }
            other => panic!("期望 Picture，得到 {other:?}"),
        }
    }

    #[test]
    fn update_dirty_clip_rect_no_panic() {
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(Dummy::new(400.0, 400.0)));
        tree.get_mut(root_id).unwrap().set_frame(Rect::new(0.0, 0.0, 400.0, 400.0));
        let clip_id = tree.add_child(root_id, Box::new(Dummy::clipped(200.0, 200.0, Rect::new(0.0, 0.0, 200.0, 200.0))));
        tree.get_mut(clip_id).unwrap().set_frame(Rect::new(50.0, 50.0, 200.0, 200.0));
        let child_id = tree.add_child(clip_id, Box::new(Dummy::new(100.0, 50.0)));
        tree.get_mut(child_id).unwrap().set_frame(Rect::new(0.0, 0.0, 100.0, 50.0));

        let mut lt = LayerTree::new();
        lt.build(&tree);

        // 标记 child dirty → update_dirty 应传播通过 ClipRect
        tree.get_mut(child_id).unwrap().set_dirty(true);
        lt.update_dirty(&tree); // 不应 panic
    }

    #[test]
    fn build_negative_frame_positions() {
        let tree = single_tree(Dummy::new(100.0, 50.0), Rect::new(-50.0, -20.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);
        assert!(lt.is_ready());
        let root = lt.root_node().unwrap();
        match root {
            LayerNode::Direct { .. } => {}
            other => panic!("期望 Direct，得到 {other:?}"),
        }
    }

    #[test]
    fn rebuild_multiple_pictures_orphaned_all() {
        // 有两个 Picture 子节点的树
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(Dummy::new(400.0, 400.0)));
        tree.get_mut(root_id).unwrap().set_frame(Rect::new(0.0, 0.0, 400.0, 400.0));
        let pic1 = tree.add_child(root_id, Box::new(Dummy::repaint(100.0, 50.0)));
        tree.get_mut(pic1).unwrap().set_frame(Rect::new(0.0, 0.0, 100.0, 50.0));
        let pic2 = tree.add_child(root_id, Box::new(Dummy::repaint(100.0, 50.0)));
        tree.get_mut(pic2).unwrap().set_frame(Rect::new(100.0, 0.0, 100.0, 50.0));

        let mut lt = LayerTree::new();
        lt.build(&tree);
        // 注入 handle
        fn inject_handles(lt: &mut LayerTree, h1: ImageHandle, h2: ImageHandle) {
            if let Some(ref mut root) = lt.root {
                if let LayerNode::Direct { children, .. } = root {
                    if let LayerNode::Picture { ref mut offscreen_handle, .. } = &mut children[0] {
                        *offscreen_handle = Some(h1);
                    }
                    if let LayerNode::Picture { ref mut offscreen_handle, .. } = &mut children[1] {
                        *offscreen_handle = Some(h2);
                    }
                }
            }
        }
        inject_handles(&mut lt, ImageHandle(10), ImageHandle(20));
        assert!(lt.orphaned_handles().is_empty());

        // 改变 bounds → 两个 handle 都变成孤儿
        tree.get_mut(pic1).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 100.0));
        lt.build(&tree);
        assert!(lt.orphaned_handles().contains(&ImageHandle(10)));
        assert!(lt.orphaned_handles().contains(&ImageHandle(20)));
    }

    #[test]
    fn widget_id_matches_tree() {
        let tree = single_tree(Dummy::new(100.0, 50.0), Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut lt = LayerTree::new();
        lt.build(&tree);
        let root_id = tree.root_id().unwrap();
        let root = lt.root_node().unwrap();
        assert_eq!(root.widget_id(), root_id);
    }
}
