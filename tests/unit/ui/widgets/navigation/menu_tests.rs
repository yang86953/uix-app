// 引入受测菜单运行时类型。
use super::{Menu, MenuItem, MenuMode};
// 引入布局约束与指针坐标。
use crate::core::{Constraints, Point};
// 引入公开布局 trait 以核对 Inline 的完整递归高度。
use crate::ui::widget_runtime::traits::WidgetLayout;
use crate::ui::{EventResult, KeyMod, MouseButton, State, SystemEvent, WidgetTree};

// 构造含递归子项的 typed 菜单数据。
fn typed_items() -> Vec<MenuItem<String>> {
    // 返回两个顶层项和一个设置子项。
    vec![
        // 设置组拥有稳定 key 与一个递归子项。
        MenuItem::with_key("设置", "settings".to_string()).children(vec![
            // 账户子项使用独立稳定 key。
            MenuItem::with_key("账户", "account".to_string()),
        ]),
        // 帮助项作为第二个顶层选择。
        MenuItem::with_key("帮助", "help".to_string()),
    ]
}

// 验证用户选择和子菜单展开先写回 typed 状态。
#[test]
fn controlled_menu_writes_selected_and_open_keys() {
    // 初始没有任何选中项。
    let selected = State::new(None::<String>);
    // 初始没有展开的菜单组。
    let open = State::new(Vec::<String>::new());
    // 构造垂直、允许组展开的受控菜单。
    let menu = Menu::controlled(typed_items(), &selected, &open)
        // UIX 首版默认使用垂直布局。
        .mode(MenuMode::Vertical)
        // 启用子菜单组展开与收起。
        .collapsible(true);
    // 使用真实组件树路由指针事件与语义生命周期。
    let mut tree = WidgetTree::new();
    // 菜单作为唯一可交互根节点。
    tree.set_root(Box::new(menu));
    // 首次布局建立菜单命中区域。
    tree.layout();

    // 点击首行设置组，同时完成选择与展开。
    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        // 首行内部坐标命中设置组。
        pos: Point::new(8.0, 8.0),
        // 使用主指针按钮。
        button: MouseButton::Left,
        // 测试不携带组合修饰键。
        mods: KeyMod::NONE,
    });

    // 选择事实必须先写回 typed Option 状态。
    assert_eq!(selected.get(), Some("settings".to_string()));
    // 设置组稳定 key 必须进入 typed 展开集合。
    assert_eq!(open.get(), vec!["settings".to_string()]);

    // 重复点击当前组只切换展开状态，不重复改变选择值。
    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        // 再次命中首行设置组。
        pos: Point::new(8.0, 8.0),
        // 使用主指针按钮。
        button: MouseButton::Left,
        // 测试不携带组合修饰键。
        mods: KeyMod::NONE,
    });
    // 重复选择保持同一个 typed key。
    assert_eq!(selected.get(), Some("settings".to_string()));
    // 第二次点击收起子菜单组。
    assert!(open.get().is_empty());
}

// 验证外部失效 key 保持原值而界面空选中。
#[test]
fn unmatched_external_key_remains_owned_by_caller() {
    // 外部状态提供菜单树中不存在的选择。
    let selected = State::new(Some("missing".to_string()));
    // 外部展开集合同样包含失效 key。
    let open = State::new(vec!["missing".to_string()]);
    // 构造受控菜单并同步外部状态。
    let menu = Menu::controlled(typed_items(), &selected, &open).collapsible(true);

    // 界面不得把失效 key 伪装成活动项。
    assert_eq!(menu.get_active_key(), "");
    // 外部选择仍由调用方拥有，运行时不得归一化。
    assert_eq!(selected.get(), Some("missing".to_string()));
    // 外部展开事实同样保持原值。
    assert_eq!(open.get(), vec!["missing".to_string()]);
    // 内部命中树不包含失效展开项。
    assert!(menu.get_open_keys().is_empty());
}

