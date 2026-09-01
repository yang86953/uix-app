// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Tag 预设颜色、能力与公共属性生成。
#[test]
fn generates_tag_contract() {
    // 生成覆盖完整静态契约的标签。
    let snapshot = generate(
        r#"<Tag color="success" closable checkable width="160px" automationId="state">已完成</Tag>"#,
    )
    // 合法标签必须成功生成。
    .expect("文档属性应映射到公开 Tag API");
    // 正文必须进入公开构造器。
    assert!(snapshot.contains("Tag :: new (\"已完成\")"));
    // 颜色必须映射到公开预设枚举。
    assert!(snapshot.contains("TagColor :: Success"));
    // 可关闭能力必须启用。
    assert!(snapshot.contains("closable ()"));
    // 可勾选能力必须启用。
    assert!(snapshot.contains("checkable (true)"));
    // 公共宽度必须继续映射。
    assert!(snapshot.contains("width (160.0)"));
    // 自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"));
    // 生成器必须进入 Tag 自己的 View/UIX 声明边界，不能直接绕过为叶节点。
    assert!(!snapshot.contains("ViewNode :: leaf"));
}

// 验证自定义颜色、动态颜色、动态能力与正文插值生成。
#[test]
fn generates_custom_and_dynamic_tag_values() {
    // 生成静态十六进制颜色。
    let custom = generate(r##"<Tag color="#336699">自定义</Tag>"##)
        // 合法十六进制颜色必须成功生成。
        .expect("自定义 Tag 颜色应生成");
    // 十六进制颜色必须进入公开 Color API。
    assert!(custom.contains("Color :: hex (\"#336699\")"));
    // 生成动态颜色、能力与正文插值。
    let dynamic = generate(
        r#"<Tag color={tag_color} closable={can_close} checkable={can_check}>状态：{label}</Tag>"#,
    )
    // 受限表达式必须成功生成。
    .expect("动态 Tag 配置应生成");
    // 动态颜色必须进入公开自定义颜色构建器。
    assert!(dynamic.contains("custom_color (tag_color)"));
    // 动态关闭能力必须生成同类型条件分支。
    assert!(dynamic.contains("if can_close") && dynamic.contains("closable ()"));
    // 动态可勾选能力必须进入公开构建器。
    assert!(dynamic.contains("checkable (can_check)"));
    // 正文插值必须按共享文本缓冲契约追加并转为字符串。
    assert!(
        dynamic.contains("push_str") && dynamic.contains("ToString") && dynamic.contains("label")
    );
}

// 验证语义色表达式映射到公开 TagColor 构建器。
#[test]
fn generates_dynamic_semantic_color_binding() {
    // 生成绑定语义色表达式的 Tag。
    let snapshot = generate(r#"<Tag semanticColor={status_color}>live</Tag>"#)
        .expect("语义色表达式应映射到公开构建器");
    // 语义色必须调用公开预设颜色入口。
    assert!(snapshot.contains("color (status_color)"), "{snapshot}");
    // 与具体色同时声明必须在编译期拒绝。
    let conflict = generate(r##"<Tag color="#999999" semanticColor={status_color}>live</Tag>"##)
        .expect_err("双色来源必须失败");
    assert!(conflict.message.contains("不能同时声明"));
    // 字面量语义色必须失败。
    let literal = generate(r#"<Tag semanticColor="success">live</Tag>"#)
        .expect_err("语义色字面量必须失败");
    assert!(literal.message.contains("TagColor 表达式"));
}

// 验证 Tag 拒绝未知颜色、非法十六进制与嵌套元素。
#[test]
fn rejects_invalid_tag_contracts() {
    // 未登记颜色不能被静默降级为默认色。
    let color = generate(r#"<Tag color="chartreuse">未知</Tag>"#)
        // 未知颜色必须失败。
        .expect_err("未登记颜色必须失败");
    // 诊断必须给出合法颜色入口。
    assert!(color.suggestion.contains("TagColor"));
    // 非法十六进制颜色不能进入运行时宽松解析。
    let hex = generate(r##"<Tag color="#12xz">非法</Tag>"##)
        // 非十六进制字符必须失败。
        .expect_err("非法十六进制颜色必须失败");
    // 诊断必须说明颜色不受支持。
    assert!(hex.message.contains("不受支持"));
    // 嵌套元素不能被当作标签正文丢弃。
    let child = generate(r#"<Tag><Icon name="star" /></Tag>"#)
        // 嵌套元素必须失败。
        .expect_err("Tag 嵌套元素必须失败");
    // 诊断必须指向文本与插值子节点边界。
    assert!(child.message.contains("只接受文本与插值"));
}
