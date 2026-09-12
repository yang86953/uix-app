// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证 cursor 五种文档值映射到公开光标枚举。
#[test]
fn generates_cursor_runtime_mapping() {
    // 覆盖每个文档值及其平台无关枚举变体。
    for (source, variant) in [
        // 默认箭头。
        ("default", "Arrow"),
        // 可点击手形。
        ("pointer", "Hand"),
        // 文字插入光标。
        ("text", "IBeam"),
        // 四向移动光标。
        ("move", "Move"),
        // 禁止操作光标。
        ("not-allowed", "NotAllowed"),
    ] {
        // 构造只包含目标光标值的元素文档。
        let document = parse_document(&format!(
            // 保留关键字源码供编译期映射。
            r#"<Text style="cursor: {source};">光标</Text>"#
        ))
        // 语法层必须接受规范样式值。
        .expect("cursor 语法应合法");
        // 生成真实 View 调用令牌。
        let tokens = generate_view(&document.root)
            // 已映射光标不得继续返回规划中诊断。
            .expect("cursor 应映射到运行时光标")
            // 规范化令牌用于断言公开契约。
            .to_string();
        // 最终节点必须通过唯一公开 cursor 入口更新。
        assert!(tokens.contains("cursor"));
        // 光标必须来自 UI 预lude 公开枚举。
        assert!(tokens.contains(&format!("CursorType :: {variant}")));
    }
}

// 验证未知 cursor 值产生确定诊断。
#[test]
fn rejects_unknown_cursor_value() {
    // 构造未登记的缩放光标值。
    let document = parse_document(r#"<Text style="cursor: zoom-in;">光标</Text>"#)
        // 样式语法层保留原始关键字供映射层诊断。
        .expect("cursor 原始值应完成语法解析");
    // 代码生成必须返回支持边界诊断。
    let error = generate_view(&document.root).expect_err("未知 cursor 必须失败");
    // 诊断必须包含文档允许值。
    assert!(error.message.contains("cursor 只支持"));
}
