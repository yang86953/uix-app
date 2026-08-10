// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 RichText 动态内容、选择配置与公共属性生成。
#[test]
// 声明完整 RichText 生成测试。
fn generates_rich_text_contract() {
    // 生成覆盖动态内容、动态选择和公共属性的富文本。
    let snapshot = generate(
        r#"<RichText content={rich_content} selectable={allow_selection} width="480px" automationId="article-body" />"#,
    )
    // 合法富文本必须成功生成。
    .expect("文档属性应映射到公开 RichText API");
    // 生成层必须调用公开 Markdown 解析器并临时借用内容。
    assert!(snapshot.contains("parse_rich_text (& * (rich_content))"));
    // 解析结果必须交给运行时 content 构建器。
    assert!(snapshot.contains("content"));
    // 动态选择表达式必须进入运行时构建器。
    assert!(snapshot.contains("selectable (allow_selection)"));
    // 组件必须物化为公开叶节点。
    assert!(snapshot.contains("ViewNode :: leaf"));
    // 公共宽度与自动化标识仍由统一属性层消费。
    assert!(snapshot.contains("width") && snapshot.contains("automation_id"));
}

// 验证 RichText 文档默认值与布尔简写。
#[test]
// 声明 RichText 默认值测试。
fn generates_rich_text_documented_selectable_values() {
    // 生成只包含必需字面量内容的最小富文本。
    let default_snapshot = generate(r#"<RichText content="正文" />"#)
        // 最小合法富文本必须成功生成。
        .expect("缺省 selectable 应使用文档默认值");
    // 文档默认值必须显式映射为 false。
    assert!(default_snapshot.contains("selectable (false)"));
    // 生成使用布尔简写开启选择的富文本。
    let enabled_snapshot = generate(r#"<RichText content="正文" selectable />"#)
        // 合法布尔简写必须成功生成。
        .expect("selectable 简写应映射为 true");
    // 布尔简写必须显式进入运行时构建器。
    assert!(enabled_snapshot.contains("selectable (true)"));
}

// 验证 RichText 必需内容与叶节点边界。
#[test]
// 声明 RichText 结构错误测试。
fn rejects_invalid_rich_text_shape() {
    // 缺失 content 时没有可解析内容。
    let missing = generate(r#"<RichText />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 content 必须被拒绝");
    // 诊断必须点名 content。
    assert!(missing.message.contains("content"));
    // RichText 自身解析并绘制内容，不能接受 View 子树。
    let child = generate(r#"<RichText content="正文"><Text>非法</Text></RichText>"#)
        // 嵌套元素必须失败。
        .expect_err("RichText 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}

// 验证 RichText 布尔类型与专有属性边界。
#[test]
// 声明 RichText 属性错误测试。
fn rejects_invalid_rich_text_attributes() {
    // 非布尔字面量不能进入选择配置。
    let selectable = generate(r#"<RichText content="正文" selectable="yes" />"#)
        // 非法布尔值必须失败。
        .expect_err("非法 selectable 必须被拒绝");
    // 共享诊断必须说明布尔值要求。
    assert!(selectable.message.contains("布尔值"));
    // 未登记属性不能被公共映射静默忽略。
    let unknown = generate(r#"<RichText content="正文" markdown="true" />"#)
        // 拼写或扩展属性必须失败。
        .expect_err("未知 RichText 属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(unknown.message.contains("markdown"));
}
