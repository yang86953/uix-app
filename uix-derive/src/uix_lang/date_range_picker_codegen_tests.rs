// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 DateRangePicker 双日期绑定与公共属性生成。
#[test]
fn generates_bound_date_range_picker_contract() {
    // 故意反转字段源码顺序以验证语义绑定顺序稳定。
    let snapshot = generate(
        r#"<DateRangePicker value={{ end: range_end, start: range_start }} width="280px" automationId="range" />"#,
    )
    // 合法属性必须成功映射到公开 DateRangePicker API。
    .expect("文档属性应映射到公开 DateRangePicker API");
    // 构造器必须使用公开日期范围类型。
    let constructor = snapshot
        // 查找公开构造入口。
        .find("DateRangePicker :: new")
        // 失败时输出完整令牌便于定位格式漂移。
        .unwrap_or_else(|| panic!("应生成 DateRangePicker 构造器：{snapshot}"));
    // 起点状态必须借用给运行时 start 入口。
    let start = snapshot
        // 查找起点绑定。
        .find("start (& (range_start))")
        // 失败表示对象字段未正确消费。
        .expect("应生成 DateRangePicker 起点绑定");
    // 终点状态必须借用给运行时 end 入口。
    let end = snapshot
        // 查找终点绑定。
        .find("end (& (range_end))")
        // 失败表示对象字段未正确消费。
        .expect("应生成 DateRangePicker 终点绑定");
    // 生成器必须始终按构造、起点、终点顺序建立完整范围。
    assert!(constructor < start && start < end);
    // 公共宽度与自动化标识必须继续映射。
    assert!(snapshot.contains("width (280.0)") && snapshot.contains("automation_id (\"range\")"));
}

// 验证 DateRangePicker 拒绝不完整或越界的结构绑定。
#[test]
fn rejects_invalid_date_range_picker_value_shapes() {
    // 缺少 value 时无法建立两个受控端点。
    let missing_value = generate(r#"<DateRangePicker />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺少 value 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_value.message.contains("value"));
    // 普通状态表达式没有两个具名所有权句柄。
    let plain = generate(r#"<DateRangePicker value={range} />"#)
        // 普通绑定必须失败。
        .expect_err("普通表达式绑定必须失败");
    // 诊断必须明确对象契约。
    assert!(plain.message.contains("对象"));
    // 缺少 end 字段必须失败。
    let missing_end = generate(r#"<DateRangePicker value={{ start: range_start }} />"#)
        // 不完整范围必须失败。
        .expect_err("缺少 end 必须失败");
    // 诊断必须点名缺失字段。
    assert!(missing_end.message.contains("缺少 end"));
    // 额外字段不能被静默忽略。
    let extra = generate(
        r#"<DateRangePicker value={{ start: range_start, end: range_end, timezone: zone }} />"#,
    )
    // 未登记字段必须失败。
    .expect_err("额外字段必须失败");
    // 诊断必须点名非法字段。
    assert!(extra.message.contains("timezone"));
}

// 验证 DateRangePicker 叶组件形状。
#[test]
fn rejects_date_range_picker_children() {
    // 子节点不能被生成器静默丢弃。
    let child = generate(
        r#"<DateRangePicker value={{ start: range_start, end: range_end }}><Text>lost</Text></DateRangePicker>"#,
    )
    // 子树形状必须失败。
    .expect_err("DateRangePicker 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
