// 引入父模块的组件树核心类型。
use super::*;

// 测试只统计当前线程确有隐藏子项时的过滤分配，避免并行测试相互污染。
#[cfg(test)]
thread_local! {
    static FILTERED_CHILD_ALLOCATION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_filtered_child_allocation_count() {
    FILTERED_CHILD_ALLOCATION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn filtered_child_allocation_count() -> usize {
    FILTERED_CHILD_ALLOCATION_COUNT.get()
}

impl BoxedWidget {
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
