// 引入宏生成代码承诺使用的公开 prelude。
use crate::prelude::*;

// 验证 Menu inline 只依赖公开运行时枚举与受控状态契约。
#[test]
fn menu_inline_compiles_against_public_uix_api() {
    // 创建调用方拥有的菜单数据树。
    let menu_items = vec![
        // 父菜单项拥有一个递归子项。
        MenuItem::with_key("设置", "settings".to_string()).children(vec![
            // 子项使用独立稳定 key。
            MenuItem::with_key("账户", "account".to_string()),
        ]),
    ];
    // 创建调用方拥有的唯一选择状态。
    let selected_menu = State::new(None::<String>);
    // 创建调用方拥有的展开状态；Inline 不会改写它。
    let open_menus = State::new(Vec::<String>::new());
    // 展开同时声明 Inline 与 collapsible 的公开 UIX 形状。
    let _inline_menu: ViewNode = crate::uix!(
        // 生成器只把 mode 映射到公开 MenuMode::Inline。
        r#"<Menu items={menu_items} selectedKey={selected_menu} openKeys={open_menus} mode="inline" collapsible />"#
    );
}
