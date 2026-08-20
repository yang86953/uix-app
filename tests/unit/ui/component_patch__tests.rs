    // 复用当前模块的快照类型与分类函数。
    use super::*;
    // 引入组合 Badge 测试使用的真实子 ViewNode。
    use crate::ui::view::ViewNode;
    // 引入 Collapse 受控稳定 key 测试使用的响应式状态。
    use crate::ui::State;
    // 引入纯绘制文本装饰契约。
    use crate::ui::theme::style::TextDecoration;

    // 验证文本装饰变化不会错误触发布局失效。
    #[test]
    fn text_decoration_change_is_paint_only() {
        // 构造未声明文本装饰的基础样式。
        let current = Style::default();
        // 构造只改变下划线绘制的下一样式。
        let next = Style::default().with_text_decoration(TextDecoration::Underline);
        // 纯装饰变化不得进入测量或放置失效。
        assert!(!style_layout_changed(&current, &next));
        // 完整样式仍然不同，协调器可以据此触发重绘。
        assert_ne!(current, next);
    }

    // 验证文本对齐只改变组件内部字形位置而不改变盒模型测量。
    #[test]
    fn text_align_change_is_paint_only() {
        // 构造未声明文本对齐的基础样式。
        let current = Style::default();
        // 构造只改变右对齐绘制的下一样式。
        let next = Style::default().with_text_align(crate::ui::theme::style::TextAlign::Right);
        // 内部字形平移不得触发父级测量或放置失效。
        assert!(!style_layout_changed(&current, &next));
        // 完整样式仍然不同，协调器可以据此触发重绘。
        assert_ne!(current, next);
    }

    // 验证 Badge 配置比较忽略布局后运行态但仍识别作者字段变化。
    #[test]
    // 测试名称陈述组合子存在事实与装饰配置边界。
    fn badge_config_comparison_ignores_runtime_child_fact() {
        // 构造已经挂载唯一真实子节点的当前 Badge。
        let mut current = Badge::new()
            // 使用稳定数字配置。
            .count(3)
            // 使用真实按钮子 View。
            .child(ViewNode::leaf(Button::new("通知")));
        // 模拟组件树登记唯一子节点，令 child_present 成为 true。
        WidgetComponent::on_children_changed(&mut current, 1);
        // 构造作者配置相同但尚未挂载子树的新声明。
        let next = Badge::new()
            // 保持数字配置不变。
            .count(3)
            // 保持组合模式不变。
            .child(ViewNode::leaf(Button::new("通知")));
        // 运行时 child_present 差异不得伪造作者配置变化。
        assert_eq!(
            builtin_widget_config_changed(&current.snapshot_fields(), &next.snapshot_fields()),
            Some(false)
        );
        // 改变作者计数必须仍被配置比较识别。
        let changed = Badge::new()
            // 使用不同数字配置。
            .count(4)
            // 保持组合模式与子形状相同。
            .child(ViewNode::leaf(Button::new("通知")));
        // 作者字段变化必须产生配置失效。
        assert_eq!(
            builtin_widget_config_changed(&current.snapshot_fields(), &changed.snapshot_fields()),
            Some(true)
        );
    }

    // 验证 Collapse 的外部稳定 key 变化进入统一布局失效入口。
    #[test]
    // 测试名称陈述受控运行态与声明配置的边界。
    fn collapse_controlled_expansion_is_runtime_layout_change() {
        // 初始外部事实只展开首项。
        let active = State::new(vec!["alpha".to_string()]);
        // 构造保存当前运行态的 live 声明等价物。
        let current = Collapse::new()
            // 两个面板使用不同稳定 key。
            .panels(vec![
                // 首项初始由外部状态展开。
                crate::ui::widgets::CollapsePanel::new("同名", "甲内容").key("alpha"),
                // 次项保持折叠。
                crate::ui::widgets::CollapsePanel::new("同名", "乙内容").key("beta"),
            ])
            // 绑定唯一外部展开事实。
            .active_keys(&active);
        // 外部状态切换到同标题的次项。
        active.set(vec!["beta".to_string()]);
        // 构造已经采用最新外部事实的下一声明。
        let next = Collapse::new()
            // 保持作者面板配置完全相同。
            .panels(vec![
                // 首项稳定身份不变。
                crate::ui::widgets::CollapsePanel::new("同名", "甲内容").key("alpha"),
                // 次项稳定身份不变。
                crate::ui::widgets::CollapsePanel::new("同名", "乙内容").key("beta"),
            ])
            // 复用已更新的外部状态句柄。
            .active_keys(&active);
        // 静态作者配置没有变化。
        assert_eq!(
            builtin_widget_config_changed(&current.snapshot_fields(), &next.snapshot_fields()),
            Some(false)
        );
        // 受控展开集合变化必须被协调器识别为运行态变化。
        assert!(builtin_widget_runtime_changed(&current, &next));
        // Collapse 使用保守布局分类，运行态变化会因此传播 Layout 失效。
        assert_eq!(
            builtin_widget_layout_changed(&current.snapshot_fields(), &next.snapshot_fields()),
            Some(true)
        );
    }

    // 验证 ScrollView 受控偏移只进入运行态绘制，不触发布局失效。
    #[test]
    // 测试名称陈述滚动位置与视口配置的分层边界。
    fn scroll_view_controlled_offset_is_runtime_paint_change() {
        // 创建初始位于原点的外部滚动状态。
        let offset = State::new(crate::core::Point::new(0.0, 0.0));
        // 构造保存当前 live 坐标的等价声明。
        let current = ScrollView::new(crate::platform::windowing::ScrollDirection::Vertical)
            // 固定视口尺寸以覆盖布局字段稳定性。
            .size(360.0, 80.0)
            // 绑定初始外部滚动位置。
            .scroll_offset(&offset);
        // 把外部运行态向下移动四十逻辑像素。
        offset.set(crate::core::Point::new(0.0, 40.0));
        // 构造已经采样最新外部坐标的下一声明。
        let next = ScrollView::new(crate::platform::windowing::ScrollDirection::Vertical)
            // 保持视口尺寸不变。
            .size(360.0, 80.0)
            // 复用同一个受控状态句柄。
            .scroll_offset(&offset);
        // 作者视口配置没有变化，运行态坐标必须被排除。
        assert_eq!(
            // 比较两份快照的作者配置部分。
            builtin_widget_config_changed(&current.snapshot_fields(), &next.snapshot_fields()),
            // 明确报告无作者配置变化。
            Some(false)
        );
        // 受控坐标变化必须让协调器执行原位同步和绘制失效。
        assert!(builtin_widget_runtime_changed(&current, &next));
        // 只改变偏移不得重新测量或放置视口。
        assert_eq!(
            // 使用统一内建布局分类入口。
            builtin_widget_layout_changed(&current.snapshot_fields(), &next.snapshot_fields()),
            // 精细分类为无布局变化。
            Some(false)
        );
    }

    // 验证未知和自定义快照都显式进入布局失效分类。
    #[test]
    fn remaining_snapshot_fields_use_explicit_conservative_layout() {
        // 构造未知快照作为旧声明状态。
        let unknown = SnapshotFields::Unknown;
        // 未知快照与自身比较也必须保留保守分类结果。
        assert_eq!(
            builtin_widget_layout_changed(&unknown, &unknown),
            Some(true)
        );
        // 构造没有专用字段解析器的自定义快照。
        let custom = SnapshotFields::Custom {
            // 保存自定义组件的稳定类型名称。
            widget: "custom",
            // 使用空字段覆盖最小自定义声明。
            fields: Vec::new(),
        };
        // 自定义快照也必须显式进入保守布局分类。
        assert_eq!(builtin_widget_layout_changed(&unknown, &custom), Some(true));
    }
