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

// 验证 Timeline 数据、状态方向与公共属性的完整生成契约。
#[test]
// 声明完整 Timeline 生成测试。
fn generates_timeline_contract() {
    // 生成覆盖必需数据、两个布尔能力和公共属性的时间轴。
    let snapshot = generate(r#"<Timeline items={timeline_items} pending="true" reverse={is_reversed} width="480px" automationId="release-timeline" />"#)
        // 合法时间轴必须成功生成。
        .expect("文档属性应映射到公开 Timeline API");
    // 时间轴必须从公开构造器开始。
    assert!(snapshot.contains("Timeline :: new ()"));
    // 数据表达式必须通过拥有所有权的 IntoIterator 统一收集。
    assert!(snapshot.contains("IntoIterator :: into_iter ((timeline_items) . clone ())"));
    // 收集目标必须锁定公开 TimelineItem 类型。
    assert!(snapshot.contains("TimelineItem"));
    // 静态 pending 必须进入公开构建器。
    assert!(snapshot.contains("pending (true)"));
    // 动态 reverse 必须保留给 Rust 核对 bool 类型。
    assert!(snapshot.contains("reverse (is_reversed)"));
    // 组件必须物化为公开叶节点。
    assert!(snapshot.contains("ViewNode :: leaf"));
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (480.0)") && snapshot.contains("automation_id"));
    // 结束完整 Timeline 生成测试。
}

// 验证 Timeline 两个布尔属性的显式默认值。
#[test]
// 声明 Timeline 默认值测试。
fn generates_timeline_boolean_defaults() {
    // 生成最小合法时间轴。
    let snapshot = generate(r#"<Timeline items={timeline_items} />"#)
        // 最小属性集合必须成功生成。
        .expect("缺省布尔属性应使用文档默认值");
    // pending 缺省值必须显式固定为 false。
    assert!(snapshot.contains("pending (false)"));
    // reverse 缺省值必须显式固定为 false。
    assert!(snapshot.contains("reverse (false)"));
    // 结束默认值测试。
}

// 验证 Timeline 必需数据与数据表达式形状诊断。
#[test]
// 声明 Timeline 数据错误测试。
fn rejects_invalid_timeline_items() {
    // 缺失 items 时没有运行时内容来源。
    let missing = generate(r#"<Timeline />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 items 必须被拒绝");
    // 诊断必须点名 items。
    assert!(missing.message.contains("items"));
    // 字符串不能伪装成类型化集合。
    let literal = generate(r#"<Timeline items="events" />"#)
        // 非表达式 items 必须失败。
        .expect_err("字符串 items 必须被拒绝");
    // 诊断必须说明 TimelineItem 集合要求。
    assert!(literal.message.contains("TimelineItem"));
    // 修复建议必须给出花括号数据引用。
    assert!(literal.suggestion.contains("items={timeline_items}"));
    // 结束数据错误测试。
}

// 验证 Timeline 叶节点、布尔类型与专有属性边界。
#[test]
// 声明 Timeline 形状错误测试。
fn rejects_invalid_timeline_shape_and_attributes() {
    // Timeline 自身绘制数据项，不能接受 View 子树。
    let child = generate(r#"<Timeline items={items}><Text>非法</Text></Timeline>"#)
        // 嵌套元素必须失败。
        .expect_err("Timeline 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // 静态 pending 只接受布尔字面量。
    let pending = generate(r#"<Timeline items={items} pending="yes" />"#)
        // 非布尔 pending 必须失败。
        .expect_err("非法 pending 必须被拒绝");
    // 诊断必须说明布尔值要求。
    assert!(pending.message.contains("布尔"));
    // 未登记属性不能被公共映射静默忽略。
    let unknown = generate(r#"<Timeline items={items} reversed="true" />"#)
        // 拼写错误的属性必须失败。
        .expect_err("未知 Timeline 属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(unknown.message.contains("reversed"));
    // 结束形状错误测试。
}
