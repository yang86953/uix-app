// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Radio 选项、状态绑定与公共属性生成。
#[test]
fn generates_bound_radio_contract() {
    // 生成文档登记的受控单选组。
    let snapshot = generate(
        r#"<Radio value={gender} options={gender_options} width="240px" automationId="gender" />"#,
    )
    // 合法单选组必须成功生成。
    .expect("文档属性应映射到公开 Radio API");
    // 构造器必须使用公开 Radio 入口。
    let constructor = snapshot
        // 查找公开构造器调用。
        .find("Radio :: new ()")
        // 失败时输出完整令牌便于定位格式漂移。
        .unwrap_or_else(|| panic!("应生成 Radio 构造器：{snapshot}"));
    // 选项表达式必须进入公开 options 构建器。
    let options = snapshot
        // 查找数据引用。
        .find("options ((gender_options) . clone ())")
        // 失败表示选项数据未保留。
        .expect("应生成 Radio 选项表达式");
    // 状态绑定必须借用 State<String> 句柄。
    let value = snapshot
        // 查找公开值绑定入口。
        .find("value (& (gender))")
        // 失败表示状态句柄未保留。
        .expect("应生成 Radio 值绑定");
    // 运行时契约要求先构造、设置选项，再同步受控值。
    assert!(constructor < options && options < value);
    // 公共宽度与自动化标识必须继续映射。
    assert!(snapshot.contains("width (240.0)") && snapshot.contains("automation_id (\"gender\")"));
}

// 验证 Radio 拒绝缺失属性、字面量与叶形状错误。
#[test]
fn rejects_invalid_radio_contracts() {
    // 缺失选项时无法构造文档声明的单选组。
    let missing_options = generate(r#"<Radio value={gender} />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺失 options 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_options.message.contains("options"));
    // 缺失状态时无法兑现双向绑定。
    let missing_value = generate(r#"<Radio options={gender_options} />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺失 value 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_value.message.contains("value"));
    // 字符串字面量不能提供选项集合。
    let literal_options = generate(r#"<Radio value={gender} options="female" />"#)
        // 字面量选项必须失败。
        .expect_err("字面量 options 必须失败");
    // 诊断必须说明可迭代字符串集合要求。
    assert!(literal_options.message.contains("可迭代字符串选项表达式"));
    // 字符串字面量不能提供 State<String> 所有权。
    let literal_value = generate(r#"<Radio value="female" options={gender_options} />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量 value 必须失败");
    // 诊断必须明确状态类型。
    assert!(literal_value.message.contains("State<String>"));
    // Radio 子节点不能被生成器丢弃。
    let child =
        generate(r#"<Radio value={gender} options={gender_options}><Text>lost</Text></Radio>"#)
            // 子树形状必须失败。
            .expect_err("Radio 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
