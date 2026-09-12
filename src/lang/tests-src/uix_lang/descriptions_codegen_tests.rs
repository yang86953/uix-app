// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
    // 结束测试生成辅助函数。
}

// 验证 Descriptions 数据、列数与公共属性的完整生成契约。
#[test]
// 声明完整 Descriptions 生成测试。
fn generates_descriptions_contract() {
    // 生成覆盖必需数据、静态列数和公共属性的描述列表。
    let snapshot = generate(r#"<Descriptions data={description_items} columns="2" width="480px" automationId="profile-details" />"#)
        // 合法描述列表必须成功生成。
        .expect("文档属性应映射到公开 Descriptions API");
    // 描述列表必须从公开构造器开始。
    assert!(snapshot.contains("Descriptions :: new ()"));
    // 数据表达式必须通过拥有所有权的 IntoIterator 统一收集。
    assert!(snapshot.contains("IntoIterator :: into_iter ((description_items) . clone ())"));
    // 收集目标必须锁定公开 DescriptionsItem 类型。
    assert!(snapshot.contains("DescriptionsItem"));
    // 静态列数必须进入公开 column 构建器。
    assert!(snapshot.contains("column (2usize)"));
    // 生成器必须进入 Descriptions 自己的 UIX 根声明，不能直接构造运行时叶节点。
    assert!(!snapshot.contains("ViewNode :: leaf"), "{snapshot}");
    // 公共宽度必须继续映射。
    assert!(snapshot.contains("width (480.0)"));
    // 自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"));
    // 结束完整 Descriptions 生成测试。
}

// 验证文档默认列数与动态 usize 列数。
#[test]
// 声明默认值和动态列数测试。
fn generates_default_and_dynamic_description_columns() {
    // 未声明 columns 时生成显式一列默认值。
    let default_columns = generate(r#"<Descriptions data={description_items} />"#)
        // 最小合法描述列表必须成功生成。
        .expect("缺省列数应使用文档默认值");
    // 默认值必须固定为一列而非运行时默认三列。
    assert!(default_columns.contains("column (1usize)"));
    // 动态列数由 Rust 核对 usize 类型。
    let dynamic_columns =
        generate(r#"<Descriptions data={description_items} columns={column_count} />"#)
            // 受限标识符表达式必须成功生成。
            .expect("动态 usize 列数应进入公开构建器");
    // 动态表达式不得在宏层被改写为浮点数。
    assert!(dynamic_columns.contains("column (column_count)"));
    // 结束默认值和动态列数测试。
}

// 验证 Descriptions 必需数据与数据表达式形状诊断。
#[test]
// 声明数据错误测试。
fn rejects_invalid_descriptions_data() {
    // 缺失 data 时没有运行时内容来源。
    let missing = generate(r#"<Descriptions />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 data 必须被拒绝");
    // 诊断必须点名 data。
    assert!(missing.message.contains("data"));
    // 字符串不能伪装成类型化集合。
    let literal = generate(r#"<Descriptions data="items" />"#)
        // 非表达式 data 必须失败。
        .expect_err("字符串 data 必须被拒绝");
    // 诊断必须说明 DescriptionsItem 集合要求。
    assert!(literal.message.contains("DescriptionsItem"));
    // 修复建议必须给出花括号数据引用。
    assert!(literal.suggestion.contains("data={description_items}"));
    // 结束数据错误测试。
}

// 验证 Descriptions 静态列数与叶节点边界诊断。
#[test]
// 声明列数与子树错误测试。
fn rejects_invalid_descriptions_shape() {
    // 零列不能形成有效描述列表网格。
    let zero = generate(r#"<Descriptions data={items} columns="0" />"#)
        // 零列必须失败。
        .expect_err("零列必须被拒绝");
    // 诊断必须要求大于零。
    assert!(zero.message.contains("大于零"));
    // 小数列数不能转换为 usize。
    let fraction = generate(r#"<Descriptions data={items} columns="1.5" />"#)
        // 非整数列数必须失败。
        .expect_err("小数列数必须被拒绝");
    // 修复建议必须包含静态和动态合法形状。
    assert!(
        fraction.suggestion.contains("columns=\"2\"")
            && fraction.suggestion.contains("column_count")
    );
    // Descriptions 自身绘制数据项，不能接受 View 子树。
    let child = generate(r#"<Descriptions data={items}><Text>非法</Text></Descriptions>"#)
        // 嵌套元素必须失败。
        .expect_err("Descriptions 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // 结束列数与子树错误测试。
}
