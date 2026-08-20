// 引入导航组件与共享 UI 测试类型。
use super::{Tab, Tabs};
// 引入测试面板使用的通用组件。
use crate::ui::widgets::{Button, Container};
// 引入受控状态与键盘事件类型。
use crate::ui::{EventResult, KeyCode, KeyMod, State, SystemEvent, WidgetTree};

// 构造切换到下一标签页的键盘事件。
fn next_tab_event() -> SystemEvent {
    // 返回无修饰键的向右导航事件。
    SystemEvent::KeyDown {
        // 向右键选择下一标签页。
        key: KeyCode::Right,
        // 测试不携带组合修饰键。
        mods: KeyMod::NONE,
    }
}

// 验证切换面板后旧面板退出命中树且焦点进入新面板首个控件。
#[test]
fn switches_child_visibility_and_moves_focus_into_active_panel() {
    // 建立由稳定字符串 key 控制的活动状态。
    let active = State::new("account".to_string());
    // 构造两个标签元数据并绑定活动 key。
    let tabs = Tabs::new()
        // 提供与两个直接面板一一对应的元数据。
        .tabs(vec![
            // 首个标签使用稳定账户 key。
            Tab::new("账户").key("account"),
            // 第二个标签使用稳定安全 key。
            Tab::new("安全").key("security"),
        ])
        // 把选择事实交给外部状态句柄。
        .active_key(&active);
    // 建立真实组件树以执行父级可见性门控。
    let mut tree = WidgetTree::new();
    // Tabs 作为可聚焦标签栏根节点。
    let tabs_id = tree.set_root(Box::new(tabs));
    // 首个直接按钮同时代表账户面板与旧焦点目标。
    let account_button = tree.add_child(tabs_id, Box::new(Button::new("账户操作")));
    // 第二个直接容器代表安全面板。
    let security_panel = tree.add_child(tabs_id, Box::new(Container::new()));
    // 安全面板内的按钮应成为切换后的首个焦点目标。
    let security_button = tree.add_child(security_panel, Box::new(Button::new("安全操作")));
    // 首次布局同步活动面板的父级可见性门控。
    tree.layout();
    // 把焦点放在即将隐藏的账户面板内。
    tree.set_focus(Some(account_button));

    // 通过焦点路径把方向键冒泡到 Tabs。
    let result = tree.dispatch_event(&next_tab_event());
    // Tabs 必须消费标签切换事件。
    assert_eq!(result, EventResult::Handled);
    // 执行布局请求并同步两个直接面板的可见性门控。
    tree.layout();

    // 活动 key 必须写回外部唯一事实源。
    assert_eq!(active.get(), "security");
    // 旧面板必须退出布局与命中树。
    assert!(
        !tree
            .get(account_button)
            .expect("账户面板存在")
            .parent_visibility_gate()
    );
    // 新面板必须重新进入布局与命中树。
    assert!(
        tree.get(security_panel)
            .expect("安全面板存在")
            .parent_visibility_gate()
    );
    // 焦点必须迁移到新面板内第一个可聚焦控件。
    assert_eq!(
        tree.managers().focus.focused_widget(),
        Some(security_button)
    );
}

// 验证新面板没有可聚焦控件时焦点退回 Tabs 标签栏。
#[test]
fn falls_back_to_tabs_bar_when_active_panel_has_no_focusable_child() {
    // 建立由稳定字符串 key 控制的活动状态。
    let active = State::new("account".to_string());
    // 构造两个标签元数据并绑定活动 key。
    let tabs = Tabs::new()
        // 提供与两个直接面板一一对应的元数据。
        .tabs(vec![
            // 首个标签使用稳定账户 key。
            Tab::new("账户").key("account"),
            // 第二个标签使用稳定空态 key。
            Tab::new("空态").key("empty"),
        ])
        // 把选择事实交给外部状态句柄。
        .active_key(&active);
    // 建立真实组件树以执行门控与焦点生命周期。
    let mut tree = WidgetTree::new();
    // Tabs 作为可聚焦标签栏根节点。
    let tabs_id = tree.set_root(Box::new(tabs));
    // 首个直接按钮同时代表旧活动面板。
    let account_button = tree.add_child(tabs_id, Box::new(Button::new("账户操作")));
    // 第二个直接容器没有任何可聚焦后代。
    let empty_panel = tree.add_child(tabs_id, Box::new(Container::new()));
    // 首次布局同步活动面板门控。
    tree.layout();
    // 把焦点放在即将隐藏的首个面板。
    tree.set_focus(Some(account_button));

    // 触发下一标签页选择并写回状态。
    assert_eq!(tree.dispatch_event(&next_tab_event()), EventResult::Handled);
    // 同步面板门控并执行焦点替代约定。
    tree.layout();

    // 外部状态必须反映空态面板的稳定 key。
    assert_eq!(active.get(), "empty");
    // 空态面板仍应处于活动可见状态。
    assert!(
        tree.get(empty_panel)
            .expect("空态面板存在")
            .parent_visibility_gate()
    );
    // 没有子控件时焦点必须退回标签栏本身。
    assert_eq!(tree.managers().focus.focused_widget(), Some(tabs_id));
}
