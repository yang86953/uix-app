// 引入文档解析与普通 View 生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 把 UIX 源码生成 Rust 令牌字符串快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析测试文档。
    let document = parse_document(source)?;
    // 生成根 View 并转成可断言字符串。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Tabs 生成元数据、受控 key、有序面板与变化事件。
#[test]
fn generates_tabs_with_controlled_key_panels_and_change() {
    // 生成包含两个静态面板的 Tabs。
    let tokens = generate(
        "<Tabs items={[Tab('账户').key('account'), Tab('安全').key('security')]} activeKey={active_tab} @change=\"on_change($event)\"><Text>账户面板</Text><Container><Button>保存</Button></Container></Tabs>",
    )
    // 合法契约必须成功生成。
    .expect("Tabs 应生成成功");
    // 快照必须包含公开组件、数据类型、状态绑定与事件注册。
    assert!(
        tokens.contains("Tabs")
            && tokens.contains("Tab")
            && tokens.contains("active_key")
            && tokens.contains("active_tab")
            && tokens.contains("on_change_fn")
            && tokens.contains("on_change"),
        "{tokens}"
    );
    // 两个直接面板都必须进入拥有型子集合以保留状态。
    assert!(
        tokens.contains("账户面板") && tokens.contains("保存"),
        "{tokens}"
    );
}

// 验证 Tabs 缺失必需属性时返回定位诊断。
#[test]
fn rejects_tabs_without_required_contract() {
    // items 缺失时不能伪造标签元数据。
    let missing_items = generate("<Tabs activeKey={active_tab}><Text>面板</Text></Tabs>")
        // 生成必须失败。
        .expect_err("缺失 items 必须失败");
    // 诊断必须点名缺失属性。
    assert!(missing_items.message.contains("items"), "{missing_items:?}");
    // activeKey 缺失时不能退化为非受控选择。
    let missing_active = generate("<Tabs items={tabs}><Text>面板</Text></Tabs>")
        // 生成必须失败。
        .expect_err("缺失 activeKey 必须失败");
    // 诊断必须点名缺失受控状态。
    assert!(
        missing_active.message.contains("activeKey"),
        "{missing_active:?}"
    );
}

// 验证动态直接面板与首版未支持 type 都被拒绝。
#[test]
fn rejects_dynamic_direct_panels_and_unimplemented_type() {
    // 直接 If 会破坏 items 与面板的一一对应。
    let dynamic = generate(
        "<Tabs items={tabs} activeKey={active_tab}><If {visible}><Text>动态</Text></If></Tabs>",
    )
    // 生成必须失败。
    .expect_err("直接动态面板必须失败");
    // 诊断必须说明直接控制流限制。
    assert!(dynamic.message.contains("直接 If/For"), "{dynamic:?}");
    // 首版没有登记 type 属性，因此 line/card 不得伪装支持。
    let type_error = generate(
        "<Tabs items={tabs} activeKey={active_tab} type=\"card\"><Text>面板</Text></Tabs>",
    )
    // 未消费属性必须由公共属性门禁拒绝。
    .expect_err("首版 type 必须失败");
    // 诊断必须点名未知属性。
    assert!(type_error.message.contains("type"), "{type_error:?}");
}
