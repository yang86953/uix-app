// 引入文档解析与组件感知代码生成入口。
use super::{ActionBody, Declaration, ExpressionKind, generate_document_view, parse_document};

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
        widget.actions[0].body,
        // setState 解析为普通调用，后续在事件位置降低。
        ActionBody::Expression(super::Expression {
            kind: ExpressionKind::Call { .. },
            ..
        })
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
    let error = parse_document(
        r#"<Widget name="Bad" actions="first: second(), second: first()"><Button @click="first()">运行</Button></Widget><Bad />"#,
    )
    // 全部 action 在声明期建立静态调用图，即使尚无调用点也拒绝闭环。
    .expect_err("action 调用闭环应在声明期发现");
    // 诊断必须给出完整闭环。
    assert!(error.message.contains("first -> second -> first"));
}

// 验证 do 块保存独立语句 AST，并保持旧单表达式兼容。
#[test]
fn parses_structured_action_blocks() {
    let document = parse_document(
        r#"<Widget name="Counter" state="count: 0, limit: 10" actions="legacy: setState(count: 0), safe: do { let next = count + 1; if next > limit { setState(count: limit); return; } else { next = next + 1; } setState(count: next); return next; }"><Button @click="safe()">增加</Button></Widget><Counter />"#,
    )
    .expect("兼容表达式与结构化 action 应同时解析");
    let Declaration::Widget(widget) = &document.declarations[0] else {
        panic!("首个声明应为 Widget");
    };
    assert!(matches!(widget.actions[0].body, ActionBody::Expression(_)));
    let ActionBody::Block(block) = &widget.actions[1].body else {
        panic!("safe 应保存独立 action 块");
    };
    assert_eq!(block.statements.len(), 4);
}

// 验证局部赋值只标记实际目标为 mut，内层可遮蔽组件字段。
#[test]
fn lowers_lexical_locals_and_ordered_state_updates() {
    let document = parse_document(
        r#"<Widget name="Counter" state="count: 0" actions="run: do { let count = count + 1; let next = count; next = next + 1; setState(count: next); setState(count: next + 1); }"><Button @click="run()">运行</Button></Widget><Counter />"#,
    )
    .expect("局部遮蔽与赋值应通过语义验证");
    let generated = normalized(generate_document_view(&document).expect("do action 应生成 Rust"));
    assert!(generated.contains("letcount="));
    assert!(generated.contains("letmutnext="));
    assert!(!generated.contains("letmutcount="));
    let first = generated.find(".set(").expect("应生成首个 setState");
    let second = generated[first + 1..]
        .find(".set(")
        .map(|offset| offset + first + 1)
        .expect("应生成第二个 setState");
    assert!(first < second);
}

// 验证嵌套 action return 使用独立标签，不跳过调用方后续语句。
#[test]
fn isolates_nested_action_returns() {
    let document = parse_document(
        r#"<Widget name="Counter" state="count: 0" actions="inner: do { if count > 1 { return; } setState(count: 1); }, outer: do { inner(); setState(count: 2); return; }"><Button @click="outer()">运行</Button></Widget><Counter />"#,
    )
    .expect("嵌套 action 应通过静态调用图");
    let tokens = generate_document_view(&document).expect("嵌套 return 应生成 Rust");
    syn::parse2::<syn::Expr>(tokens.clone())
        .unwrap_or_else(|error| panic!("生成物不是合法 Rust 表达式：{error}\n{tokens}"));
    let generated = normalized(tokens);
    assert!(generated.contains("_inner:"));
    assert!(generated.contains("_outer:"));
    assert!(generated.matches("break'__uix_action_return_").count() >= 2);
}

// 验证块级重复 let 与非局部赋值在声明期给出定向诊断。
#[test]
fn rejects_invalid_action_local_bindings() {
    let duplicate = parse_document(
        r#"<Widget name="Bad" actions="run: do { let value = 1; let value = 2; }"><Button @click="run()">运行</Button></Widget><Bad />"#,
    )
    .expect_err("同块重复 let 必须失败");
    assert!(duplicate.message.contains("同一块重复声明局部 value"));

    let state_assignment = parse_document(
        r#"<Widget name="Bad" state="count: 0" actions="run: do { count = count + 1; }"><Button @click="run()">运行</Button></Widget><Bad />"#,
    )
    .expect_err("state 直接赋值必须失败");
    assert!(
        state_assignment
            .message
            .contains("state count 不能直接赋值")
    );

    let shared_state_assignment = parse_document(
        r#"<Widget name="Bad" props="count: State<number>" actions="run: do { count = count + 1; }"><Button @click="run()">运行</Button></Widget><Bad count={shared} />"#,
    )
    .expect_err("State<T> prop 直接赋值必须失败");
    assert!(
        shared_state_assignment
            .message
            .contains("state count 不能直接赋值"),
        "{}",
        shared_state_assignment.message
    );

    let unknown = parse_document(
        r#"<Widget name="Bad" actions="run: do { missing = 1; }"><Button @click="run()">运行</Button></Widget><Bad />"#,
    )
    .expect_err("未声明局部赋值必须失败");
    assert!(unknown.message.contains("赋值目标 missing 尚未声明"));
}

// 验证内层块允许遮蔽，初始化表达式仍读取外层同名绑定。
#[test]
fn allows_inner_shadowing_and_outer_initializer_reads() {
    let document = parse_document(
        r#"<Widget name="Counter" state="count: 0" actions="run: do { let value = count; if value >= 0 { let value = value + 1; setState(count: value); } setState(count: value); }"><Button @click="run()">运行</Button></Widget><Counter />"#,
    )
    .expect("内层遮蔽与外层初始化读取应合法");
    generate_document_view(&document).expect("遮蔽语义应生成确定 Rust");
}

// 验证无调用点的递归和 do 块内 $event 也在声明期拒绝。
#[test]
fn validates_every_action_before_reachability() {
    let recursion = parse_document(
        r#"<Widget name="Bad" actions="unused: do { unused(); }"><Text>静态</Text></Widget><Bad />"#,
    )
    .expect_err("不可达递归也必须失败");
    assert!(recursion.message.contains("unused -> unused"));

    let event = parse_document(
        r#"<Widget name="Bad" actions="run: do { notify($event.x); }"><Button @click="run()">运行</Button></Widget><Bad />"#,
    )
    .expect_err("do action 不得捕获事件");
    assert!(event.message.contains("不能隐式引用 $event"));
}

// 验证 action 内直接与间接 setStyle 都在实际事件 View 上闭合降低。
#[test]
fn binds_action_set_style_to_actual_event_view() {
    let document = parse_document(
        r#"base { color: red; } active { color: blue; } <Widget name="Styled" actions="paint: do { setStyle('active'); }, run: do { paint(); }"><Button class="base" @click="run()">切换</Button></Widget><Styled />"#,
    )
    .expect("间接 setStyle action 应解析");
    let tokens = generate_document_view(&document).expect("setStyle 应绑定实际 Button");
    syn::parse2::<syn::Expr>(tokens.clone())
        .unwrap_or_else(|error| panic!("生成物不是合法 Rust 表达式：{error}\n{tokens}"));
    let generated = normalized(tokens);
    assert!(generated.contains(".on_click_fn("));
    assert!(generated.contains("enum__uix_dynamic_style_"));
    assert!(!generated.contains("setStyle"));
}
