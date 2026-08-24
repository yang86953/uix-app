// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 ResultView 状态、文本与公共属性生成。
#[test]
fn generates_result_view_contract() {
    // 生成覆盖完整静态契约的未找到结果页。
    let snapshot = generate(
        r#"<ResultView status="404" title="页面不存在" extraText="返回首页" width="320px" automationId="result" />"#,
    )
    // 合法结果页必须成功生成。
    .expect("文档属性应映射到公开 ResultView API");
    // 状态必须映射到公开枚举。
    assert!(snapshot.contains("ResultType :: NotFound"));
    // 标题必须进入公开构建器。
    assert!(snapshot.contains("title (& * (\"页面不存在\"))"));
    // 辅助文字必须进入公开构建器。
    assert!(snapshot.contains("extra_text (\"返回首页\")"));
    // 公共宽度必须继续映射。
    assert!(snapshot.contains("width (320.0)"));
    // 自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"));
    // 生成器必须进入 ResultView 自己的 View/UIX 声明边界，不能直接绕过为叶节点。
    assert!(!snapshot.contains("ViewNode :: leaf"));
}

// 验证缺省状态与动态文本保留公开 Rust 类型检查。
#[test]
fn generates_default_status_and_dynamic_text() {
    // 生成缺省成功状态及动态文本。
    let snapshot = generate(r#"<ResultView title={result_title} extraText={result_action} />"#)
        // 受限字符串表达式必须成功生成。
        .expect("动态 ResultView 文本应生成");
    // 缺省状态必须是成功结果。
    assert!(snapshot.contains("ResultType :: Success"));
    // 标题表达式必须以字符串借用进入公开构建器。
    assert!(snapshot.contains("title (& * (result_title))"));
    // 辅助文字表达式必须进入公开构建器。
    assert!(snapshot.contains("extra_text (result_action)"));
}

// 验证 ResultView 拒绝未知状态、动态状态与子节点。
#[test]
fn rejects_invalid_result_view_contracts() {
    // 未登记状态不能被静默降级为成功结果。
    let status = generate(r#"<ResultView status="pending" />"#)
        // 非法关键字必须失败。
        .expect_err("未登记状态必须失败");
    // 诊断必须给出合法集合。
    assert!(
        status
            .suggestion
            .contains("success、error、info、warning 或 404")
    );
    // 动态状态无法确定映射到哪个公开枚举。
    let dynamic = generate(r#"<ResultView status={result_status} />"#)
        // 动态关键字必须失败。
        .expect_err("动态状态必须失败");
    // 诊断必须说明字符串字面量要求。
    assert!(dynamic.message.contains("字符串字面量"));
    // ResultView 子节点不能被生成器丢弃。
    let child = generate(r#"<ResultView><Text>lost</Text></ResultView>"#)
        // 子树形状必须失败。
        .expect_err("ResultView 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
