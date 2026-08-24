//! WidgetTree 的 ScenePaint 实现 — UI 与 draw compositor 的桥接。

use std::collections::HashSet;

use crate::core::DirtyRegion;
use crate::core::{Point, Rect, WidgetId};
use crate::draw::painting::PaintContext;
use crate::draw::scene::NodeId;
use crate::draw::scene::PicturePolicy;
use crate::draw::scene::ScenePaint;
use crate::draw::scene::{HoverInspectorNode, HoverInspectorSnapshot};
use crate::ui::reactive::state::StateBindCaptureGuard;
use crate::ui::widget_runtime::paint_scope::PaintWidgetScope;
use crate::ui::widget_runtime::widget::WidgetCore;
use crate::ui::widget_runtime::widget::WidgetTree;

impl ScenePaint for WidgetTree {
    fn root_id(&self) -> Option<NodeId> {
        // 非运行态树不得把协调中或已停止的半提交根交给合成器。
        if !self.accepts_external_work() {
            // 以无根场景阻止后续节点遍历。
            return None;
        }
        self.root_id()
    }

    fn tree_version(&self) -> u64 {
        self.tree_version()
    }

    fn dirty_region(&self) -> DirtyRegion {
        self.dirty_region()
    }

    fn node_visible(&self, id: NodeId) -> bool {
        // 非运行态树不得暴露节点可见性。
        if !self.accepts_external_work() {
            // 所有节点在故障停止态对合成器均不可见。
            return false;
        }
        self.get(id).is_some_and(|n| n.visible())
    }

    fn node_frame(&self, id: NodeId) -> Rect {
        self.get(id).map(|n| n.frame()).unwrap_or_default()
    }

    fn node_dirty(&self, id: NodeId) -> bool {
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .node_needs_paint(id)
    }

    fn paint_invalidation_snapshot_into(&self, ids: &mut HashSet<NodeId>) -> Option<bool> {
        let invalidation = self
            .invalidation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        invalidation.paint_ids_into(ids);
        Some(invalidation.needs_full_frame())
    }

    fn node_z_index(&self, id: NodeId) -> i32 {
        self.get(id).map(|n| n.z_index()).unwrap_or(0)
    }

    fn node_transform(&self, id: NodeId) -> crate::draw::Transform {
        // 让合成器、命中和脏区共享 relative/sticky 最终变换。
        self.positioned_visual_transform(id)
    }

    fn node_opacity(&self, id: NodeId) -> f32 {
        self.get(id)
            .map(|node| node.view_transition_opacity())
            .unwrap_or(1.0)
    }

    fn node_children(&self, id: NodeId) -> &[NodeId] {
        static EMPTY: &[NodeId] = &[];
        // 非运行态树不得暴露半提交的子节点关系。
        if !self.accepts_external_work() {
            // 对合成器返回稳定的空子集。
            return EMPTY;
        }
        self.get(id).map(|n| n.children()).unwrap_or(EMPTY)
    }

    // 暴露布局阶段存入节点的父级片段裁剪快照。
    fn node_clip_regions(&self, id: NodeId) -> Option<Vec<Rect>> {
        // 只在节点仍存在时读取其片段元数据。
        self.get(id)
            // 克隆小型矩形集合交给合成树独立消费。
            .and_then(|node| node.parent_clip_regions())
    }

    fn node_picture_policy(&self, id: NodeId) -> PicturePolicy {
        // 停止树不得查询组件的 Picture 缓存策略。
        if !self.accepts_external_work() {
            // 禁止半提交节点进入离屏 Picture 路径。
            return PicturePolicy::Never;
        }
        self.get(id)
            .map(|n| {
                if n.view_transition_active() {
                    PicturePolicy::Never
                } else {
                    n.picture_policy()
                }
            })
            .unwrap_or(PicturePolicy::Never)
    }

    fn node_has_semantic_handlers(&self, id: NodeId) -> bool {
        self.get(id)
            .is_some_and(|n| !n.handler_signatures().is_empty())
    }

    fn node_has_dynamic_content(&self, id: NodeId) -> bool {
        // 停止树不得读取组件声明的动态内容能力。
        if !self.accepts_external_work() {
            // 对合成器保守报告不存在动态内容。
            return false;
        }
        self.get(id).is_some_and(|n| n.has_dynamic_content())
    }

