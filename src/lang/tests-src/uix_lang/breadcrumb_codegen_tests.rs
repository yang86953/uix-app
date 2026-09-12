// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Breadcrumb 条目、末项当前页、分隔符、折叠阈值与公共属性生成。
#[test]
// 声明完整 Breadcrumb 生成测试。
fn generates_breadcrumb_contract() {
    // 生成覆盖类型化数据、动态分隔符、折叠阈值和公共属性的面包屑。
    let snapshot = generate(
        r#"<Breadcrumb items={breadcrumb_items} separator={separator_text} maxItems="3" width="480px" automationId="page-path" />"#,
    )
    // 合法 Breadcrumb 必须成功生成。
    .expect("文档属性应映射到公开 Breadcrumb API");
    // 数据表达式必须通过拥有权 IntoIterator 统一收集。
    assert!(snapshot.contains("IntoIterator :: into_iter ((breadcrumb_items) . clone ())"));
    // 收集目标必须锁定公开 BreadcrumbItem 类型。
    assert!(snapshot.contains("Vec < :: uix_app :: prelude :: BreadcrumbItem >"));
    // 末项当前页语义必须交给运行时窄构建契约。
    assert!(snapshot.contains("last_active ()"));
    // 动态分隔文本必须映射到公开 separator 构建器。
    assert!(snapshot.contains("separator (separator_text)"));
    // 静态折叠阈值必须映射为 usize 并调用公开 max_items 构建器。
    assert!(snapshot.contains("max_items (3usize)"));
    // Breadcrumb 必须生成叶 View。
    assert!(snapshot.contains("ViewNode :: leaf"));
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (480.0)") && snapshot.contains("automation_id"));
}

// 验证 Breadcrumb 必需数据诊断。
#[test]
// 声明 Breadcrumb 数据错误测试。
fn rejects_invalid_breadcrumb_data() {
    // 缺失 items 时没有路径数据来源。
    let missing = generate(r#"<Breadcrumb />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 items 必须被拒绝");
    // 诊断必须点名 items。
    assert!(missing.message.contains("items"));
    // 字符串不能伪装成类型化 BreadcrumbItem 集合。
    let literal = generate(r#"<Breadcrumb items="home" />"#)
        // 非表达式 items 必须失败。
        .expect_err("字符串 items 必须被拒绝");
    // 诊断必须说明 BreadcrumbItem 集合要求。
    assert!(literal.message.contains("BreadcrumbItem"));
    // 小数不能进入 usize 折叠阈值契约。
    let max_items = generate(r#"<Breadcrumb items={breadcrumb_items} maxItems="2.5" />"#)
        // 非整数阈值必须失败。
        .expect_err("小数 maxItems 必须被拒绝");
    // 诊断必须明确 usize 要求。
    assert!(max_items.message.contains("usize"));
}

// 验证 Breadcrumb 叶节点与专有属性边界。
#[test]
// 声明 Breadcrumb 形状错误测试。
fn rejects_invalid_breadcrumb_shape_and_attributes() {
    // Breadcrumb 自身绘制路径数据，不能接受 View 子树。
    let child = generate(r#"<Breadcrumb items={breadcrumb_items}><Text>非法</Text></Breadcrumb>"#)
        // 嵌套元素必须失败。
        .expect_err("Breadcrumb 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // activeIndex 尚未登记为 UIX 文档属性。
    let active = generate(r#"<Breadcrumb items={breadcrumb_items} activeIndex={active} />"#)
        // 文档外属性必须失败。
        .expect_err("未知 Breadcrumb 属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(active.message.contains("activeIndex"));
}
