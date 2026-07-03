//! LayerTree — 合成树（Phase 3 迁入 graphics）。
//!
//! 通过 ScenePaint trait 读取场景结构并下发绘制，不依赖 uix-ui。

use std::collections::HashSet;

use uix_platform::{Point, Rect};

use crate::compositor::ScenePaint;
use crate::font_service::FontService;
use crate::painting::{PaintContext, ThemeSnapshot};
use crate::pipeline::NodeId;
use crate::traits::GraphicsEngine;
use crate::types::{DirtyRegion, ImageHandle};
use crate::FontHandle;

/// 图层节点。
pub enum LayerNode {
    /// 图片图层：缓存 RepaintBoundary 子树的栅格结果。
    /// 包含 children 以支持嵌套 RepaintBoundary（#96）。
    Picture {
        widget_id: NodeId,
        bounds: Rect,
        is_dirty: bool,
        offscreen_handle: Option<ImageHandle>,
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
        // 收集旧 Picture 节点的离屏缓冲（按 widget_id 索引）
        let mut old_cache: std::collections::HashMap<NodeId, (Rect, Option<ImageHandle>)> =
            std::collections::HashMap::new();
        if let Some(ref root) = self.root {
            Self::collect_picture_handles(root, &mut old_cache);
        }

        self.root = scene
            .root_id()
            .and_then(|root_id| Self::build_node_cached(scene, root_id, &old_cache));

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
    pub fn update_dirty(&mut self, scene: &impl ScenePaint) {
        if let Some(ref mut root) = self.root {
            Self::update_dirty_node(root, scene);
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
        scene: &impl ScenePaint,
        theme: &ThemeSnapshot<'_>,
        font: FontHandle,
        font_service: &FontService,
    ) {
        let dirty_region = scene.dirty_region();
        let dpi = engine.dpi();
        let dpr = engine.device_pixel_ratio();
        let orientation = engine.orientation();
        let surface_w = engine.canvas_2d().width();
        let surface_h = engine.canvas_2d().height();
        let mut rctx = PaintContext::new(
            engine.canvas_2d(),
            font,
            font_service,
            theme.tokens(),
            dpi,
            dpr,
            orientation,
            surface_w,
            surface_h,
        );
        let dirty_bounds = dirty_region.bounds();
        if let Some(ref mut root) = self.root {
            Self::render_node(root, &mut rctx, scene, dirty_region, dirty_bounds, false);
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
        scene: &impl ScenePaint,
        theme: &ThemeSnapshot<'_>,
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
        let mut rctx = PaintContext::new(
            engine.canvas_2d(),
            font,
            font_service,
            theme.tokens(),
            dpi,
            dpr,
            orientation,
            surface_w,
            surface_h,
        );
        rctx.set_debug_mode(debug_mode);

        // 预计算悬浮链：光标所在 widget + 所有父节点
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

        let dirty_bounds = dirty_region.bounds();
        if let Some(ref root) = self.root {
            Self::render_overlay_node(
                root,
                &mut rctx,
                scene,
                0,
                &hovered_chain,
                debug_mode,
                dirty_region,
                dirty_bounds,
                false,
            );
        }

        // 焦点环：在聚焦 widget 周围绘制 2px 轮廓
        if let Some(focused_id) = scene.focused_node() {
            if scene.node_visible(focused_id) && scene.node_focusable(focused_id) {
                let frame = scene.node_frame(focused_id);
                // 轮廓颜色使用主题 primary 色
                let focus_color = theme.tokens().color_primary();
                rctx.canvas_2d().stroke_rect(frame, focus_color, 2.0, None);
            }
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
        cache: &mut std::collections::HashMap<NodeId, (Rect, Option<ImageHandle>)>,
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
        scene: &impl ScenePaint,
        id: NodeId,
        cache: &std::collections::HashMap<NodeId, (Rect, Option<ImageHandle>)>,
    ) -> Option<LayerNode> {
        if !scene.node_visible(id) {
            return None;
        }
        let frame = scene.node_frame(id);

        if scene.is_repaint_boundary(id) {
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
            let children = Self::build_children_cached(scene, id, cache);
            Some(LayerNode::Picture {
                widget_id: id,
                bounds,
                is_dirty: offscreen_handle.is_none() || scene.node_dirty(id),
                offscreen_handle,
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
        cache: &std::collections::HashMap<NodeId, (Rect, Option<ImageHandle>)>,
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

    /// 递归渲染单个节点。
    /// 子节点已在 build 时预排序，直接遍历无需再次排序。
    /// 终极方案：空间+脏状态双剪枝。
    /// `dirty_bounds` 是所有脏矩形的外接包围盒，用于捕捉脏矩形间隙中的组件。
    /// `force_render`：为 true 时跳过子节点的脏检查，强制渲染所有子节点。
    /// 滚动容器（ScrollView）需要 force_render，因为子节点 frame 在 content 坐标系，
    /// 而 dirty_region 在 viewport 坐标系，直接交叉检测会导致子节点被错误跳过。
    fn render_node(
        node: &mut LayerNode,
        ctx: &mut PaintContext<'_>,
        scene: &impl ScenePaint,
        dirty_region: &DirtyRegion,
        dirty_bounds: Rect,
        force_render: bool,
    ) {
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
                        scene,
                        w,
                        h,
                        retry_count,
                        dirty_region,
                        dirty_bounds,
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
                let widget_frame = scene.node_frame(*widget_id);
                let in_bounds = dirty_bounds.intersect(&widget_frame).is_some();
                let needs_render = scene.node_dirty(*widget_id)
                    || dirty_region.intersects(widget_frame)
                    || in_bounds;
                if needs_render {
                    Self::render_widget_self(*widget_id, ctx, scene);
                }
                let r = *rect;
                ctx.canvas_2d().push_clip(r);
                // 应用画布偏移（ScrollView 的滚动平移）使子节点在内容坐标下绘制
                let scroll_off = Self::get_scroll_offset(scene, *widget_id);
                if let Some((sx, sy)) = scroll_off {
                    ctx.canvas_2d().translate(-sx, -sy);
                }
                // 滚动时强制渲染所有子节点：子节点 frame 在 content 坐标系，
                // dirty_region 在 viewport 坐标系，坐标不匹配会导致子节点被跳过。
                // clip rect 已限制实际像素写入范围，不会产生多余绘制。
                let child_force = scroll_off.is_some() || force_render;
                for child in children.iter_mut() {
                    Self::render_node(child, ctx, scene, dirty_region, dirty_bounds, child_force);
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
                Self::render_widget_and_children(
                    *widget_id,
                    children,
                    ctx,
                    scene,
                    dirty_region,
                    dirty_bounds,
                    force_render,
                );
            }
        }
    }

    /// 仅渲染 widget 自身的视觉效果（不处理子节点）。
    fn render_widget_self(id: NodeId, ctx: &mut PaintContext<'_>, scene: &impl ScenePaint) {
        if !scene.node_visible(id) {
            return;
        }
        let frame = scene.node_frame(id);
        ctx.save();
        scene.paint(id, frame, ctx);
        ctx.restore();
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
        id: NodeId,
        children: &mut [LayerNode],
        ctx: &mut PaintContext<'_>,
        scene: &impl ScenePaint,
        dirty_region: &DirtyRegion,
        dirty_bounds: Rect,
        force_render: bool,
    ) {
        if !scene.node_visible(id) {
            return;
        }
        let frame = scene.node_frame(id);
        // 空间+脏状态双剪枝 + dirty_bounds 间隙修正：
        // widget 不脏且 frame 既不在 dirty_region 也不在 dirty_bounds → 跳过 render_self
        let in_bounds = dirty_bounds.intersect(&frame).is_some();
        let need_self_render =
            force_render || scene.node_dirty(id) || dirty_region.intersects(frame) || in_bounds;

        if need_self_render {
            ctx.save();
            scene.paint(id, frame, ctx);
        }

        // 仅在 widget 内部可见时才渲染子节点。
        // 支持 Modal 等组件：内部 visible 为 false 时 render() 已返回，
        // 同时阻止子节点在隐藏位渲染（子节点 frame 可能仍在屏幕内）。
        if scene.node_visible(id) {
            for child in children.iter_mut() {
                Self::render_node(child, ctx, scene, dirty_region, dirty_bounds, force_render);
            }
        }

        if need_self_render {
            ctx.restore();
        }
    }

    /// 离屏创建失败时，回退到在主缓冲直接渲染 widget 及其子树。
    /// 不使用 LayerNode children，而是通过 ScenePaint 递归遍历整个子树。
    ///
    /// 注意：widget 的 frame 是绝对坐标（由 layout 阶段设置），
    /// 子节点 frame 也是绝对坐标，因此直接使用子节点的 frame 即可，
    /// 无需也不能累加父级偏移。
    fn render_widget_and_children_direct(
        widget_id: NodeId,
        ctx: &mut PaintContext<'_>,
        scene: &impl ScenePaint,
    ) {
        if !scene.node_visible(widget_id) {
            return;
        }
        let frame = scene.node_frame(widget_id);
        ctx.save();
        scene.paint(widget_id, frame, ctx);
        // 仅在 widget 内部可见时才递归渲染子节点
        if scene.node_visible(widget_id) {
            for &child_id in scene.node_children(widget_id) {
                Self::render_widget_and_children_direct(child_id, ctx, scene);
            }
        }
        ctx.restore();
    }

    /// 渲染脏 Picture 节点。
    /// 离屏 API 待重建（TODO v2），当前始终回退到主缓冲直接渲染子树。
    fn render_picture_dirty(
        widget_id: NodeId,
        _bounds: &Rect,
        _offscreen_handle: &mut Option<ImageHandle>,
        _children: &mut [LayerNode],
        ctx: &mut PaintContext<'_>,
        scene: &impl ScenePaint,
        _w: i32,
        _h: i32,
        _retry_count: &mut u8,
        _dirty_region: &DirtyRegion,
        _dirty_bounds: Rect,
    ) {
        Self::render_widget_and_children_direct(widget_id, ctx, scene);
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
        ctx: &mut PaintContext<'_>,
        scene: &impl ScenePaint,
        depth: usize,
        hovered_chain: &Option<HashSet<NodeId>>,
        debug_mode: bool,
        dirty_region: &DirtyRegion,
        dirty_bounds: Rect,
        force_overlay: bool,
    ) {
        let widget_id = node.widget_id();

        // 判断此 widget 是否需要重新绘制 overlay
        let needs_overlay = force_overlay
            || if debug_mode {
                // 调试模式下始终绘制边框，不跳过
                true
            } else if scene.node_visible(widget_id) {
                let frame = scene.node_frame(widget_id);
                // 使用 dirty_rect（含 draw_margin 扩展）而非 raw frame
                // 确保阴影等扩展区域的 overlay 内容被正确重绘
                let draw_area = scene.dirty_rect(widget_id, frame);
                // dirty_bounds 间隙修正：parent 在脏包围盒内重绘会覆盖 overlay 内容
                dirty_region.intersects(draw_area)
                    || dirty_bounds.intersect(&draw_area).is_some()
            } else {
                false
            };

        // 调用 widget 的 post_render（仅当需要时）
        if needs_overlay {
            if scene.node_visible(widget_id) {
                let frame = scene.node_frame(widget_id);
                ctx.save();
                scene.paint_overlay(widget_id, frame, ctx);
                ctx.restore();
            }
        }

        // 调试模式：悬浮 widget 及其父链显示坐标信息，其余仅极淡边框
        if debug_mode {
            if scene.node_visible(widget_id) {
                let frame = scene.node_frame(widget_id);
                let hovered = hovered_chain
                    .as_ref()
                    .is_some_and(|c| c.contains(&widget_id));
                ctx.draw_debug_border(frame, depth, hovered);
                if hovered {
                    ctx.draw_debug_frame_info(widget_id, frame);
                }
            }
        }

        // 递归子节点（深度 + 1）
        match node {
            LayerNode::Picture { children, .. } | LayerNode::Direct { children, .. } => {
                for child in children {
                    Self::render_overlay_node(
                        child,
                        ctx,
                        scene,
                        depth + 1,
                        hovered_chain,
                        debug_mode,
                        dirty_region,
                        dirty_bounds,
                        force_overlay,
                    );
                }
            }
            LayerNode::ClipRect {
                widget_id,
                rect,
                children,
                ..
            } => {
                ctx.canvas_2d().push_clip(*rect);
                // 应用画布偏移（ScrollView 滚动平移）使子节点 overlay 在内容坐标下绘制
                let scroll_off = Self::get_scroll_offset(scene, *widget_id);
                if let Some((sx, sy)) = scroll_off {
                    ctx.canvas_2d().translate(-sx, -sy);
                }
                // 滚动时强制渲染所有子节点 overlay：子节点 frame 在 content 坐标系，
                // dirty_region 在 viewport 坐标系，坐标不匹配导致脏检查不可靠。
                // clip 已限制实际像素写入范围，不会产生多余绘制。
                let child_force = scroll_off.is_some() || force_overlay;
                for child in children {
                    Self::render_overlay_node(
                        child,
                        ctx,
                        scene,
                        depth + 1,
                        hovered_chain,
                        debug_mode,
                        dirty_region,
                        dirty_bounds,
                        child_force,
                    );
                }
                if let Some((sx, sy)) = scroll_off {
                    ctx.canvas_2d().translate(sx, sy);
                }
                ctx.canvas_2d().pop_clip();
            }
        }
    }

    /// 获取滚动容器的 content 偏移（viewport → content）。
    fn get_scroll_offset(scene: &impl ScenePaint, widget_id: NodeId) -> Option<(f32, f32)> {
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
