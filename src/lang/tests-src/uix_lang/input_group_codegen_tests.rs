// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 InputGroup 文档属性与文本状态绑定生成。
#[test]
fn generates_bound_input_group_contract() {
    // 生成覆盖两侧附加文本、状态绑定和公共尺寸的 InputGroup。
    let snapshot =
        generate(r#"<InputGroup addonBefore="¥" value={amount} addonAfter="元" width="200px" />"#)
            // 合法复合输入必须成功生成。
            .expect("文档属性应映射到公开 InputGroup API");
    // 前置附加文本必须先建立。
    let before = snapshot
        .find("addon_before (\"¥\")")
        .expect("应生成前置文本");
    // 状态绑定必须借用 State<String>。
    let value = snapshot.find("value (& (amount))").expect("应生成状态绑定");
    // 后置附加文本必须存在。
    let after = snapshot
        .find("addon_after (\"元\")")
        .expect("应生成后置文本");
    // 专有配置顺序必须保持文档结构。
    assert!(before < value && value < after);
    // 公共宽度必须继续映射。
    assert!(snapshot.contains("width (200.0)"));
}

// 验证动态附加文本表达式保持公开 String 类型检查。
#[test]
fn generates_dynamic_input_group_addons() {
    // 生成动态前后文本与状态绑定。
    let snapshot =
        generate(r#"<InputGroup addonBefore={prefix} value={amount} addonAfter={suffix} />"#)
            // 动态配置必须成功生成。
            .expect("动态附加文本应生成");
    // 两侧表达式必须进入对应公开构建器。
    assert!(
        snapshot.contains("addon_before (prefix)") && snapshot.contains("addon_after (suffix)")
    );
}

// 验证 InputGroup 拒绝无法兑现的绑定与子树。
#[test]
fn rejects_invalid_input_group_contracts() {
    // 字符串 value 不具备 State<String> 双向绑定所有权。
    let literal = generate(r#"<InputGroup value="12.50" />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量绑定必须失败");
    // 诊断必须明确文本状态类型。
    assert!(literal.message.contains("State<String>"));
    // InputGroup 子节点不能被生成器丢弃。
    let child = generate(r#"<InputGroup><Text>lost</Text></InputGroup>"#)
        // 子树形状必须失败。
        .expect_err("InputGroup 子节点必须失败");
    // 诊断必须说明叶组合边界。
    assert!(child.message.contains("不接受子节点"));
}
