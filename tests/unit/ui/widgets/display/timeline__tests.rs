    // 引入被测节点构建器。
    use super::TimelineItem;
    // 引入最终绘制颜色值。
    use crate::draw::Color;

    // 默认节点跟随主题，显式颜色保持最高优先级。
    #[test]
    fn timeline_item_resolves_theme_and_explicit_colors() {
        // 使用与默认主题无关的测试主色。
        let theme_primary = Color::from_rgb(1, 2, 3);
        // 未覆写节点必须使用调用方提供的当前主题主色。
        let themed = TimelineItem::new("主题节点");
        // 核对主题默认路径。
        assert_eq!(themed.resolved_dot_color(theme_primary), theme_primary);
        // 构造明确不同的显式节点色。
        let explicit = Color::from_rgb(4, 5, 6);
        // 使用公开构建器登记显式颜色。
        let customized = TimelineItem::new("自定义节点").color(explicit);
        // 核对显式覆写优先于主题主色。
        assert_eq!(customized.resolved_dot_color(theme_primary), explicit);
    }