// 验证重复稳定 key 保留首项并产生可观察诊断。
#[test]
fn duplicate_typed_key_keeps_first_item_with_diagnostic() {
    // 初始没有选中项。
    let selected = State::new(None::<String>);
    // 初始没有展开项。
    let open = State::new(Vec::<String>::new());
    // 构造包含重复全局 key 的菜单树。
    let menu = Menu::controlled(
        // 后出现的重复项必须被忽略。
        vec![
            // 首项取得 dup 身份所有权。
            MenuItem::with_key("首项", "dup".to_string()),
            // 后项不得覆盖首项。
            MenuItem::with_key("后项", "dup".to_string()),
        ],
        // 绑定 typed 选择状态。
        &selected,
        // 绑定 typed 展开状态。
        &open,
    );

    // 运行时必须暴露唯一重复 key 诊断。
    assert_eq!(menu.diagnostics().len(), 1);
    // 诊断必须点名被忽略的稳定 key。
    assert!(menu.diagnostics()[0].contains("dup"));
}

// 验证视图重建后 typed 展开状态不会被旧内部快照覆盖。
#[test]
fn controlled_sync_from_keeps_external_open_keys_authoritative() {
    // 初始没有选中项。
    let selected = State::new(None::<String>);
    // 初始没有展开项。
    let open = State::new(Vec::<String>::new());
    // 构造将被原位同步的旧菜单实例。
    let mut current = Menu::controlled(typed_items(), &selected, &open).collapsible(true);
    // 模拟组件重建前由外部业务打开设置组。
    open.set(vec!["settings".to_string()]);
    // 使用同一状态句柄构造下一帧菜单声明。
    let next = Menu::controlled(typed_items(), &selected, &open).collapsible(true);

    // 运行真实的原位组件同步路径。
    current.sync_from(next);

    // 内部展开集合必须采用最新外部事实。
    assert_eq!(current.get_open_keys(), &["settings".to_string()]);
    // 外部状态不得被旧内部快照反向覆盖。
    assert_eq!(open.get(), vec!["settings".to_string()]);
}

// 验证 Inline 即使启用 collapsible 也始终展开且不改写外部展开状态。
#[test]
fn inline_mode_keeps_children_visible_without_toggling_open_keys() {
    // 初始没有任何选中项。
    let selected = State::new(None::<String>);
    // 调用方保持空的展开集合。
    let open = State::new(Vec::<String>::new());
    // 构造同时声明 Inline 与 collapsible 的受控菜单。
    let menu = Menu::controlled(typed_items(), &selected, &open)
        // Inline 是垂直且始终展开的呈现模式。
        .mode(MenuMode::Inline)
        // 该配置在 Inline 下不得取得展开状态控制权。
        .collapsible(true);
    // 三个递归菜单项都必须参与固有高度。
    assert_eq!(
        // 使用公开布局 trait 读取完整递归列表尺寸。
        WidgetLayout::measure(&menu, Constraints::unconstrained()).h,
        // 每行三十二像素，父项、子项与第二个顶层项共三行。
        96.0
    );
    // 使用真实组件树路由父菜单项点击。
    let mut tree = WidgetTree::new();
    // 菜单作为唯一可交互根节点。
    tree.set_root(Box::new(menu));
    // 首次布局建立三行命中区域。
    tree.layout();
    // 点击首行父菜单项。
    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        // 首行内部坐标命中设置组。
        pos: Point::new(8.0, 8.0),
        // 使用主指针按钮。
        button: MouseButton::Left,
        // 测试不携带组合修饰键。
        mods: KeyMod::NONE,
    });
    // 父项选择仍需写回唯一选择状态。
    assert_eq!(selected.get(), Some("settings".to_string()));
    // Inline 点击不得把呈现事实写入调用方展开状态。
    assert!(open.get().is_empty());
}

