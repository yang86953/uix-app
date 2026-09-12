// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 DatePicker 日期绑定、选择粒度与公共属性生成。
#[test]
fn generates_bound_date_picker_contract() {
    // 生成包含季度粒度和公共属性的受控日期选择器。
    let snapshot = generate(
        r#"<DatePicker value={selected_date} mode="quarter" width="240px" automationId="date" />"#,
    )
    // 合法属性必须成功映射到公开 DatePicker API。
    .expect("文档属性应映射到公开 DatePicker API");
    // 构造器必须使用公开日期选择类型。
    assert!(snapshot.contains("DatePicker :: new"));
    // 日期状态必须借用给运行时受控入口。
    assert!(snapshot.contains("value (& (selected_date))"));
    // quarter 关键字必须映射到公开粒度枚举。
    assert!(snapshot.contains("PickerMode :: Quarter"));
    // 公共宽度与自动化标识必须继续映射。
    assert!(snapshot.contains("width (240.0)") && snapshot.contains("automation_id (\"date\")"));
}

// 验证所有文档化 DatePicker 选择粒度。
#[test]
fn generates_all_date_picker_modes() {
    // 逐项验证关键字与公开枚举变体的稳定映射。
    for (keyword, variant) in [
        // date 映射默认日期粒度。
        ("date", "Date"),
        // week 映射周粒度。
        ("week", "Week"),
        // month 映射月粒度。
        ("month", "Month"),
        // quarter 映射季度粒度。
        ("quarter", "Quarter"),
    ] {
        // 为当前关键字生成最小合法文档。
        let source = format!(r#"<DatePicker value={{selected_date}} mode="{keyword}" />"#);
        // 每个登记关键字都必须成功生成。
        let snapshot = generate(&source).expect("登记的 DatePicker mode 应成功生成");
        // 生成结果必须包含对应公开枚举变体。
        assert!(snapshot.contains(&format!("PickerMode :: {variant}")));
    }
}

// 验证 DatePicker 拒绝缺失绑定、字面量、非法粒度与子节点。
#[test]
fn rejects_invalid_date_picker_contracts() {
    // 缺少日期状态时无法兑现双向绑定。
    let missing_value = generate(r#"<DatePicker />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺少 value 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_value.message.contains("value"));
    // 字符串字面量不能提供 State<Date> 句柄。
    let literal_value = generate(r#"<DatePicker value="2026-08-10" />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量 value 必须失败");
    // 诊断必须明确状态类型。
    assert!(literal_value.message.contains("State<Date>"));
    // 未登记粒度不能静默回退到默认日期模式。
    let invalid_mode = generate(r#"<DatePicker value={selected_date} mode="year" />"#)
        // 非法枚举关键字必须失败。
        .expect_err("非法 mode 必须失败");
    // 诊断必须指出非法值和允许集合。
    assert!(invalid_mode.message.contains("year"));
    // 动态表达式不能绕过编译期粒度集合。
    let dynamic_mode = generate(r#"<DatePicker value={selected_date} mode={picker_mode} />"#)
        // 非字面量 mode 必须失败。
        .expect_err("表达式 mode 必须失败");
    // 诊断必须说明字面量约束。
    assert!(dynamic_mode.message.contains("字符串字面量"));
    // DatePicker 子节点不能被生成器丢弃。
    let child = generate(r#"<DatePicker value={selected_date}><Text>lost</Text></DatePicker>"#)
        // 子树形状必须失败。
        .expect_err("DatePicker 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