    fn node_has_interactive_state(&self, id: NodeId) -> bool {
        self.get(id).is_some_and(|n| n.has_interactive_state())
    }

    fn node_wants_continuous_pointer_move(&self, id: NodeId) -> bool {
        // 停止树不得查询组件的连续指针事件能力。
        if !self.accepts_external_work() {
            // 对合成器保守报告不需要连续指针更新。
            return false;
        }
        self.get(id)
            .is_some_and(|n| n.wants_continuous_pointer_move())
    }

    fn node_is_overlay(&self, id: NodeId) -> bool {
        // 停止树不得通过组件浮层回调读取覆盖层元数据。
        if !self.accepts_external_work() {
            // 对合成器保守报告该节点不是浮层。
            return false;
        }
        // fixed 节点作为合成浮层脱离祖先滚动和裁剪，同时保留树生命周期。
        self.node_is_fixed(id)
            || self
                // 读取组件自身声明的浮层入口。
                .get(id)
                // 普通浮层继续使用既有条目判定。
                .is_some_and(|n| n.overlay_entry(id, n.frame()).is_some())
    }

    fn node_overlay_transform(&self, id: NodeId) -> crate::draw::Transform {
        let uses_viewport_coordinates = self.node_is_fixed(id)
            || self
                // Modal、Drawer 与图片预览会直接按逻辑表面绘制，不能再叠加锚点滚动。
                .get(id)
                // 使用无需表面参数的条目只判定坐标语义；真实 bounds 仍由布局阶段解析。
                .and_then(|node| node.overlay_entry(id, node.frame()))
                .is_some_and(|entry| {
                    matches!(
                        entry.kind(),
                        crate::ui::OverlayKind::Modal | crate::ui::OverlayKind::Drawer
                    )
                });
        if uses_viewport_coordinates {
            // 窗口级浮层保持视口坐标语义，不继承祖先滚动。
            self.positioned_visual_transform(id)
        } else {
            // 输入类弹层仍锚定原组件树，补回被根浮层提升跳过的祖先变换。
            crate::draw::scene::viewport_transform::overlay_root_visual_transform(self, id)
        }
    }

    fn node_requires_overlay_backdrop(&self, id: NodeId) -> bool {
        // 只有模态遮罩或显式 blur 需要跨帧保留不含浮层的干净背景。
        // Message、Notification、Tooltip 等局部浮层直接在 retained 主表面按 damage 重绘。
        self.overlay_stack().iter().any(|entry| {
            entry.owner() == id && (entry.is_modal() || entry.backdrop_blur_value().is_some())
        })
    }

    // 把 UI overlay 栈与当前 Theme token 解析为 draw System 的唯一效果计划。
    fn overlay_backdrop_effect(&self) -> Option<crate::draw::OverlayBackdropEffect> {
        // 停止树不得暴露半提交 overlay 或主题状态。
        if !self.accepts_external_work() {
            // 故障/停止态不请求任何效果事务。
            return None;
        }
        // 读取本帧已安装的主题 token。
        let theme_radius = self.theme_tokens().backdrop_blur_radius();
        // 由 OverlayStack 聚合所有显式请求。
        self.overlay_stack().backdrop_effect(theme_radius)
    }

    // 把组件显式二阶段绘制契约投影给 draw System。
    fn node_paints_after_children(&self, id: NodeId) -> bool {
        // 停止树不得读取组件渲染能力。
        if !self.accepts_external_work() {
            // 故障或停止态不允许二阶段回调。
            return false;
        }
        // 只在节点仍存在时读取显式能力位。
        self.get(id)
            // 从 UI 组件的渲染 capability 读取二阶段声明。
            .and_then(|node| node.widget().as_render())
            // 没有节点或渲染能力时保持安全默认值。
            .is_some_and(|render| render.paint_after_children())
    }

    fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect> {
        // 停止树不得通过组件渲染契约计算子树裁剪。
        if !self.accepts_external_work() {
            // 返回无裁剪以阻止读取半提交组件元数据。
            return None;
        }
        self.get(id).and_then(|n| n.children_clip(frame))
    }

    fn dirty_rect(&self, id: NodeId, frame: Rect) -> Rect {
        // 停止树不得通过组件渲染契约扩展脏矩形。
        if !self.accepts_external_work() {
            // 保留调用方给定 frame 作为安全的局部默认值。
            return frame;
        }
        self.get(id).map(|n| n.dirty_rect(frame)).unwrap_or(frame)
    }

    fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
        // 停止树不得通过组件事件契约读取滚动偏移。
        if !self.accepts_external_work() {
            // 对合成器报告不存在可用滚动状态。
            return None;
        }
        self.get(id).and_then(|n| n.viewport_scroll_offset())
    }

    fn focused_node(&self) -> Option<NodeId> {
        self.managers().focus.focused_widget()
    }

    fn node_focusable(&self, id: NodeId) -> bool {
        // 停止树不得经可聚焦性计算读取组件可见性契约。
        if !self.accepts_external_work() {
            // 对合成器保守报告节点不可聚焦。
            return false;
        }
        self.get(id).is_some_and(|n| n.is_focusable())
    }

    fn hit_test(&self, pos: Point) -> Option<NodeId> {
        self.hit_test(pos)
    }

    fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.parent())
    }

    fn hover_inspector(&self, leaf: NodeId) -> Option<HoverInspectorSnapshot> {
        // 故障停止态不得通过检查器读取半提交树。
        if !self.accepts_external_work() || self.get(leaf).is_none() {
            return None;
        }
        // 从叶向根收集，并用访问集合防御损坏父链导致无限循环。
        let mut path = Vec::new();
        let mut visited = HashSet::new();
        let mut current = Some(leaf);
        while let Some(id) = current {
            if !visited.insert(id) {
                return None;
            }
            let node = self.get(id)?;
            path.push(id);
            current = node.parent();
        }
        path.reverse();

        // 交互状态与失效状态各读取一次共享事实，避免逐节点重复加锁。
        let managers = self.managers();
        let hovered = managers.interaction.hovered_widget();
        let pressed = managers.interaction.pressed_widget();
        let focused = managers.focus.focused_widget();
        let invalidation = self
            .invalidation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let nodes = path
            .into_iter()
            .filter_map(|id| {
                let node = self.get(id)?;
                let stable_id = node
                    .automation_id()
                    .map(|value| format!("automation:{value}"))
                    .or_else(|| node.key().map(|value| format!("key:{value}")))
                    .unwrap_or_else(|| id.to_string());
                Some(HoverInspectorNode {
                    node_id: id,
                    type_name: node.debug_type_name(),
                    stable_id,
                    frame: node.frame(),
                    z_index: node.z_index(),
                    child_count: node.children().len(),
                    visible: node.visible(),
                    dirty: invalidation.node_needs_paint(id),
                    hovered: hovered == Some(id),
                    pressed: pressed == Some(id),
                    focused: focused == Some(id),
                    disabled: !node.is_interaction_enabled(),
                    attached: node.attached(),
                    mounted: node.mounted(),
                    active: node.active(),
                    pending_removal: node.pending_removal(),
                })
            })
            .collect();
        Some(HoverInspectorSnapshot { nodes })
    }

    fn paint(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext) {
        // 非运行态树不得执行节点 render 回调。
        if !self.accepts_external_work() {
            // 保留故障现场并跳过绘制。
            return;
        }
        if let Some(node) = self.get(id) {
            if node.visible() {
                let dirty = node.dirty_rect(frame);
                let paint_rect = (dirty.w > 0.0 && dirty.h > 0.0)
                    .then_some(dirty)
                    .and_then(|rect| self.node_visual_rect(id, rect));
                // 建立可在用户绘制 panic 时自动恢复的响应式捕获作用域。
                let capture =
                    StateBindCaptureGuard::begin(id, self.invalidation_handle(), paint_rect);
                // 建立可嵌套且在异常展开时恢复的当前绘制节点作用域。
                let _paint_widget_scope = PaintWidgetScope::enter(id);
                let theme_tokens = self.theme_tokens();
                let mut ui_ctx =
                    crate::ui::widget_runtime::paint_context::PaintContext::new(ctx, theme_tokens);
                node.render(frame, &mut ui_ctx, self);
                // 正常绘制完成后将本轮依赖转换为节点所有的租约。
                let leases = capture.finish();
                // 整体替换旧租约，依赖变化时同步解绑不再读取的源。
                node.replace_paint_state_binds(leases);
            }
        }
    }
}

// WidgetId 与 NodeId 同型
const _: () = assert!(std::mem::size_of::<WidgetId>() == std::mem::size_of::<NodeId>());
