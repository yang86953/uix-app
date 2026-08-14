// 引入组合 Navigation、共享数据与外壳类型。
use super::{Menu, MenuItem, Navigation, NavigationShell};
// 引入事件行为 trait 以直接验证折叠事实建立顺序。
use crate::ui::component::traits::EventHandler;
// 引入指针坐标与状态/事件类型。
use crate::core::Point;
use crate::ui::{KeyMod, MouseButton, State, SystemEvent};

// 验证组合入口只创建一个受控 Menu 子树。
#[test]
fn controlled_navigation_owns_exactly_one_menu_child() {
    // 初始没有活动菜单项。
    let active = State::new(None::<String>);
    // 初始没有展开菜单组。
    let open = State::new(Vec::<String>::new());
    // 初始侧栏处于展开态。
    let collapsed = State::new(false);
    // 构造带调用方版本元数据的组合 Navigation。
    let view = Navigation::controlled(
        // 标题只归侧栏外壳。
        "应用",
        // 数据树只交给 Menu。
        vec![MenuItem::with_key("首页", "home".to_string())],
        // 绑定唯一选择事实。
        &active,
        // 绑定唯一展开事实。
        &open,
        // 绑定整栏折叠事实。
        &collapsed,
        // 版本由调用方精确提供。
        Some("v1.2.3".to_string()),
    );

    // 组合根必须是 NavigationShell。
    assert!(view.widget.as_any().is::<NavigationShell>());
    // 外壳只允许一个直接 Menu 子树。
    assert_eq!(view.children.len(), 1);
    // 唯一子树必须是受控 Menu，而不是第二套 NavItem 集合。
    assert!(view.children[0].widget.as_any().is::<Menu>());
}

// 验证折叠按钮先写回调用方状态并保留 Menu 状态。
#[test]
fn shell_toggle_updates_only_collapsed_state() {
    // 建立 Menu 唯一拥有的活动项事实。
    let active = State::new(Some("home".to_string()));
    // 建立 Menu 唯一拥有的展开组事实。
    let open = State::new(vec!["admin".to_string()]);
    // 建立外壳唯一拥有的调用方折叠状态。
    let collapsed = State::new(false);
    // 构造显式版本外壳。
    let mut shell = NavigationShell::new("应用", Some("v1.2.3".to_string()), &collapsed);
    // 模拟右上折叠按钮点击。
    let result = EventHandler::on_event(
        &mut shell,
        &SystemEvent::PointerDown {
            // 默认 200px 宽度下命中右上 32px 按钮。
            pos: Point::new(180.0, 12.0),
            // 使用主指针按钮。
            button: MouseButton::Left,
            // 不携带修饰键。
            mods: KeyMod::NONE,
        },
    );

    // 点击必须由外壳处理。
    assert_eq!(result, crate::ui::EventResult::Handled);
    // 折叠事实必须先写回调用方 State。
    assert!(collapsed.get());
    // 外壳折叠不得改写 Menu 的活动项事实。
    assert_eq!(active.get(), Some("home".to_string()));
    // 外壳折叠不得改写 Menu 的展开组事实。
    assert_eq!(open.get(), vec!["admin".to_string()]);
    // 外壳当前帧呈现同步进入折叠态。
    assert!(shell.is_collapsed());
    // 几何变化必须请求一次重新布局。
    assert!(EventHandler::take_layout_request(&mut shell));
    // 布局请求只能消费一次。
    assert!(!EventHandler::take_layout_request(&mut shell));
}
