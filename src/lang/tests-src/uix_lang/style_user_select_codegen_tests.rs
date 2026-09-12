// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证 userSelect 四种文档值映射到公开选择策略枚举。
#[test]
fn generates_user_select_runtime_mapping() {
    // 覆盖每个文档值及其运行时枚举变体。
    for (source, variant) in [
        // 保留组件默认选择能力。
        ("auto", "Auto"),
        // 禁止普通文字选择。
        ("none", "None"),
        // 启用文字范围选择。
        ("text", "Text"),
        // 选择最近的完整子树。
        ("all", "All"),
    ] {
        // 构造只包含目标选择值的元素文档。
        let document = parse_document(&format!(
            // 保留关键字源码供编译期映射。
            r#"<Text style="userSelect: {source};">文字</Text>"#
        ))
        // 语法层必须接受规范样式值。
        .expect("userSelect 语法应合法");
        // 生成真实 View 调用令牌。
        let tokens = generate_view(&document.root)
            // 已映射值不得继续返回规划中诊断。
            .expect("userSelect 应映射到运行时策略")
            // 规范化令牌用于断言公开契约。
            .to_string();
        // 最终节点必须通过唯一公开 user_select 入口更新。
        assert!(tokens.contains("user_select"));
        // 策略必须来自 UI 预lude 公开枚举。
        assert!(tokens.contains(&format!("UserSelect :: {variant}")));
    }
}

// 验证未知 userSelect 值产生确定诊断。
#[test]
fn rejects_unknown_user_select_value() {
    // 构造文档未登记的 contain 值。
    let document = parse_document(r#"<Text style="userSelect: contain;">文字</Text>"#)
        // 样式语法层保留原始关键字供映射层诊断。
        .expect("userSelect 原始值应完成语法解析");
    // 代码生成必须返回支持边界诊断。
    let error = generate_view(&document.root).expect_err("未知 userSelect 必须失败");
    // 诊断必须包含文档允许值。
    assert!(error.message.contains("userSelect 只支持"));
}
