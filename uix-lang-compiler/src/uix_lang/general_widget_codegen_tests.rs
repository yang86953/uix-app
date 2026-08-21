// 引入完整文档测试生成入口。
use super::generate_test_document_view as generate;

// 验证 Button 尺寸、加载态与 Label 选择态生成公开运行时调用。
#[test]
fn generates_button_and_label_documented_properties() {
    // 构造覆盖外部动态布尔值和确定尺寸关键字的文档根。
    let tokens = generate(
        // 同时覆盖普通按钮与可选 Label，保留外部变量名供快照断言。
        r#"
        <Column>
          <Button size="small" loading={busy}>提交</Button>
          <Label selectable={can_select}>可选择文字</Label>
        </Column>
        "#,
    )
    // 文档登记属性必须生成成功。
    .expect("Button 与 Label 规划属性应生成");
    // Button small 必须映射公开尺寸枚举。
    assert!(tokens.contains("ControlSize :: Small"));
    // 动态 loading 必须进入公开构建器。
    assert!(tokens.contains("loading (busy)"));
    // Label 必须直接构造既有公开组件。
    assert!(tokens.contains("Label :: new"));
    // 动态 selectable 必须保留真假两支。
    assert!(tokens.contains("if can_select"));
    // 真分支必须调用既有文本选择入口。
    assert!(tokens.contains("selectable ()"));
}

// 验证 ButtonGroup 直接子按钮复用尺寸与加载映射。
#[test]
fn button_group_preserves_size_loading_and_group_position() {
    // 构造带连体位置的两个直接按钮。
    let tokens = generate(
        // 首项覆盖大尺寸和布尔简写加载态。
        r#"<ButtonGroup><Button size="large" loading>一</Button><Button size="middle">二</Button></ButtonGroup>"#,
    )
    // ButtonGroup 必须复用普通按钮生成路径。
    .expect("ButtonGroup 子按钮属性应生成");
    // 大尺寸必须映射公开变体。
    assert!(tokens.contains("ControlSize :: Large"));
    // 默认文档关键字 middle 必须映射 Medium。
    assert!(tokens.contains("ControlSize :: Medium"));
    // 布尔简写必须启用加载态。
    assert!(tokens.contains("loading (true)"));
    // 连体位置仍必须写入底层按钮。
    assert!(tokens.contains("group_position"));
}

// 验证非法尺寸与 Text 越权属性保持精确诊断。
#[test]
fn rejects_invalid_button_size_and_text_selectable() {
    // 未登记尺寸不得回退到中尺寸。
    let size = generate(r#"<Button size="huge">按钮</Button>"#)
        // 提取预期诊断。
        .expect_err("非法 Button size 必须失败");
    // 消息必须保留非法关键字。
    assert!(size.message.contains("huge"));
    // 建议必须列出合法关键字。
    assert!(size.suggestion.contains("small") && size.suggestion.contains("large"));
    // selectable 只属于 Label，不扩大 Text 语义。
    let text = generate(r#"<Text selectable>文字</Text>"#)
        // 提取统一未知属性诊断。
        .expect_err("Text selectable 必须失败");
    // 诊断必须指出越权属性。
    assert!(text.message.contains("selectable"));
}
