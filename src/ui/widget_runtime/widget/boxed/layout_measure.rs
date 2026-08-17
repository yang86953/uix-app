// 透明包装节点的同轮子测量继续由原 BoxedWidget 所有。
impl super::BoxedWidget {
    // 在统一组件上下文中请求可选的直接子节点代理测量。
    pub fn measure_from_children(
        // 借用当前透明包装节点。
        &self,
        // 传入父级分配给包装节点的约束。
        constraints: crate::core::Constraints,
        // 传入持有全部直接子节点的单窗组件树。
        tree: &crate::ui::widget_runtime::widget::WidgetTree,
        // 没有代理契约时返回空并继续使用组件自身测量。
    ) -> Option<crate::core::Size> {
        // 保持代理测量与普通组件测量相同的组件上下文边界。
        self.with_component_context(|component| {
            // 只有布局组件才能声明代理测量。
            component.as_layout().and_then(|layout| {
                // 将真实直接子身份交给布局组件，但不借出树的可变所有权。
                layout.measure_from_children(constraints, &self.children, tree)
            })
        })
    }
}
