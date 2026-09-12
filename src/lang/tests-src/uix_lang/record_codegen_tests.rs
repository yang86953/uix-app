// 引入确定映射。
use std::collections::BTreeMap;

// 引入 Compiler System 的稳定源码身份。
use crate::source_graph::SourceId;
// 引入 record 生成、来源上下文、文档解析与表达式解析入口。
use super::{
    Declaration, ExpressionKind, SourceSpan, generate_record_items, parse_document,
    parse_expression, with_source_markers,
};

// 构造独立表达式测试使用的绝对起点。
fn origin() -> SourceSpan {
    // 返回非零位置以验证跨度映射。
    SourceSpan {
        // 模拟文档内绝对字节起点。
        start: 300,
        // 表达式入口只使用起点定位。
        end: 300,
        // 模拟第三十行。
        line: 30,
        // 模拟第五列。
        column: 5,
    }
}

// 验证 record 声明生成模块级结构体且字段类型正确映射。
#[test]
fn generates_record_structs_for_uix_items() {
    // 解析带 record 声明的文档。
    let document = parse_document(
        r#"
<Record name="Profile" fields="email: String, level: String, accepted: bool, channel: String, notifications: bool, volume: number" />
<Widget name="Page" state="profile: Profile = { email: 'a@b.com', level: '中级', accepted: true, channel: '邮件', notifications: false, volume: 30.0 }">
  <Text>表单页</Text>
</Widget>
<Page />
"#,
    )
    // 合法文档必须成功。
    .expect("record 文档应成功解析");
    // 生成模块级结构体。
    let tokens = generate_record_items(&document)
        // 生成必须成功。
        .expect("record 生成应成功")
        // 转为快照文本。
        .to_string();
    // 快照必须包含结构体与 Clone 派生。
    assert!(tokens.contains("struct Profile"), "{tokens}");
    assert!(tokens.contains("Clone"), "{tokens}");
    // 快照必须包含全部公开字段。
    for field in [
        "email",
        "level",
        "accepted",
        "channel",
        "notifications",
        "volume",
    ] {
        // 逐字段验证。
        assert!(tokens.contains(field), "{field} 缺失: {tokens}");
    }
}

// 验证对象字面量解析保存嵌套空数组字段。
#[test]
fn parses_record_initial_object_with_array_fields() {
    // 解析含集合字段的对象字面量。
    let expression = parse_expression("{ labels: [], values: [] }", origin())
        // 合法对象必须成功。
        .expect("结构对象应成功解析");
    // 提取字段序列。
    let ExpressionKind::Object(fields) = &expression.kind else {
        // 非对象结构立即失败。
        panic!("应生成对象节点");
    };
    // 两个字段必须保持源码顺序。
    assert_eq!(fields.len(), 2);
    // 字段值必须是空数组。
    for field in fields {
        // 每个字段值都应为空数组。
        assert!(
            matches!(&field.value.kind, ExpressionKind::Array(items) if items.is_empty()),
            "字段 {} 应为空数组",
            field.name
        );
    }
}

// 验证 record 引用、重复声明与未知字段类型的拒绝路径。
#[test]
fn rejects_invalid_record_declarations_and_references() {
    // 未声明 record 的 state 引用必须失败。
    let error = parse_document(
        r#"<Widget name="Bad" state="profile: Profile = { a: 'x' }"><Text>A</Text></Widget><Bad />"#,
    )
    // 未声明引用必须失败。
    .expect_err("未声明 record 引用必须失败");
    // 诊断必须指出 record 未声明。
    assert!(
        error.message.contains("未在当前文档声明"),
        "{}",
        error.message
    );
    // record 与组件共享命名空间，重名必须失败。
    let error = parse_document(
        r#"<Record name="Same" fields="a: String" /><Widget name="Same"><Text>A</Text></Widget><Same />"#,
    )
    // 重名声明必须失败。
    .expect_err("record 与组件重名必须失败");
    // 诊断必须指向共享命名空间。
    assert!(error.message.contains("重复声明"), "{}", error.message);
    // 未知 record 字段类型（非 PascalCase）必须失败。
    let error = parse_document(
        r#"<Record name="Bad" fields="a: dateTime" /><Widget name="Page"><Text>A</Text></Widget><Page />"#,
    )
    // 未知字段类型必须失败。
    .expect_err("未知 record 字段类型必须失败");
    // 诊断必须指出字段类型不支持。
    assert!(
        error.message.contains("不支持 record 字段类型"),
        "{}",
        error.message
    );
    // record 字段引用未声明的 PascalCase 类型同样必须失败。
    let error = parse_document(
        r#"<Record name="Bad" fields="a: DateTime" /><Widget name="Page"><Text>A</Text></Widget><Page />"#,
    )
    // 未声明 record 引用必须失败。
    .expect_err("record 字段引用未声明类型必须失败");
    // 诊断必须指出 record 未声明。
    assert!(
        error.message.contains("未在当前文档声明"),
        "{}",
        error.message
    );
}

// 验证 Items 不依赖元素来源令牌也能保留 Record 的具名来源上下文。
#[test]
fn record_codegen_error_keeps_named_source_context() {
    // 先解析合法 Record，再只在测试内制造生成器必须拒绝的 Rust 关键字字段。
    let mut document =
        parse_document(r#"<Record name="Broken" fields="field: String" /><Text>根</Text>"#)
            .expect("初始 Record 必须合法");
    let field_span = match document.declarations.first_mut() {
        Some(Declaration::Record(record)) => {
            let field = record.fields.first_mut().expect("Record 必须有字段");
            field.name = "type".to_string();
            field.span
        }
        _ => panic!("首个声明必须是 Record"),
    };
    // 模拟根文件导入具名 Record 后的 Compiler System 来源登记。
    let root_source = SourceId::from_source_name("main.uix");
    let record_source = SourceId::from_source_name("helper.uix");
    let record_sources = BTreeMap::from([("Broken".to_string(), record_source)]);
    // Items 生成错误必须取得 Record 来源，而不是顶层根来源。
    let error = with_source_markers(
        root_source,
        BTreeMap::new(),
        record_sources,
        BTreeMap::new(),
        || generate_record_items(&document),
    )
    .expect_err("Rust 关键字字段必须被生成器拒绝");
    assert_eq!(error.source_id, Some(record_source));
    assert_eq!(error.span, field_span);
}
