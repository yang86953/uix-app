// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Empty 描述、图标与公共属性生成。
#[test]
fn generates_empty_contract() {
    // 生成覆盖完整静态契约的空状态。
    let snapshot = generate(
        r#"<Empty description="暂无数据" icon="inbox" width="240px" automationId="empty" />"#,
    )
    // 合法空状态必须成功生成。
    .expect("文档属性应映射到公开 Empty API");
    // 描述必须进入公开构建器。
    assert!(snapshot.contains("description (\"暂无数据\")"));
    // 图标名必须进入公开构建器。
    assert!(snapshot.contains("icon (\"inbox\")"));
    // 公共宽度必须继续映射。
    assert!(snapshot.contains("width (240.0)"));
    // 自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"));
    // 生成器必须进入 Empty 自己的 View/UIX 声明边界，不能直接绕过为叶节点。
    assert!(!snapshot.contains("ViewNode :: leaf"));
}

// 验证动态文本保留公开 Into<String> 类型检查。
#[test]
fn generates_dynamic_empty_content() {
    // 生成动态描述与图标名配置。
    let snapshot = generate(r#"<Empty description={empty_description} icon={empty_icon} />"#)
        // 受限字符串表达式必须成功生成。
        .expect("动态 Empty 内容应生成");
    // 描述表达式必须进入公开构建器。
    assert!(snapshot.contains("description (empty_description)"));
    // 图标表达式必须进入公开构建器。
    assert!(snapshot.contains("icon (empty_icon)"));
}

// 验证 Empty 拒绝不能兑现的叶节点形状。
#[test]
fn rejects_empty_children() {
    // Empty 子节点不能被生成器丢弃。
    let child = generate(r#"<Empty><Text>lost</Text></Empty>"#)
        // 子树形状必须失败。
        .expect_err("Empty 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
