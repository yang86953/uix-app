// 将命令式 WidgetNode 转换为可嵌入声明 View 的等价节点。
pub(super) fn adopt_widget_node(
    // 接收已物化的命令式节点。
    node: crate::ui::widget_runtime::widget::WidgetNode,
    // 返回保存运行时元数据的声明节点。
) -> crate::ui::view::ViewNode {
    // 保留子组件构建所需的提供者上下文。
    let provider_context = node.provider_context.clone();
    // 在原提供者上下文内物化命令式组件声明的子节点。
    let mut children: Vec<crate::ui::widget_runtime::widget::WidgetNode> =
        // 让隐式组件子节点继承原节点的上下文。
        crate::ui::widget_runtime::provider_context::with_provider_context(&provider_context, || {
            // 取出命令式组件声明的直接子组件。
            node.widget
                // 构建组件声明的子节点。
                .build()
                // 遍历构建出的子组件集合。
                .into_iter()
                // 将每个子组件包装为运行时叶节点。
                .map(crate::ui::widget_runtime::widget::WidgetNode::leaf)
                // 收集为可继续追加的节点列表。
                .collect::<Vec<_>>()
        });
    // 追加调用方已经显式提供的运行时子节点。
    children.extend(node.children);
    // 构造保留命令式运行时元数据的声明节点。
    crate::ui::view::ViewNode {
        // 转移命令式节点的组件实例。
        widget: node.widget,
        // 递归转换全部运行时子节点。
        children: children.into_iter().map(adopt_widget_node).collect(),
        // 转移节点已经携带的结构性 State 输出。
        captured_state_binds: node.captured_state_binds,
        // 命令式节点不携带子树作用域重建工厂。
        scoped_rebuild: None,
        // 转移节点已经携带的节点生命周期 Effect 输出。
        captured_effects: node.captured_effects,
        // 命令式节点没有声明构建期的动画源输出。
        animated_sources: Vec::new(),
        // 保留提供者上下文。
        provider_context,
        // 保留默认声明样式。
        style: crate::ui::theme::style::Style::default(),
        // 命令式节点没有显式样式声明。
        style_decl: crate::ui::theme::style::StyleDiff::default(),
        // 保留运行时视觉变换。
        visual_transform: node.visual_transform,
        // 保留命令式节点声明的定位元数据。
        position: node.position,
        // 保留命令式节点声明的文字选择策略。
        user_select: node.user_select,
        // 保留运行时节点显式声明的可继承光标。
        cursor: node.cursor,
        // 保留入场动画配置。
        enter_animation: node.enter_animation,
        // 保留入场截止时间。
        enter_deadline: node.enter_deadline,
        // 保留离场动画配置。
        leave_animation: node.leave_animation,
        // 命令式节点没有声明交错配置。
        stagger_enter: None,
        // 命令式节点没有声明弹性增长覆盖。
        flex_grow_override: None,
        // 命令式节点没有声明弹性收缩覆盖。
        flex_shrink_override: None,
        // 保留运行时层叠顺序。
        z_index: node.z_index,
        // 保留可选身份键。
        key: node.key.map(|key| key.to_string()),
        // 保留可选自动化标识。
        automation_id: node.automation_id.map(|id| id.to_string()),
        // 优先保留显式 tab 覆盖，否则恢复非零历史索引。
        tab_index: node
            // 读取运行时显式 tab 覆盖。
            .tab_index_override
            // 从非零旧索引恢复兼容值。
            .or_else(|| (node.tab_idx != 0).then_some(node.tab_idx)),
        // 保留焦点句柄。
        focus_handle: node.focus_handle,
        // 保留可访问性覆盖。
        accessibility_override: node.accessibility_override,
        // 转移普通事件处理器。
        handlers: node.handlers,
        // 转移系统事件处理器。
        system_event_handlers: node.system_event_handlers,
        // 转移渲染处理器。
        render_handlers: node.render_handlers,
        // 保留嵌入节点携带的内联组件状态作用域。
        uix_widget_scopes: node.uix_widget_scopes,
        // 命令式节点不拥有声明根状态存储。
        widget_state_store: None,
        // 命令式节点没有声明捕获产生的待提交组件状态回执。
        widget_state_receipts: Vec::new(),
        // 命令式节点没有声明捕获产生的 @media 断点。
        captured_media_breakpoints: Vec::new(),
        captured_viewport_width: None,
    }
}