// 验证零物化遍历保持 Inline 的递归命中顺序。
#[test]
fn borrowed_visible_traversal_preserves_inline_hit_order() {
    // 选择状态用于观察每个可见行的真实命中结果。
    let selected = State::new(None::<String>);
    // Inline 不依赖外部展开集合也必须显示完整子树。
    let open = State::new(Vec::<String>::new());
    // 构造同时包含顶层项、子项和禁用项的展开菜单。
    let menu = Menu::controlled(
        vec![
            MenuItem::from_text("禁用", "disabled").disabled(true),
            MenuItem::from_text("首项", "first"),
            MenuItem::from_text("分组", "group").children(vec![
                MenuItem::from_text("子项", "child"),
                MenuItem::from_text("禁用子项", "child-disabled").disabled(true),
            ]),
            MenuItem::from_text("末项", "last"),
        ],
        &selected,
        &open,
    )
    .mode(MenuMode::Inline)
    .collapsible(true);
    // 真实组件树负责布局与行命中。
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(menu));
    tree.layout();

    // 逐行点击验证先序顺序；禁用行保持前一个选择不变。
    for (row, expected) in [
        (0, None),
        (1, Some("first")),
        (2, Some("group")),
        (3, Some("child")),
        (4, Some("child")),
        (5, Some("last")),
    ] {
        let _ = tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(8.0, row as f32 * 32.0 + 8.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        });
        assert_eq!(selected.get().as_deref(), expected);
    }
    // Inline 命中不得把递归呈现事实写入外部展开状态。
    assert!(open.get().is_empty());
}

// 验证零物化键盘导航跳过禁用项并保持首尾环绕语义。
#[test]
fn borrowed_keyboard_navigation_skips_disabled_items_and_wraps() {
    // 外部状态用于观察导航写回并注入禁用当前项。
    let selected = State::new(None::<String>);
    let open = State::new(Vec::<String>::new());
    // 使用与命中测试相同的可见顺序覆盖子项和禁用项。
    let menu = Menu::controlled(
        vec![
            MenuItem::from_text("禁用", "disabled").disabled(true),
            MenuItem::from_text("首项", "first"),
            MenuItem::from_text("分组", "group").children(vec![
                MenuItem::from_text("子项", "child"),
                MenuItem::from_text("禁用子项", "child-disabled").disabled(true),
            ]),
            MenuItem::from_text("末项", "last"),
        ],
        &selected,
        &open,
    )
    .mode(MenuMode::Inline);
    // 把方向键通过真实焦点路径交给菜单。
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(menu));
    tree.layout();
    tree.set_focus(Some(root));
    let navigate = |tree: &mut WidgetTree, key| {
        tree.dispatch_event(&SystemEvent::KeyDown {
            key,
            mods: KeyMod::NONE,
        })
    };

    // 无当前项时向前选择首个可用项。
    assert_eq!(
        navigate(&mut tree, crate::ui::KeyCode::Down),
        EventResult::Handled
    );
    assert_eq!(selected.get().as_deref(), Some("first"));
    // 向前依次进入分组与可用子项。
    let _ = navigate(&mut tree, crate::ui::KeyCode::Down);
    assert_eq!(selected.get().as_deref(), Some("group"));
    let _ = navigate(&mut tree, crate::ui::KeyCode::Down);
    assert_eq!(selected.get().as_deref(), Some("child"));
    // 禁用子项必须被跳过。
    let _ = navigate(&mut tree, crate::ui::KeyCode::Down);
    assert_eq!(selected.get().as_deref(), Some("last"));
    // 尾项向前环绕到首个可用项。
    let _ = navigate(&mut tree, crate::ui::KeyCode::Down);
    assert_eq!(selected.get().as_deref(), Some("first"));
    // 首项向后环绕到最后可用项。
    let _ = navigate(&mut tree, crate::ui::KeyCode::Up);
    assert_eq!(selected.get().as_deref(), Some("last"));

    // 当前项禁用时保持旧契约：向前回到首项，向后回到末项。
    selected.set(Some("disabled".to_string()));
    let _ = navigate(&mut tree, crate::ui::KeyCode::Down);
    assert_eq!(selected.get().as_deref(), Some("first"));
    selected.set(Some("child-disabled".to_string()));
    let _ = navigate(&mut tree, crate::ui::KeyCode::Up);
    assert_eq!(selected.get().as_deref(), Some("last"));
}
