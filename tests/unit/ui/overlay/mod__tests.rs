// 引入本模块公开契约。
use super::*;

// 验证全部 OverlayKind 共用一个 typed effect 入口，并聚合区域与最大半径。
#[test]
fn backdrop_effect_aggregates_all_overlay_kinds() {
    // 枚举产品决策开放的全部 overlay 类型。
    let kinds = [
        // 模态对话框。
        OverlayKind::Modal,
        // 抽屉。
        OverlayKind::Drawer,
        // 气泡卡片。
        OverlayKind::Popover,
        // 提示。
        OverlayKind::Tooltip,
        // 上下文菜单。
        OverlayKind::ContextMenu,
        // 全局消息。
        OverlayKind::Message,
        // 通知。
        OverlayKind::Notification,
        // 自定义浮层。
        OverlayKind::Custom,
    ];
    // 创建空栈并逐项登记显式请求。
    let mut stack = OverlayStack::new();
    // 每个类型使用不同区域与半径，证明没有按 kind 开洞。
    for (index, kind) in kinds.into_iter().enumerate() {
        // 使用稳定组件身份。
        let owner = WidgetId::new(index + 1);
        // 默认请求跟随当前 entry 的 mask bounds。
        let entry = OverlayEntry::new(owner, kind)
            // 让全部区域形成可预测并集。
            .bounds(Rect::new(index as f32, 0.0, 10.0, 10.0))
            // 最大半径应来自最后一个请求。
            .backdrop_blur(OverlayBackdropBlur::radius(index as f32 + 1.0));
        // 使用公开统一入口登记。
        stack.push_entry(entry);
    }
    // 聚合时主题值不会覆盖各 entry 的显式半径。
    let effect = stack
        // 传入不同 Theme 默认值以证明显式值优先。
        .backdrop_effect(3.0)
        // 八个合法请求必须形成计划。
        .expect("all overlay kinds should share backdrop blur");
    // 区域从 x=0 延伸到最后一个 bounds 的右边界 17。
    assert_eq!(effect.region(), Rect::new(0.0, 0.0, 17.0, 10.0));
    // 最大显式半径为八。
    assert_eq!(effect.radius(), 8.0);
}

// 验证 Theme 默认、独立区域与 no-op 半径解析。
#[test]
fn backdrop_effect_resolves_theme_and_independent_region() {
    // 使用 Custom 证明独立区域不依赖 mask bounds。
    let entry = OverlayEntry::new(WidgetId::new(1), OverlayKind::Custom)
        // 故意不设置 bounds。
        .backdrop_blur(
            // 半径延迟读取 Theme。
            OverlayBackdropBlur::theme()
                // 独立逻辑区域供 Custom 使用。
                .region(Rect::new(4.0, 6.0, 20.0, 12.0)),
        );
    // 构造只含一个 Custom 的栈。
    let mut stack = OverlayStack::new();
    // 登记统一 effect 请求。
    stack.push_entry(entry);
    // 主题默认值应进入已解析效果。
    let effect = stack
        // 使用产品默认八像素。
        .backdrop_effect(8.0)
        // 独立区域不需要 mask bounds。
        .expect("independent backdrop region should resolve");
    // 保留调用方逻辑区域。
    assert_eq!(effect.region(), Rect::new(4.0, 6.0, 20.0, 12.0));
    // 使用 Theme token 半径。
    assert_eq!(effect.radius(), 8.0);
    // 小于半像素按契约为 no-op。
    assert!(stack.backdrop_effect(0.49).is_none());
}

// 验证无排序缓冲的插入仍保持层级与同层声明顺序。
#[test]
fn overlay_entry_insertion_preserves_stable_z_order() {
    let mut stack = OverlayStack::new();
    stack.push_entry(OverlayEntry::new(WidgetId::new(1), OverlayKind::Popover).z_index(10));
    stack.push_entry(OverlayEntry::new(WidgetId::new(2), OverlayKind::Tooltip).z_index(5));
    stack.push_entry(OverlayEntry::new(WidgetId::new(3), OverlayKind::Custom).z_index(10));

    let owners: Vec<_> = stack.iter().map(OverlayEntry::owner).collect();
    assert_eq!(
        owners,
        vec![WidgetId::new(2), WidgetId::new(1), WidgetId::new(3)]
    );
}
