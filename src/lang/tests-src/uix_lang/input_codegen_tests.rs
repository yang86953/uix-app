// 引入解析与核心生成入口。
use super::{generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, super::Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Input 全部文档属性与 Change 载荷生成。
#[test]
fn generates_bound_password_input_with_change_payload() {
    // 生成覆盖绑定、密码、禁用简写、事件载荷和公共尺寸的 Input。
    let snapshot = generate(
        r#"<Input value={name} placeholder="请输入名称" type="password" disabled @change="on_name_change($event)" width="240px" />"#,
    )
    // 合法 Input 必须成功生成。
    .expect("文档属性应映射到公开 Input API");
    // 绑定必须借用 State<String> 表达式。
    assert!(snapshot.contains("value (& (name))"));
    // 密码、占位文本和禁用状态必须进入公开构建链。
    assert!(
        snapshot.contains("Input :: password")
            && snapshot.contains("placeholder (\"请输入名称\")")
            && snapshot.contains("disabled (true)")
    );
    // Change 事件必须把文本载荷交给处理器。
    assert!(
        snapshot.contains("on_change_fn")
            && snapshot.contains("(on_name_change) (__uix_change_value)"),
        "{snapshot}"
    );
    // 公共宽度必须在专有事件之后继续映射。
    assert!(snapshot.contains("width (240.0)"));
}

// 验证三种文档输入类型都使用精确构造器。
#[test]
fn maps_text_textarea_and_password_constructors() {
    // 默认类型必须使用单行构造器。
    let text = generate(r#"<Input />"#).expect("默认文本输入应生成");
    // textarea 必须使用多行构造器。
    let textarea = generate(r#"<Input type="textarea" />"#).expect("多行输入应生成");
    // password 必须使用密码构造器。
    let password = generate(r#"<Input type="password" />"#).expect("密码输入应生成");
    // 三类构造器不得互相近似。
    assert!(
        text.contains("Input :: new")
            && textarea.contains("Input :: textarea")
            && password.contains("Input :: password")
    );
}

// 验证行数与长度上限映射到公开 usize 构建器。
#[test]
fn maps_rows_and_max_length_builders() {
    // 生成带行数与长度上限的多行输入。
    let snapshot = generate(
        r#"<Input type="textarea" rows="2" maxLength="8192" value={goal} />"#,
    )
    .expect("行数与长度上限应映射到公开构建器");
    // 行数必须进入公开构建链并保持 usize 字面量。
    assert!(
        snapshot.contains("rows (2usize)") || snapshot.contains("rows (2)"),
        "{snapshot}"
    );
    // 长度上限必须进入公开构建链。
    assert!(
        snapshot.contains("max_length (8192usize)") || snapshot.contains("max_length (8192)"),
        "{snapshot}"
    );
    // 非整数行数必须在编译期拒绝。
    let invalid = generate(r#"<Input rows="wide" />"#).expect_err("非整数行数必须失败");
    // 诊断必须指向 usize 语义。
    assert!(invalid.message.contains("usize"));
}

// 验证 Input 拒绝无法兑现的绑定、类型与子树。
#[test]
fn rejects_invalid_input_shapes() {
    // 字符串 value 不具备 State<String> 双向绑定所有权。
    let literal = generate(r#"<Input value="name" />"#).expect_err("字面量绑定必须失败");
    // 诊断必须明确状态类型。
    assert!(literal.message.contains("State<String>"));
    // 未登记输入类型不得静默降级为 text。
    let kind = generate(r#"<Input type="search" />"#).expect_err("未知类型必须失败");
    // 诊断必须列出文档登记类型。
    assert!(
        kind.message.contains("Input type")
            && kind.suggestion.contains("text、textarea 或 password")
    );
    // Input 子节点不能被生成器丢弃。
    let child = generate(r#"<Input><Text>lost</Text></Input>"#)
        // 子树形状必须失败。
        .expect_err("Input 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
