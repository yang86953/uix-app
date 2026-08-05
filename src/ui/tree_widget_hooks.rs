//! 树对具体组件语义的访问点（System 私有边界）。
//!
//! # SMC 边界（SMC-04）
//!
//! 组件树的通用遍历/布局/事件代码（component Module）不得引用具体组件
//! （widgets Module）。本文件把这些「树 → 具体组件」的语义访问点集中到
//! System 私有边界：component 经本边界消费 widgets 提供的窄语义
//! （Modal 生命周期、viewport 滚动轴、显式尺寸锁、导航兄弟联动），
//! 依赖方向为 `component → System 私有边界 → widgets`，与
//! `tree_dynamic` / `render_handler` / `adapter` 同类。

use crate::ui::component::widget::{WidgetCore, WidgetId, WidgetTree};
use crate::ui::widgets::window_chrome::WindowInteractionRegion;
use crate::ui::widgets::{Container, Grid, Modal, ScrollView, Space};
// 导航 capability 启用时才引入兄弟项联动所需类型。
#[cfg(feature = "navigation")]
use crate::ui::widgets::NavItem;

/// 最近 viewport 祖先允许内容溢出的轴。
///
/// 最近 viewport 决定当前内容坐标系：嵌套 ScrollView 时不能越过内层
/// viewport，错误地继承外层的滚动方向。未知 viewport 保守地禁止溢出。
pub(crate) fn nearest_viewport_overflow_axes(
    tree: &WidgetTree,
    id: WidgetId,
) -> Option<(bool, bool)> {
    let mut current = id;
    while let Some(parent_id) = tree.get(current).and_then(|node| node.parent()) {
        let parent = tree.get(parent_id)?;
        if parent.children_clip(parent.frame()).is_some() {
            let axes = parent
                .component()
                .as_any()
                .downcast_ref::<ScrollView>()
                .map(|scroll_view| {
                    let direction = scroll_view.scroll_direction();
                    (direction.can_scroll_x(), direction.can_scroll_y())
                })
                .unwrap_or((false, false));
            return Some(axes);
        }
        current = parent_id;
    }
    None
}

/// Phase 2 不得撑开显式定宽/定高的节点（Container/Space/ScrollView 等）。
pub(crate) fn phase2_explicit_size_locks(tree: &WidgetTree, id: WidgetId) -> (bool, bool) {
    let Some(node) = tree.get(id) else {
        return (false, false);
    };
    let component = node.component().as_any();
    if let Some(container) = component.downcast_ref::<Container>() {
        return (
            container.style.width.is_some(),
            container.style.height.is_some(),
        );
    }
    if let Some(grid) = component.downcast_ref::<Grid>() {
        return (grid.style.width.is_some(), grid.style.height.is_some());
    }
    if let Some(space) = component.downcast_ref::<Space>() {
        return space.explicit_size_locks();
    }
    component
        .downcast_ref::<ScrollView>()
        .map(ScrollView::explicit_size_locks)
        .unwrap_or((false, false))
}

/// 节点当前是否为在场（打开）的 Modal。
pub(crate) fn modal_was_present(tree: &WidgetTree, id: WidgetId) -> bool {
    tree.get(id)
        .and_then(|node| node.component().as_any().downcast_ref::<Modal>())
        .is_some_and(Modal::is_present)
}

/// 节点为「关闭动画结束、应销毁」的 Modal。
pub(crate) fn modal_closed_for_destruction(tree: &WidgetTree, id: WidgetId) -> bool {
    tree.get(id).is_some_and(|node| {
        node.component()
            .as_any()
            .downcast_ref::<Modal>()
            .is_some_and(|modal| modal.should_destroy_on_close() && !modal.is_present())
    })
}

/// 语义路径上存在请求上下文关闭的 Modal 时，逐个关闭并复位激活态。
pub(crate) fn apply_modal_context_requests(tree: &mut WidgetTree, path: &[WidgetId]) {
    let requested: Vec<WidgetId> = path
        .iter()
        .copied()
        .filter(|&id| {
            tree.get(id)
                .and_then(|node| node.component().as_any().downcast_ref::<Modal>())
                .is_some_and(|modal| modal.take_context_close_request())
        })
        .collect();
    if requested.is_empty() {
        return;
    }

    for id in requested {
        if let Some(node) = tree.get_mut(id) {
            if let Some(modal) = node.component_mut().as_any_mut().downcast_mut::<Modal>() {
                modal.close();
                node.set_active(true);
            }
        }
    }
    tree.mark_full_frame_dirty();
    tree.rebuild_widget_overlays();
}

/// 文本输入组件的当前值（语义快照 value_text 用）。
pub(crate) fn component_input_value(
    component: &dyn crate::ui::component::traits::WidgetComponent,
) -> Option<String> {
    component
        .as_any()
        .downcast_ref::<crate::ui::widgets::Input>()
        .map(|input| input.current_value().to_owned())
}

/// 节点是否为窗口交互拖拽区域（自定义标题栏拖拽）。
pub(crate) fn is_drag_region(tree: &WidgetTree, id: WidgetId) -> bool {
    tree.get(id).is_some_and(|node| {
        node.component()
            .as_any()
            .downcast_ref::<WindowInteractionRegion>()
            .is_some_and(WindowInteractionRegion::is_drag_region)
    })
}
/// NavItem 共享 active 索引时，刷新整组导航项（取消/选中态联动）。
pub(crate) fn invalidate_nav_siblings(tree: &mut WidgetTree, clicked: WidgetId) {
    // 导航能力启用时保留 NavItem 共享状态的整组重绘语义。
    #[cfg(feature = "navigation")]
    {
        let is_nav = tree.get(clicked).is_some_and(|node| {
            node.component().as_any().type_id() == std::any::TypeId::of::<NavItem>()
        });
        if !is_nav {
            return;
        }
        if let Some(parent) = tree.get(clicked).and_then(|node| node.parent()) {
            tree.invalidate_paint_subtree(parent);
        }
    }
    // 关闭导航能力时消费参数并退化为空操作，保持通用事件路径稳定。
    #[cfg(not(feature = "navigation"))]
    {
        let _ = (tree, clicked);
    }
}
