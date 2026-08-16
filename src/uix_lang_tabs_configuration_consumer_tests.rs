// 引入宏生成代码承诺使用的公开 prelude。
use crate::prelude::*;

// 验证 Tabs 方位与溢出滚动只依赖公开 UIX 运行时契约。
#[test]
fn tabs_configuration_compiles_against_public_uix_api() {
    // 创建调用方拥有的活动标签稳定键状态。
    let active_tab = State::new("account".to_string());
    // 创建与两个直接面板一一对应的标签元数据。
    let tabs = vec![
        // 首个标签使用稳定账户键。
        Tab::new("账户").key("account"),
        // 第二个标签使用稳定安全键。
        Tab::new("安全").key("security"),
    ];
    // 声明由 Rust 类型系统核对的动态标签栏方位。
    let tab_position = TabPosition::Right;
    // 声明由 Rust 类型系统核对的动态滚动能力。
    let allow_tab_scroll = true;
    // 展开动态方位、动态滚动、受控键与真实面板子树。
    let _dynamic: ViewNode = crate::uix!(
        // 使用完整已登记 Tabs UIX 形状。
        r#"<Tabs items={tabs} activeKey={active_tab} tabPosition={tab_position} scrollable={allow_tab_scroll}><Container><Text>账户面板</Text></Container><Container><Button>安全操作</Button></Container></Tabs>"#
    );
    // 展开静态底部方位与布尔简写滚动能力。
    let _static_configuration: ViewNode = crate::uix!(
        // 复用同一调用方数据以证明生成器只取得声明快照。
        r#"<Tabs items={tabs} activeKey={active_tab} tabPosition="bottom" scrollable><Text>账户</Text><Text>安全</Text></Tabs>"#
    );
}
