//! 树对具体组件语义的访问点（System 私有边界）。
//!
//! # SMC 边界（SMC-04）
//!
//! 组件树的通用遍历/布局/事件代码（widget Module）不得引用具体组件
//! （widgets Module）。本文件把这些「树 → 具体组件」的语义访问点集中到
//! System 私有边界：widget 经本边界消费 widgets 提供的窄语义
//! （Modal 生命周期、viewport 滚动轴、显式尺寸锁、导航兄弟联动），
//! 依赖方向为 `widget → System 私有边界 → widgets`，与
//! `tree_dynamic` / `render_handler` / `adapter` 同类。

use crate::ui::widget_runtime::widget::{WidgetCore, WidgetId, WidgetTree};
use crate::ui::widgets::window_chrome::WindowInteractionRegion;
use crate::ui::widgets::{Container, Grid, ScrollView, Space};
// 反馈 capability 启用时才引入 Modal 与 Popconfirm 生命周期类型。
#[cfg(feature = "feedback")]
use crate::ui::widgets::{Modal, Popconfirm, Popover};
// 导航 capability 启用时才引入兄弟项联动与 Tabs 焦点迁移所需类型。
#[cfg(feature = "navigation")]
use crate::ui::widgets::{Dropdown, MenuBar, NavItem, Tabs};

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
                .widget()
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
    let widget = node.widget().as_any();
    if let Some(container) = widget.downcast_ref::<Container>() {
        return (
            container.style.width.is_some(),
            container.style.height.is_some(),
        );
    }
    if let Some(grid) = widget.downcast_ref::<Grid>() {
        return (grid.style.width.is_some(), grid.style.height.is_some());
    }
    if let Some(space) = widget.downcast_ref::<Space>() {
        return space.explicit_size_locks();
    }
    widget
        .downcast_ref::<ScrollView>()
        .map(ScrollView::explicit_size_locks)
        .unwrap_or((false, false))
}

/// 节点当前是否为在场（打开）的 Modal。
pub(crate) fn modal_was_present(tree: &WidgetTree, id: WidgetId) -> bool {
    // 反馈能力启用时保留 Modal 在场状态探测。
    #[cfg(feature = "feedback")]
    {
        tree.get(id)
            .and_then(|node| node.widget().as_any().downcast_ref::<Modal>())
            .is_some_and(Modal::is_present)
    }
    // 关闭反馈能力时通用动画流程不再识别 Modal。
    #[cfg(not(feature = "feedback"))]
    {
        let _ = (tree, id);
        false
    }
}

/// 节点为「关闭动画结束、应销毁」的 Modal。
pub(crate) fn modal_closed_for_destruction(tree: &WidgetTree, id: WidgetId) -> bool {
    // 反馈能力启用时保留 Modal 关闭销毁状态探测。
    #[cfg(feature = "feedback")]
    {
        tree.get(id).is_some_and(|node| {
            node.widget()
                .as_any()
                .downcast_ref::<Modal>()
                .is_some_and(|modal| modal.should_destroy_on_close() && !modal.is_present())
        })
    }
    // 关闭反馈能力时通用动画流程不产生 Modal 销毁请求。
    #[cfg(not(feature = "feedback"))]
    {
        let _ = (tree, id);
        false
    }
}

/// 语义路径上存在请求上下文关闭的 Modal 时，逐个关闭并复位激活态。
pub(crate) fn apply_modal_context_requests(tree: &mut WidgetTree, path: &[WidgetId]) {
    // 反馈能力启用时处理语义路径上的 Modal 上下文关闭请求。
    #[cfg(feature = "feedback")]
    {
        let requested: Vec<WidgetId> = path
            .iter()
            .copied()
            .filter(|&id| {
                tree.get(id)
                    .and_then(|node| node.widget().as_any().downcast_ref::<Modal>())
                    .is_some_and(|modal| modal.take_context_close_request())
            })
            .collect();
        if requested.is_empty() {
            return;
        }

        for id in requested {
            if let Some(node) = tree.get_mut(id) {
                if let Some(modal) = node.widget_mut().as_any_mut().downcast_mut::<Modal>() {
                    modal.close();
                    node.set_active(true);
                }
            }
        }
        tree.mark_full_frame_dirty();
        tree.rebuild_widget_overlays();
    }
    // 关闭反馈能力时保留通用语义调用点并退化为空操作。
    #[cfg(not(feature = "feedback"))]
    {
        let _ = (tree, path);
    }
}

