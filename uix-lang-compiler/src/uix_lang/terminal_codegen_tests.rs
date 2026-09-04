// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Terminal 数据、提示符与公共属性的完整生成契约。
#[test]
// 声明完整 Terminal 生成测试。
fn generates_terminal_contract() {
    // 生成覆盖类型化数据、静态提示符与公共属性的终端。
    let snapshot = generate(r#"<Terminal data={terminal_lines} prompt="$ " width="420px" automationId="console" />"#)
        // 合法终端必须成功生成。
        .expect("文档属性应映射到公开 Terminal API");
    // 数据表达式必须通过拥有所有权的 IntoIterator 统一收集。
    assert!(snapshot.contains("IntoIterator :: into_iter ((terminal_lines) . clone ())"));
    // 收集目标必须锁定公开 TerminalLine 类型。
    assert!(snapshot.contains("TerminalLine"));
    // 运行时构造器必须接收类型化输出行集合。
    assert!(snapshot.contains("Terminal :: new"));
    // 静态提示符必须进入公开构建器。
    assert!(snapshot.contains("prompt (\"$ \")"));
    // 组件必须物化为公开叶节点。
    assert!(snapshot.contains("ViewNode :: leaf"));
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (420.0)") && snapshot.contains("automation_id"));
}

// 验证 Terminal 必需数据与数据表达式形状诊断。
#[test]
// 声明 Terminal 数据错误测试。
fn rejects_invalid_terminal_data() {
    // 缺失 data 时没有运行时输出行来源。
    let missing = generate(r#"<Terminal />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 data 必须被拒绝");
    // 诊断必须点名 data。
    assert!(missing.message.contains("data"));
    // 字符串不能伪装成类型化集合。
    let literal = generate(r#"<Terminal data="lines" />"#)
        // 非表达式 data 必须失败。
        .expect_err("字符串 data 必须被拒绝");
    // 诊断必须说明 TerminalLine 集合要求。
    assert!(literal.message.contains("TerminalLine"));
    // 修复建议必须给出花括号数据引用。
    assert!(literal.suggestion.contains("data={terminal_lines}"));
}

// 验证 Terminal 叶节点与专有属性边界。
#[test]
// 声明 Terminal 形状错误测试。
fn rejects_invalid_terminal_shape_and_attributes() {
    // Terminal 自身绘制输出与命令行，不能接受 View 子树。
    let child = generate(r#"<Terminal data={lines}><Text>非法</Text></Terminal>"#)
        // 嵌套元素必须失败。
        .expect_err("Terminal 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // 未登记属性不能被公共映射静默忽略。
    let unknown = generate(r#"<Terminal data={lines} history="true" />"#)
        // 拼写错误的属性必须失败。
        .expect_err("未知 Terminal 属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(unknown.message.contains("history"));
}
