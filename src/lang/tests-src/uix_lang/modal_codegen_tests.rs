// 引入与公开宏一致的完整文档测试生成入口。
use super::generate_test_document_view as generate;

// 验证 Modal 受控状态、配置、事件、有序子树与公共属性的完整生成契约。
#[test]
// 声明完整 Modal 生成测试。
fn generates_controlled_modal_contract() {
    // 生成覆盖动态状态、配置、双事件、普通节点、If/For 与公共属性的 Modal。
    let snapshot = generate(
        r#"<Modal open={modal_open} title={modal_title} maskClosable={can_mask_close} footerVisible={show_footer} @ok="save_modal" @cancel="cancel_modal" width="480px" automationId="editor-modal"><Text>正文</Text><If {show_extra}><Text>补充</Text></If><For {item} in {items}><Text>{item}</Text></For></Modal>"#,
    )
    // 合法 Modal 必须成功生成。
    .expect("文档属性应映射到公开 ModalBuilder API");
    // open 必须借用声明端 State<bool> 句柄。
    assert!(snapshot.contains("open (& (modal_open))"));
    // 动态标题必须临时借用并由运行时复制。
    assert!(snapshot.chars().filter(|ch| !ch.is_whitespace()).collect::<String>().contains(".title(&*(modal_title))"));
    // 两个动态布尔配置必须进入运行时构建器。
    assert!(snapshot.contains("mask_closable (can_mask_close)"));
    // 页脚显隐必须进入运行时构建器。
    assert!(snapshot.contains("footer_visible (show_footer)"));
    // 确认处理器必须映射到实例窄回调。
    assert!(snapshot.contains("on_ok (move ||"));
    // 取消处理器必须映射到实例窄回调。
    assert!(snapshot.contains("on_cancel (move ||"));
    // 全部内容必须作为有序集合交给 ModalBuilder。
    assert!(snapshot.contains("content_nodes ("));
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

// 验证 Modal 文档默认值与静态配置。
#[test]
// 声明 Modal 默认值测试。
fn generates_modal_defaults_and_static_values() {
    // 生成最小 Modal，页脚缺省必须显式可见。
    let defaults = generate(r#"<Modal><Text>内容</Text></Modal>"#)
        // 文档缺省值必须足以构造组件。
        .expect("Modal 默认属性应生成");
    // UIX 缺省页脚必须覆盖受控构建器的兼容默认值。
    assert!(defaults.contains("Modal :: builder () . footer_visible (true)"));
    // 未提供 open 时不得虚构第二份 State。
    assert!(!defaults.contains(". open"));
    // 生成静态标题和关闭配置。
    let configured =
        generate(r#"<Modal title="确认删除" maskClosable="false" footerVisible="false" triggerVisible="false" />"#)
            // 合法静态配置必须成功生成。
            .expect("Modal 静态配置应生成");
    // 静态标题必须映射。
    assert!(configured.contains("确认删除"));
    // 静态遮罩关闭必须映射。
    assert!(configured.contains("mask_closable (false)"));
    // 静态页脚隐藏必须覆盖缺省值。
    assert!(configured.contains("footer_visible (false)"));
    assert!(configured.contains("trigger_visible (false)"));
}

// 验证 Modal 受控状态与布尔值诊断。
#[test]
// 声明 Modal 值错误测试。
fn rejects_invalid_modal_values() {
    // open 字面量无法提供 State<bool> 生命周期句柄。
    let open = generate(r#"<Modal open="true" />"#)
        // 非受控字面量必须失败。
        .expect_err("Modal open 字面量必须被拒绝");
    // 诊断必须点明状态类型。
    assert!(open.message.contains("State<bool>"));
    // 非布尔遮罩策略必须失败。
    let mask = generate(r#"<Modal maskClosable="sometimes" />"#)
        // 非布尔字面量必须失败。
        .expect_err("Modal 非布尔 maskClosable 必须被拒绝");
    // 诊断必须点明布尔值要求。
    assert!(mask.message.contains("布尔"), "{mask:?}");
    let trigger = generate(r#"<Modal triggerVisible="sometimes" />"#)
        .expect_err("内置入口显隐必须使用布尔值");
    assert!(trigger.message.contains("布尔"), "{trigger:?}");
    // 非布尔页脚策略必须失败。
    let footer = generate(r#"<Modal footerVisible="sometimes" />"#)
        // 非布尔字面量必须失败。
        .expect_err("Modal 非布尔 footerVisible 必须被拒绝");
    // 诊断必须点明布尔值要求。
    assert!(footer.message.contains("布尔"), "{footer:?}");
}

// 验证 Modal 未登记属性与事件边界。
#[test]
// 声明 Modal 属性错误测试。
fn rejects_unknown_modal_attributes_and_events() {
    // 未登记 destroyOnClose 不能静默改变生命周期。
    let unknown = generate(r#"<Modal destroyOnClose />"#)
        // 文档外属性必须失败。
        .expect_err("Modal 未登记属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(unknown.message.contains("destroyOnClose"));
    // 未登记 afterClose 事件不能静默接线。
    let event = generate(r#"<Modal @afterClose="after_close" />"#)
        // 文档外事件必须失败。
        .expect_err("Modal 未登记事件必须被拒绝");
    // 诊断必须包含具体未知事件名。
    assert!(event.message.contains("@afterClose"));
}
