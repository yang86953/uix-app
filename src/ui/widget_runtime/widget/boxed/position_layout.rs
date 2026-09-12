// 引入父模块的组件树核心类型。
use super::*;

impl BoxedWidget {
    /// 在树级复用工作区中测量并排列当前节点的可见直接子节点。
    pub(crate) fn layout_children_into(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        scratch: &mut crate::ui::widget_runtime::widget::tree_core::LayoutArrangeScratch,
    ) {
        let crate::ui::widget_runtime::widget::tree_core::LayoutArrangeScratch {
            visible_children,
            measured,
            in_flow,
            out_of_flow,
            positions,
            engine,
        } = scratch;
        let all_visible = children
            .iter()
            .all(|child_id| tree.is_effectively_visible(*child_id));
        visible_children.clear();
        if !all_visible {
            #[cfg(test)]
            let previous_capacity = visible_children.capacity();
            visible_children.extend(
                children
                    .iter()
                    .copied()
                    .filter(|child_id| tree.is_effectively_visible(*child_id)),
            );
            #[cfg(test)]
            if visible_children.capacity() > previous_capacity {
                FILTERED_CHILD_ALLOCATION_COUNT.set(FILTERED_CHILD_ALLOCATION_COUNT.get() + 1);
            }
        }
        let visible = if all_visible {
            children
        } else {
            visible_children.as_slice()
        };
        positions.clear();
        self.with_widget_context(|widget| {
            let Some(layout) = widget.as_layout() else {
                return;
            };
            layout.measure_children_into(frame, visible, tree, measured);
            tree.arrange_positioned_children_into(
                layout,
                frame,
                measured,
                in_flow,
                out_of_flow,
                engine,
                positions,
            );
        });
    }

    // 读取当前节点完整定位元数据。
    pub(crate) const fn position(&self) -> crate::ui::position::PositionedLayout {
        // 返回复制值，避免布局算法取得节点可变所有权。
        self.position
    }

    // 替换当前节点完整定位元数据。
    pub(crate) fn set_position(&mut self, position: crate::ui::position::PositionedLayout) {
        // 保存已经由公开值对象验证的模式与 inset。
        self.position = position;
    }

    // 读取声明节点交付的 min/max 尺寸约束。
    pub(crate) const fn size_constraints(&self) -> crate::ui::theme::style::SizeConstraints {
        self.size_constraints
    }

    // 替换声明节点交付的 min/max 尺寸约束。
    pub(crate) fn set_size_constraints(
        &mut self,
        constraints: crate::ui::theme::style::SizeConstraints,
    ) {
        self.size_constraints = constraints;
    }

    // 测量并排列当前节点的全部有效可见直接子节点。
    pub fn layout_children(
        // 读取当前父组件的布局能力。
        &self,
        // 接收父节点已分配的布局 frame。
        frame: Rect,
        // 接收声明顺序中的全部直接子节点。
        children: &[WidgetId],
        // 接收定位元数据与包含块的只读树快照。
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        // 全部子项可见是常见热路径，直接借用节点切片，避免每个容器、每轮收敛都分配 Vec。
        let filtered_children;
        let visible_children = if children
            .iter()
            .all(|child_id| tree.is_effectively_visible(*child_id))
        {
            children
        } else {
            // 只有确有隐藏子树时才物化过滤结果，并保留声明顺序。
            filtered_children = children
                .iter()
                .copied()
                .filter(|child_id| tree.is_effectively_visible(*child_id))
                .collect::<Vec<_>>();
            #[cfg(test)]
            FILTERED_CHILD_ALLOCATION_COUNT.set(FILTERED_CHILD_ALLOCATION_COUNT.get() + 1);
            filtered_children.as_slice()
        };
        // 在当前组件的 Provider 上下文中调用其布局能力。
        self.with_widget_context(|widget| {
            // 没有布局能力的组件不产生子节点位置。
            widget
                // 读取可选布局能力。
                .as_layout()
                // 对存在的布局能力执行统一定位协调。
                .map(|layout| {
                    // 测量阶段不得再观察或缓存隐藏子节点。
                    let measured = layout.measure_children(frame, visible_children, tree);
                    // 由组件树统一移除 out-of-flow 子项并追加其定位结果。
                    tree.arrange_positioned_children(layout, frame, measured)
                })
                // 没有布局能力时返回稳定空集合。
                .unwrap_or_default()
        })
    }
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../../../tests-src/ui/widget_runtime/widget/boxed/position_layout_tests.rs"]
mod position_layout_tests;

// 以下完整测试实现位于 tests-src，经模块级 include! 保持原私有作用域；
// 文件内各项自带 cfg(test)，生产构建展开为空。
#[cfg(test)]
include!("../../../../../tests-src/ui/widget_runtime/widget/boxed/position_layout_test_fns.rs");
