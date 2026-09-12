// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Table 类型化行列、运行配置与公共属性生成。
#[test]
fn generates_typed_table_contract() {
    // 生成覆盖全部首版专有属性的静态表格。
    let snapshot = generate(
        r#"<Table data={rows} columns={columns} virtual loading={loading} width="640px" automationId="orders" />"#,
    )
    // 合法 Table 必须成功生成。
    .expect("Table 首版类型化契约应生成");
    // 行集合必须统一收集为公开 TableRow。
    assert!(snapshot.contains("Vec < :: uix_app :: prelude :: TableRow >"));
    // 列集合必须统一收集为公开 TableColumn。
    assert!(snapshot.contains("Vec < :: uix_app :: prelude :: TableColumn >"));
    // 运行时配置必须映射到已有公开构建器。
    assert!(snapshot.contains("virtual_scroll (true)") && snapshot.contains("loading (loading)"));
    // 公共尺寸和自动化身份继续由公共属性层消费。
    assert!(snapshot.contains("width (640.0)") && snapshot.contains("automation_id"));
}

// 验证 Table 必需数据与类型化表达式诊断。
#[test]
fn rejects_invalid_table_data_and_columns() {
    // 缺少 data 时没有运行时行集合。
    let missing_data = generate(r#"<Table columns={columns} />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 data 必须失败");
    // 诊断必须点名 data。
    assert!(missing_data.message.contains("data"));
    // 字符串不能伪装成 TableRow 集合。
    let literal_data = generate(r#"<Table data="rows" columns={columns} />"#)
        // 非表达式 data 必须失败。
        .expect_err("字符串 data 必须失败");
    // 诊断必须说明 TableRow 类型。
    assert!(literal_data.message.contains("TableRow"));
    // 缺少 columns 时没有运行时列定义。
    let missing_columns = generate(r#"<Table data={rows} />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 columns 必须失败");
    // 诊断必须点名 columns。
    assert!(missing_columns.message.contains("columns"));
    // 字符串不能伪装成 TableColumn 集合。
    let literal_columns = generate(r#"<Table data={rows} columns="columns" />"#)
        // 非表达式 columns 必须失败。
        .expect_err("字符串 columns 必须失败");
    // 诊断必须说明 TableColumn 类型。
    assert!(literal_columns.message.contains("TableColumn"));
}

// 验证 Table 叶形状与未登记选择契约继续拒绝。
#[test]
fn rejects_table_children_and_unregistered_selection() {
    // Table 自身绘制所有单元格，不能接受 UIX 子树。
    let child = generate(r#"<Table data={rows} columns={columns}><Text>非法</Text></Table>"#)
        // 可见子节点必须失败。
        .expect_err("Table 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // rowSelection 尚无受控状态与事件闭环。
    let selection = generate(r#"<Table data={rows} columns={columns} rowSelection={selection} />"#)
        // 未登记对象契约必须失败。
        .expect_err("rowSelection 不得被 bool 近似");
    // 诊断必须保留具体属性名。
    assert!(selection.message.contains("rowSelection"));
}
