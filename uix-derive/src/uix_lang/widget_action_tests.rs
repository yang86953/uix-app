// 引入文档解析与组件感知代码生成入口。
use super::{Declaration, ExpressionKind, generate_document_view, parse_document};

// 去除生成令牌中的空白，稳定核对静态 action 展开结果。
fn normalized(tokens: impl ToString) -> String {
    // Rust 令牌空白不承载语义。
    tokens
        // 转换为文本。
        .to_string()
        // 删除全部空白字符。
        .chars()
        // 只保留令牌内容。
        .filter(|character| !character.is_whitespace())
        // 收集为可比较字符串。
        .collect()
}

// 验证 actions 属性保存无参数单表达式声明。
#[test]
fn parses_ordered_widget_actions() {
    // 解析两个互相独立的同步 action。
    let document = parse_document(
        r#"<Widget name="Counter" state="count: 0" actions="increment: setState(count: count + 1), reset: setState(count: 0)"><Button @click="increment()">加一</Button></Widget><Counter />"#,
    )
    // 合法 action 声明必须解析成功。
    .expect("同步 action 声明应成功解析");
    // 提取组件声明。
    let Declaration::Widget(widget) = &document.declarations[0] else {
        // 结构不匹配时失败。
        panic!("首个声明应为 Widget");
    };
    // action 必须保持源码顺序。
    assert_eq!(widget.actions.len(), 2);
    // 首个 action 名称必须保存。
    assert_eq!(widget.actions[0].name, "increment");
    // action 主体必须复用受限表达式调用 AST。
    assert!(matches!(
        // 检查首个 action 表达式形状。
        widget.actions[0].expression.kind,
        // setState 解析为普通调用，后续在事件位置降低。
        ExpressionKind::Call { .. }
    ));
}

// 验证事件中的 action 在编译期内联为既有 setState Rust 调用。
#[test]
fn expands_widget_action_without_runtime_dispatch() {
    // 构造私有状态 action 与调用事件。
    let document = parse_document(
        r#"<Widget name="Counter" state="count: 0" actions="increment: setState(count: count + 1)"><Button @click="increment()">{count}</Button></Widget><Counter />"#,
    )
    // 声明必须先通过解析。
    .expect("action 文档应成功解析");
    // 生成最终 Rust 令牌。
    let generated =
        normalized(generate_document_view(&document).expect("action 应静态生成 Rust 事件处理器"));
    // 生成物必须包含 State 写回。
    assert!(generated.contains(".set("));
    // 作者 action 名不得进入运行时符号查找或函数调用。
    assert!(!generated.contains("increment()"));
    // 现有事件入口必须继续承载生成的处理器。
    assert!(generated.contains(".on_click_fn("));
}

// 验证 action 只能在事件位置调用且第一阶段不接收参数。
#[test]
fn rejects_action_outside_event_or_with_arguments() {
    // 在文本插值中调用 action。
    let outside = parse_document(
        r#"<Widget name="Bad" actions="value: 1"><Text>{value()}</Text></Widget><Bad />"#,
    )
    // 语法本身允许解析，作用域由展开阶段诊断。
    .expect("action 作用域应在生成阶段验证");
    // 非事件调用必须失败。
    let outside = generate_document_view(&outside).expect_err("action 不得在插值中调用");
    // 诊断必须说明事件边界。
    assert!(outside.message.contains("只能在 Widget 的事件处理器中调用"));

    // 在事件中给无参数 action 传值。
    let arguments = parse_document(
        r#"<Widget name="Bad" actions="save: true"><Button @click="save(1)">保存</Button></Widget><Bad />"#,
    )
    // 参数形状由 action 展开器验证。
    .expect("action 参数边界应在生成阶段验证");
    // 带参数调用必须失败。
    let arguments = generate_document_view(&arguments).expect_err("首阶段 action 不得接收参数");
    // 诊断必须包含 action 名称与参数限制。
    assert!(arguments.message.contains("action save 当前不接受参数"));
}

// 验证 action 名称唯一、不能遮蔽字段且不能隐式捕获事件。
#[test]
fn rejects_invalid_widget_action_declarations() {
    // 重复 action 名称必须在声明期失败。
    let duplicate = parse_document(
        r#"<Widget name="Bad" actions="save: true, save: false"><Text>A</Text></Widget><Bad />"#,
    )
    // 重复名称不得进入生成阶段。
    .expect_err("重复 action 名称必须失败");
    // 诊断必须指出具体 action。
    assert!(duplicate.message.contains("action save 重复声明"));

    // action 不得与 state 同名。
    let collision = parse_document(
        r#"<Widget name="Bad" state="save: false" actions="save: setState(save: true)"><Text>A</Text></Widget><Bad />"#,
    )
    // 名称冲突必须失败。
    .expect_err("action 与 state 重名必须失败");
    // 诊断必须说明共享名称空间。
    assert!(collision.message.contains("action save 与组件字段同名"));

    // 无参数 action 不得隐式读取事件载荷。
    let event = parse_document(
        r#"<Widget name="Bad" actions="save: notify($event.x)"><Button @click="save()">保存</Button></Widget><Bad />"#,
    )
    // 隐式事件捕获必须在声明期失败。
    .expect_err("无参数 action 不得捕获事件");
    // 诊断必须说明 $event 边界。
    assert!(event.message.contains("不能隐式引用 $event"));
}

// 验证 action 直接与间接递归在静态展开时给出闭环诊断。
#[test]
fn rejects_recursive_widget_actions() {
    // 构造两个 action 的循环调用。
    let document = parse_document(
        r#"<Widget name="Bad" actions="first: second(), second: first()"><Button @click="first()">运行</Button></Widget><Bad />"#,
    )
    // 单个声明表达式本身合法。
    .expect("action 调用闭环应在静态展开阶段发现");
    // 展开必须拒绝循环。
    let error = generate_document_view(&document).expect_err("递归 action 不得生成 Rust");
    // 诊断必须给出完整闭环。
    assert!(error.message.contains("first -> second -> first"));
}
