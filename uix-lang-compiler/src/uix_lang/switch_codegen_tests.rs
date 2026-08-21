// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Switch 禁用、状态绑定与公共属性生成。
#[test]
fn generates_bound_switch_contract() {
    // 生成覆盖静态禁用与状态绑定契约的开关。
    let snapshot = generate(r#"<Switch checked={enabled} disabled width="180px" />"#)
        // 合法开关必须成功生成。
        .expect("文档属性应映射到公开 Switch API");
    // 构造器必须使用无标签的公开开关入口。
    let constructor = snapshot
        // 查找公开构造器调用。
        .find("Switch :: new ()")
        // 失败时输出完整令牌便于定位格式漂移。
        .unwrap_or_else(|| panic!("应生成 Switch 构造器：{snapshot}"));
    // 禁用构建器必须存在。
    let disabled = snapshot.find("disabled (true)").expect("应生成禁用状态");
    // 状态绑定必须存在。
    let checked = snapshot
        // 查找公开开关绑定入口。
        .find("checked (& (enabled))")
        // 失败表示状态句柄未保留。
        .expect("应生成开关状态绑定");
    // 构建顺序必须先构造与配置禁用，再绑定状态。
    assert!(constructor < disabled && disabled < checked);
    // 公共宽度必须继续映射。
    assert!(snapshot.contains("width (180.0)"));
}

// 验证动态禁用布尔值保留公开 Rust 类型检查。
#[test]
fn generates_dynamic_switch_disabled_value() {
    // 生成动态禁用配置。
    let snapshot = generate(r#"<Switch checked={enabled} disabled={locked} />"#)
        // 动态配置必须成功生成。
        .expect("动态开关配置应生成");
    // 动态布尔值必须进入禁用构建器。
    assert!(snapshot.contains("disabled (locked)"));
}

// 验证 Switch 拒绝无法兑现的绑定、布尔值、专属属性与叶形状。
#[test]
fn rejects_invalid_switch_contracts() {
    // 布尔字面量不具备 State<bool> 双向绑定所有权。
    let literal = generate(r#"<Switch checked="true" />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量绑定必须失败");
    // 诊断必须明确状态类型。
    assert!(literal.message.contains("State<bool>"));
    // 非布尔禁用值必须由共享规则拒绝。
    let disabled = generate(r#"<Switch disabled="yes" />"#)
        // 非布尔值必须失败。
        .expect_err("非布尔禁用值必须失败");
    // 诊断必须说明布尔要求。
    assert!(disabled.message.contains("布尔"));
    // Checkbox 专属 text 属性不能在 Switch 上被静默忽略。
    let text = generate(r#"<Switch text="不可见标签" />"#)
        // 未登记的专属属性必须失败。
        .expect_err("Switch 不应接受 Checkbox 文本属性");
    // 诊断必须指出未知属性映射。
    assert!(text.message.contains("text") && text.message.contains("尚无已登记"));
    // Switch 子节点不能被生成器丢弃。
    let child = generate(r#"<Switch><Text>lost</Text></Switch>"#)
        // 子树形状必须失败。
        .expect_err("Switch 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
