// 引入文档解析与普通 View 生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 把 UIX 源码生成 Rust 令牌字符串快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析测试文档。
    let document = parse_document(source)?;
    // 生成根 View 并转成可断言字符串。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Menu 生成 typed 数据、双状态、方向、组折叠与选择事件。
#[test]
fn generates_controlled_menu_contract() {
    // 生成覆盖首版全部专有属性的菜单。
    let tokens = generate(
        "<Menu items={[MenuItem('设置', 'settings').children([MenuItem('账户', 'account')])]} selectedKey={selected} openKeys={open} mode=\"horizontal\" collapsible @select=\"on_select($event)\" width=\"240px\" />",
    )
    // 合法契约必须成功生成。
    .expect("Menu 应生成成功");
    // 快照必须包含受控构造、两个句柄、方向与折叠配置。
    assert!(
        tokens.contains("Menu :: controlled")
            && tokens.contains("selected")
            && tokens.contains("open")
            && tokens.contains("MenuMode :: Horizontal")
            && tokens.contains("collapsible (true)"),
        "{tokens}"
    );
    // 选择事件必须通过公开 Change 语义入口发布稳定 key。
    assert!(
        tokens.contains("on_change_fn") && tokens.contains("on_select"),
        "{tokens}"
    );
    // 公共尺寸属性必须继续应用到 View。
    assert!(tokens.contains("width (240.0)"), "{tokens}");
}

// 验证 Menu inline 映射到运行时始终展开模式。
#[test]
fn generates_inline_menu_mode() {
    // 生成同时声明 Inline 与 collapsible 的完整受控菜单。
    let tokens = generate(
        // 该组合必须由运行时统一解决展开呈现，不由生成器改写 openKeys。
        "<Menu items={items} selectedKey={selected} openKeys={open} mode=\"inline\" collapsible />",
    )
    // 已登记模式必须成功生成。
    .expect("Menu inline 应生成成功");
    // 生成代码必须只映射公开枚举并保留原有折叠配置。
    assert!(
        // 同时锁定模式与配置，防止适配层复制展开状态。
        tokens.contains("MenuMode :: Inline") && tokens.contains("collapsible (true)"),
        // 失败时输出完整生成令牌。
        "{tokens}"
    );
}

// 验证 Menu 缺失必需受控属性时返回定位诊断。
#[test]
fn rejects_menu_without_required_state_contract() {
    // selectedKey 缺失时不能退化为非受控单选。
    let missing_selected = generate("<Menu items={items} openKeys={open} />")
        // 生成必须失败。
        .expect_err("缺失 selectedKey 必须失败");
    // 诊断必须点名缺失单选状态。
    assert!(
        missing_selected.message.contains("selectedKey"),
        "{missing_selected:?}"
    );
    // openKeys 缺失时不能把递归展开状态藏在组件内部。
    let missing_open = generate("<Menu items={items} selectedKey={selected} />")
        // 生成必须失败。
        .expect_err("缺失 openKeys 必须失败");
    // 诊断必须点名缺失展开状态。
    assert!(
        missing_open.message.contains("openKeys"),
        "{missing_open:?}"
    );
}

// 验证非法方向和内容子树被编译期拒绝。
#[test]
fn rejects_invalid_menu_mode_and_children() {
    // 未登记的 stacked 不在文档关键字集合。
    let mode =
        generate("<Menu items={items} selectedKey={selected} openKeys={open} mode=\"stacked\" />")
            // 生成必须失败。
            .expect_err("非法 mode 必须失败");
    // 诊断必须列出允许模式。
    assert!(
        mode.message.contains("vertical")
            && mode.message.contains("horizontal")
            && mode.message.contains("inline")
    );
    // Menu 数据树由 items 唯一拥有，子 View 不得被丢弃。
    let child = generate(
        "<Menu items={items} selectedKey={selected} openKeys={open}><Text>非法</Text></Menu>",
    )
    // 生成必须失败。
    .expect_err("Menu 子节点必须失败");
    // 诊断必须明确叶组件边界。
    assert!(child.message.contains("不接受子节点"), "{child:?}");
}
