// 引入与公开宏一致的完整文档测试生成入口。
use super::generate_test_document_view as generate;

// 验证 Drawer 受控状态、方向、宽度、有序子树与公共属性的完整生成契约。
#[test]
// 声明完整 Drawer 生成测试。
fn generates_controlled_drawer_contract() {
    // 生成覆盖状态、方向、动态宽度、普通节点、If/For 与公共属性的 Drawer。
    let snapshot = generate(
        r#"<Drawer open={drawer_open} placement="left" width={panel_width} automationId="settings-drawer"><Text>正文</Text><If {show_extra}><Text>补充</Text></If><For {item} in {items}><Text>{item}</Text></For></Drawer>"#,
    )
    // 合法 Drawer 必须成功生成。
    .expect("文档属性应映射到公开 Drawer API");
    // open 必须借用声明端 State<bool> 句柄。
    assert!(snapshot.contains("controlled_open (& (drawer_open))"));
    // 左侧方向必须映射到公开枚举。
    assert!(snapshot.contains("DrawerPlacement :: Left"));
    // 动态宽度必须映射到 Drawer 面板宽度而非公共 View 宽度。
    assert!(snapshot.contains(". width (panel_width)"));
    // 完整内容必须作为有序集合进入 ViewNode。
    assert!(snapshot.contains("ViewNode :: new"));
    // 普通正文必须保留。
    assert!(snapshot.contains("正文"));
    // If 控制流必须保留。
    assert!(snapshot.contains("if show_extra"));
    // For 控制流必须带内部位置身份并保留输入集合。
    assert!(
        snapshot.contains("__uix_for_ordinal")
            && snapshot.contains("enumerate")
            && snapshot.contains("items")
    );
    // 公共自动化身份必须继续映射。
    assert!(snapshot.contains("automation_id"));
}

// 验证 Drawer 默认值、四个方向与像素宽度。
#[test]
// 声明 Drawer 静态值测试。
fn generates_drawer_defaults_and_static_values() {
    // 最小 Drawer 应沿用运行时关闭、右侧与默认宽度。
    let defaults = generate(r#"<Drawer />"#).expect("Drawer 默认属性应生成");
    // 默认生成不得虚构受控 State。
    assert!(!defaults.contains("controlled_open"));
    // 默认生成不得覆盖运行时方向。
    assert!(!defaults.contains("DrawerPlacement"));
    // 默认生成不得覆盖运行时面板宽度。
    assert!(!defaults.contains(". width"));

    // 逐个验证文档允许的四个方向。
    for (source, variant) in [
        // 左侧方向样例。
        (r#"<Drawer placement="left" />"#, "Left"),
        // 右侧方向样例。
        (r#"<Drawer placement="right" />"#, "Right"),
        // 顶部方向样例。
        (r#"<Drawer placement="top" />"#, "Top"),
        // 底部方向样例。
        (r#"<Drawer placement="bottom" />"#, "Bottom"),
    ] {
        // 生成当前方向样例。
        let snapshot = generate(source).expect("合法 Drawer placement 应生成");
        // 快照必须包含对应公开枚举。
        assert!(snapshot.contains(&format!("DrawerPlacement :: {variant}")));
    }

    // px 字面量必须转换为 f32 面板宽度。
    let width = generate(r#"<Drawer width="420px" />"#).expect("Drawer px 宽度应生成");
    // 生成值必须进入 Drawer 宽度构建器。
    assert!(width.contains(". width (420"));
}

// 验证 Drawer 受控状态、方向与宽度诊断。
#[test]
// 声明 Drawer 值错误测试。
fn rejects_invalid_drawer_values() {
    // open 字面量无法提供 State<bool> 生命周期句柄。
    let open = generate(r#"<Drawer open="true" />"#)
        // 非受控字面量必须失败。
        .expect_err("Drawer open 字面量必须被拒绝");
    // 诊断必须点明状态类型。
    assert!(open.message.contains("State<bool>"));
    // 动态方向无法在编译期确定枚举。
    let dynamic = generate(r#"<Drawer placement={side} />"#)
        // 动态方向必须失败。
        .expect_err("Drawer 动态 placement 必须被拒绝");
    // 诊断必须点明字符串字面量要求。
    assert!(dynamic.message.contains("字符串字面量"));
    // 未登记方向必须失败。
    let placement = generate(r#"<Drawer placement="center" />"#)
        // 非法方向必须失败。
        .expect_err("Drawer 非法 placement 必须被拒绝");
    // 诊断必须包含允许方向。
    assert!(placement.suggestion.contains("left"));
    // 非数值宽度必须失败。
    let width = generate(r#"<Drawer width="wide" />"#)
        // 非数值字面量必须失败。
        .expect_err("Drawer 非数值 width 必须被拒绝");
    // 诊断必须点明数值映射。
    assert!(width.message.contains("f32"));
}

// 验证 Drawer 未登记属性边界。
#[test]
// 声明 Drawer 属性错误测试。
fn rejects_unknown_drawer_attributes() {
    // 文档未登记 title，不能静默借用运行时扩展能力。
    let title = generate(r#"<Drawer title="设置" />"#)
        // 文档外属性必须失败。
        .expect_err("Drawer title 尚未登记，必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(title.message.contains("title"));
    // 未登记关闭事件不能静默接线。
    let event = generate(r#"<Drawer @close="after_close" />"#)
        // 文档外事件必须失败。
        .expect_err("Drawer @close 尚未登记，必须被拒绝");
    // 诊断必须包含具体未知事件名。
    assert!(event.message.contains("@close"));
}