/// 把浮层外部点击转换为具体反馈组件拥有的用户取消动作。
pub(crate) fn dismiss_overlay_owner_from_outside(tree: &mut WidgetTree, owner: WidgetId) {
    // 反馈 capability 启用时通知 Popconfirm/Popover 的唯一取消端口。
    #[cfg(feature = "feedback")]
    {
        // 只在 owner 仍可寻址且确实为 Popconfirm 时执行。
        if let Some(popconfirm) = tree
            // 获取浮层 owner 节点的可变引用。
            .get_mut(owner)
            // 下转到具体反馈组件。
            .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Popconfirm>())
        {
            // 用户外部点击必须执行一次 @cancel 后再离场。
            popconfirm.cancel_action();
        }
        // 同一取消端口同时服务 Popover：外部点击直接关闭弹层。
        if let Some(popover) = tree
            // 获取浮层 owner 节点的可变引用。
            .get_mut(owner)
            // 下转到具体反馈组件。
            .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Popover>())
        {
            // 用户外部点击关闭已打开的 Popover 弹层。
            popover.close();
        }
    }
    // 导航 capability 启用时通知 Dropdown 的外部点击关闭端口。
    #[cfg(feature = "navigation")]
    {
        // 只在 owner 仍可寻址且确实为 Dropdown 时执行。
        if let Some(dropdown) = tree
            // 获取浮层 owner 节点的可变引用。
            .get_mut(owner)
            // 下转到具体导航组件。
            .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Dropdown>())
        {
            // 用户外部点击关闭已打开的 Dropdown 菜单。
            dropdown.close();
        }
        // 同一取消端口同时服务 MenuBar：外部点击关闭已开菜单。
        if let Some(menu_bar) = tree
            // 获取浮层 owner 节点的可变引用。
            .get_mut(owner)
            // 下转到具体导航组件。
            .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<MenuBar>())
        {
            // 用户外部点击关闭菜单栏的已开菜单。
            menu_bar.close();
        }
    }
    // 反馈与导航能力均关闭时保留通用调用点并退化为空操作。
    #[cfg(not(any(feature = "feedback", feature = "navigation")))]
    {
        // 消费参数以避免能力裁剪构建产生告警。
        let _ = (tree, owner);
    }
}

/// 返回具体组合组件在指针动作后应保持的焦点目标。
pub(crate) fn pointer_focus_target(tree: &WidgetTree, target: WidgetId) -> WidgetId {
    // 反馈能力启用时为组合 Popconfirm 恢复真实 trigger 焦点。
    #[cfg(feature = "feedback")]
    {
        // 判断当前指针目标是否为在场的组合 Popconfirm owner。
        let restores_trigger = tree.get(target).is_some_and(|node| {
            // 下转到具体反馈组件并读取窄语义。
            node.widget()
                .as_any()
                .downcast_ref::<Popconfirm>()
                // 离场期间仍需把气泡按钮点击后的焦点交还 trigger。
                .is_some_and(|popconfirm| {
                    popconfirm.uses_custom_trigger() && popconfirm.is_present()
                })
        });
        // 只有组合 owner 需要代理到第一个真实可聚焦子节点。
        if restores_trigger {
            // 返回 trigger 子树内第一个有效 Tab 目标。
            if let Some(trigger) = tree.collect_focusable_within(target).into_iter().next() {
                // 直接使用完整 trigger 子树拥有的焦点身份。
                return trigger;
            }
        }
    }
    // 关闭反馈能力或非组合 owner 保持通用目标。
    target
}

/// 文本输入组件的当前值（语义快照 value_text 用）。
pub(crate) fn widget_input_value(
    widget: &dyn crate::ui::widget_runtime::traits::Widget,
) -> Option<String> {
    widget
        .as_any()
        .downcast_ref::<crate::ui::widgets::Input>()
        .map(|input| input.current_value().to_owned())
}

/// 节点是否为窗口交互拖拽区域（自定义标题栏拖拽）。
pub(crate) fn is_drag_region(tree: &WidgetTree, id: WidgetId) -> bool {
    tree.get(id).is_some_and(|node| {
        node.widget()
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
            node.widget().as_any().type_id() == std::any::TypeId::of::<NavItem>()
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

/// 父级可见性切换隐藏当前焦点时，返回具体组件约定的替代焦点。
pub(crate) fn focus_replacement_after_child_visibility(
    tree: &WidgetTree,
    focused: WidgetId,
    changes: &[(WidgetId, bool)],
) -> Option<WidgetId> {
    // 导航能力启用时执行 Tabs 面板切换的焦点迁移约定。
    #[cfg(feature = "navigation")]
    {
        // 只处理包含旧焦点且刚被隐藏的直接面板。
        let hidden_panel = changes.iter().find_map(|(child, visible)| {
            // 可见面板不可能是旧焦点失效的来源。
            if *visible || !tree.is_descendant_of(focused, *child) {
                // 继续检查其余可见性变化。
                return None;
            }
            // 返回被隐藏的直接面板标识。
            Some(*child)
        })?;
        // 读取被隐藏面板的父节点。
        let tabs_id = tree.get(hidden_panel)?.parent()?;
        // 确认父节点确实是 Tabs，避免改变其他容器的通用行为。
        let tabs = tree
            .get(tabs_id)?
            .widget()
            .as_any()
            .downcast_ref::<Tabs>()?;
        // 找到切换后处于活动状态的直接面板。
        let active_panel = tree
            .get(tabs_id)?
            .children()
            .get(tabs.active_index())
            .copied();
        // 优先把焦点迁移到新面板内第一个可聚焦控件。
        if let Some(target) =
            active_panel.and_then(|panel| tree.collect_focusable_within(panel).into_iter().next())
        {
            // 返回新面板内的首个可聚焦目标。
            return Some(target);
        }
        // 新面板没有可聚焦控件时退回 Tabs 标签栏自身。
        tree.get(tabs_id)
            .is_some_and(|node| node.is_focusable() && tree.focus_target_available(tabs_id))
            .then_some(tabs_id)
    }
    // 关闭导航能力时消费参数并保留通用清焦点行为。
    #[cfg(not(feature = "navigation"))]
    {
        // 避免无导航构建产生未使用参数告警。
        let _ = (tree, focused, changes);
        // 不为其他组件提供替代焦点。
        None
    }
}
