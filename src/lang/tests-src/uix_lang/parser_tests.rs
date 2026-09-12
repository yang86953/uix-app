//! `uix-lang-compiler/uix_lang/parser.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

// 引入待测解析入口和 AST 联合。
use super::{AttributeValue, Node, parse_document};

// 验证注释、属性、嵌套、自闭合、文本与插值保持顺序。
#[test]
fn parses_core_document_in_source_order() {
    // 构造覆盖核心语法的文档。
    let source = "/* header */\n<App theme=\"light\"><Text size={titleSize}>你好，{name}!</Text><Icon name=\"star\" /></App>";
    // 解析文档并要求成功。
    let document = parse_document(source).expect("核心文档应成功解析");
    // 根元素名称必须保留。
    assert_eq!(document.root.name, "App");
    // 根属性字面值必须去除双引号。
    assert_eq!(
        document.root.attributes[0].value,
        AttributeValue::Literal("light".to_string())
    );
    // 根元素必须保留两个有序子元素。
    assert_eq!(document.root.children.len(), 2);
    // 第一个子节点必须是 Text。
    let Node::Element(text) = &document.root.children[0] else {
        // 结构不匹配时主动失败。
        panic!("第一个子节点应为元素");
    };
    // 表达式属性必须保留源码。
    assert!(matches!(
        // 借用表达式属性以检查源码。
        &text.attributes[0].value,
        // 要求表达式节点保留原始内容。
        AttributeValue::Expression(node) if node.source == "titleSize"
    ));
    // 文本、插值、文本必须保持源顺序。
    assert!(matches!(&text.children[0], Node::Text(node) if node.value == "你好，"));
    // 中间节点必须是名称表达式。
    assert!(matches!(&text.children[1], Node::Interpolation(node) if node.source == "name"));
    // 尾部文本必须保留标点。
    assert!(matches!(&text.children[2], Node::Text(node) if node.value == "!"));
}
// 验证文本花括号转义不会被误判为插值。
#[test]
fn decodes_escaped_text_braces() {
    // 解析带转义花括号的文本。
    let document = parse_document("<Text>签名: \\{name\\}</Text>").expect("转义文本应成功");
    // 提取唯一文本节点。
    let Node::Text(text) = &document.root.children[0] else {
        // 结构不匹配时主动失败。
        panic!("应生成文本节点");
    };
    // 转义结果必须是字面花括号。
    assert_eq!(text.value, "签名: {name}");
}
// 验证 UTF-8 文本后的错误位置使用字符列而不是字节列。
#[test]
fn reports_utf8_line_and_column_for_mismatched_tag() {
    // 构造第二行中文文本后的错误结束标签。
    let source = "<App>\n  <Text>中文</Wrong>\n</App>";
    // 解析并取得预期诊断。
    let error = parse_document(source).expect_err("标签不匹配必须失败");
    // 错误必须定位到第二行。
    assert_eq!(error.span.line, 2);
    // 错误列必须按 Unicode 字符计算。
    assert_eq!(error.span.column, 13);
    // 原因必须包含实际与期望标签。
    assert!(error.message.contains("Wrong") && error.message.contains("Text"));
    // 修复建议必须给出正确结束标签。
    assert!(error.suggestion.contains("</Text>"));
}
// 验证文档严格执行唯一根元素约束。
#[test]
fn rejects_multiple_roots() {
    // 解析包含两个根元素的文档。
    let error = parse_document("<App /><Text />").expect_err("多个根必须失败");
    // 失败原因必须明确唯一根约束。
    assert!(error.message.contains("一个根元素"));
    // 解析缺失根元素的空文档。
    let error = parse_document("").expect_err("缺失根元素必须失败");
    // 失败原因必须明确指出根元素缺失。
    assert!(error.message.contains("缺少根元素"));
}
// 验证三类未闭合词法结构都提供修复建议。
#[test]
fn rejects_unclosed_lexical_structures() {
    // 逐项覆盖注释、字符串和表达式。
    for source in ["/* open", "<App name=\"open />", "<App name={value />"] {
        // 解析并取得诊断。
        let error = parse_document(source).expect_err("未闭合结构必须失败");
        // 每个诊断都必须包含原因。
        assert!(!error.message.is_empty());
        // 每个诊断都必须包含修复建议。
        assert!(!error.suggestion.is_empty());
    }
}

// 验证标签名必须遵循 PascalCase 起始约束。
#[test]
fn rejects_lowercase_tag_name() {
    // 解析小写标签并取得诊断。
    let error = parse_document("<button />").expect_err("小写标签必须失败");
    // 失败原因必须指出 PascalCase。
    assert!(error.message.contains("PascalCase"));
}
