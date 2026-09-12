// 引入文档解析与普通 View 生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 把 UIX 源码生成 Rust 令牌字符串快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析测试文档。
    let document = parse_document(source)?;
    // 生成根 View 并转成可断言字符串。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 MenuBar 生成 keyed 菜单集与 Change 拥有型稳定 key。
#[test]
fn generates_keyed_menus_and_change_contract() {
    // 生成覆盖首版全部专有属性的菜单栏。
    let tokens = generate(
        "<MenuBar menus={[MenuBarMenu('文件', 'file').item(MenuBarItem('新建', 'new'))]} @change=\"on_menu($event)\" width=\"320px\" automationId=\"app-menu\" />",
    )
    // 合法声明契约必须生成成功。
    .expect("MenuBar 应生成成功");
    // 快照必须包含 keyed 数据门禁与 Change 注册。
    assert!(
        tokens.contains("MenuBar :: new")
            && tokens.contains("keyed_menus")
            && tokens.contains("MenuBarMenu :: from_text")
            && tokens.contains("MenuBarItem :: from_text")
            && tokens.contains("on_change_fn"),
        "{tokens}"
    );
    // 载荷必须物化为拥有型 String 再进入声明层处理器。
    assert!(
        tokens.contains("let __uix_menu_bar_change = __uix_menu_bar_change . to_string ()"),
        "{tokens}"
    );
}

// 验证 MenuBar 拒绝子节点、缺失 menus 与未知事件载荷字段。
#[test]
fn rejects_children_missing_menus_and_unknown_payload() {
    // 子节点违反叶组件形状。
    let children = generate("<MenuBar menus={[MenuBarMenu('文件', 'file')]}><Text>内容</Text></MenuBar>");
    // 必须产生明确形状诊断。
    assert!(children.is_err(), "MenuBar 不接受子节点");
    // 缺失必需 menus 属性。
    let missing = generate("<MenuBar @change=\"on_menu($event)\" />");
    // 必须产生缺失属性诊断。
    assert!(missing.is_err(), "MenuBar 缺少 menus 应被拒绝");
    // 未知事件载荷字段不得静默通过。
    let payload = generate(
        "<MenuBar menus={[MenuBarMenu('文件', 'file')]} @change=\"on_menu($event.unknown)\" />",
    );
    // 必须产生未登记字段诊断。
    assert!(payload.is_err(), "MenuBar @change 只登记 value 字段");
}
