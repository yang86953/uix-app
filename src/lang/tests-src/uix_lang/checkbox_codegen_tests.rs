// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Checkbox 文本、禁用、状态绑定与公共属性生成。
#[test]
fn generates_bound_checkbox_contract() {
    // 生成覆盖完整静态契约的复选框。
    let snapshot =
        generate(r#"<Checkbox checked={accepted} disabled text="同意协议" width="180px" />"#)
            // 合法复选框必须成功生成。
            .expect("文档属性应映射到公开 Checkbox API");
    // 标签必须进入公开构造器。
    let label = snapshot
        // 查找包含中文文本的构造器。
        .find("Checkbox :: new (\"同意协议\")")
        // 失败时输出完整令牌便于定位格式漂移。
        .unwrap_or_else(|| panic!("应生成 Checkbox 标签：{snapshot}"));
    // 禁用构建器必须存在。
    let disabled = snapshot.find("disabled (true)").expect("应生成禁用状态");
    // 状态绑定必须存在。
    let checked = snapshot
        // 查找公开勾选绑定入口。
        .find("checked (& (accepted))")
        // 失败表示状态句柄未保留。
        .expect("应生成勾选状态绑定");
    // 构建顺序必须先确定标签与禁用配置再绑定状态。
    assert!(label < disabled && disabled < checked);
    // 公共宽度必须继续映射。
    assert!(snapshot.contains("width (180.0)"));
}

// 验证动态文本与禁用布尔值保留公开 Rust 类型检查。
#[test]
fn generates_dynamic_checkbox_values() {
    // 生成动态标签与禁用配置。
    let snapshot =
        generate(r#"<Checkbox checked={accepted} disabled={locked} text={agreement_text} />"#)
            // 动态配置必须成功生成。
            .expect("动态复选框配置应生成");
    // 动态文本必须进入公开构造器。
    assert!(snapshot.contains("Checkbox :: new (agreement_text)"));
    // 动态布尔值必须进入禁用构建器。
    assert!(snapshot.contains("disabled (locked)"));
}

// 验证 Checkbox 勾选提交事件的文本载荷生成。
#[test]
fn generates_checkbox_change_event() {
    // 生成带勾选提交事件的复选框。
    let snapshot = generate(
        r#"<Checkbox text="自动匹配" checked={enabled} @change="on_enabled_change($event)" />"#,
    )
    // 合法事件必须成功生成。
    .expect("Checkbox @change 应映射到公开 View Change 入口");
    // Change 事件必须把勾选文本载荷交给处理器。
    assert!(
        snapshot.contains("on_change_fn")
            && snapshot.contains("(on_enabled_change) (__uix_change_value)"),
        "{snapshot}"
    );
}

// 验证 Checkbox 拒绝无法兑现的绑定、布尔值与叶形状。
#[test]
fn rejects_invalid_checkbox_contracts() {
    // 布尔字面量不具备 State<bool> 双向绑定所有权。
    let literal = generate(r#"<Checkbox checked="true" />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量绑定必须失败");
    // 诊断必须明确状态类型。
    assert!(literal.message.contains("State<bool>"));
    // 非布尔禁用值必须由共享规则拒绝。
    let disabled = generate(r#"<Checkbox disabled="yes" />"#)
        // 非布尔值必须失败。
        .expect_err("非布尔禁用值必须失败");
    // 诊断必须说明布尔要求。
    assert!(disabled.message.contains("布尔"));
    // Checkbox 子节点不能被生成器丢弃。
    let child = generate(r#"<Checkbox><Text>lost</Text></Checkbox>"#)
        // 子树形状必须失败。
        .expect_err("Checkbox 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
