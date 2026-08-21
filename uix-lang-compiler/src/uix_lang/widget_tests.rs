// 引入组件 AST 与文档解析入口。
use super::{
    Declaration, ExpressionKind, WidgetPropType, WidgetStateInitial, WidgetValueType,
    parse_document,
};

// 验证 computed 按源码顺序保存名称与受限表达式。
#[test]
fn parses_ordered_widget_computed_contract() {
    // 解析依赖 state 与先前 computed 的两个派生值。
    let document = parse_document(
        // 后项 remaining 合法引用前项 doubled。
        r#"<Widget name="Summary" state="count: 1" computed="doubled: count + count, remaining: doubled + 1"><Text>{remaining}</Text></Widget><Summary />"#,
    )
    // 合法有序派生声明必须解析成功。
    .expect("computed 声明契约应成功解析");
    // 提取组件声明。
    let Declaration::Widget(widget) = &document.declarations[0] else {
        // 结构不匹配时失败。
        panic!("首个声明应为 Widget");
    };
    // 两个派生值必须保持源码顺序。
    assert_eq!(widget.computed.len(), 2);
    // 首个派生名称必须保持不变。
    assert_eq!(widget.computed[0].name, "doubled");
    // 后续派生名称必须保持不变。
    assert_eq!(widget.computed[1].name, "remaining");
    // 派生值必须复用受限表达式 AST。
    assert!(matches!(
        // 检查首个派生表达式形状。
        widget.computed[0].expression.kind,
        // 加法表达式必须解析为二元节点。
        ExpressionKind::Binary { .. }
    ));
}

// 验证 computed 名称在自身与组件字段命名空间中唯一。
#[test]
fn rejects_duplicate_or_colliding_widget_computed_names() {
    // 解析重复派生名称。
    let duplicate = parse_document(
        // 同一 computed 属性两次声明 total。
        r#"<Widget name="Bad" computed="total: 1, total: 2"><Text>A</Text></Widget><Bad />"#,
    )
    // 重复派生名称必须失败。
    .expect_err("重复 computed 名称不得通过");
    // 诊断必须包含具体重复名称。
    assert!(duplicate.message.contains("computed total 重复声明"));
    // 解析与私有 state 重名的派生值。
    let collision = parse_document(
        // count 同时声明为 state 与 computed。
        r#"<Widget name="Bad" state="count: 1" computed="count: 2"><Text>A</Text></Widget><Bad />"#,
    )
    // 跨类别名称冲突必须失败。
    .expect_err("computed/state 重名不得通过");
    // 诊断必须说明 computed 名称冲突。
    assert!(collision.message.contains("与 computed 派生值同名"));
}

// 验证 external 白名单按源码顺序进入组件声明。
#[test]
fn parses_widget_external_contract() {
    // 解析两个显式 Rust 外部依赖。
    let document = parse_document(
        // 使用逗号与空白覆盖规范化行为。
        r#"<Widget name="SearchBox" external="debounce, format"><Text>{format('x')}</Text></Widget><SearchBox />"#,
    )
    // 合法 external 声明必须解析成功。
    .expect("组件 external 契约应成功解析");
    // 提取组件声明。
    let Declaration::Widget(widget) = &document.declarations[0] else {
        // 结构不匹配时失败。
        panic!("首个声明应为 Widget");
    };
    // external 名称必须保持源码顺序并去除周围空白。
    assert_eq!(widget.external, ["debounce", "format"]);
}

// 验证 external 只接受唯一的普通 Rust 标识符。
#[test]
fn rejects_invalid_or_duplicate_widget_external_names() {
    // 解析包含成员路径的非法 external 名称。
    let invalid = parse_document(
        // external 只声明根标识符而不是 Rust 路径。
        r#"<Widget name="Bad" external="service::format"><Text>A</Text></Widget><Bad />"#,
    )
    // 非标识符名称必须在声明阶段失败。
    .expect_err("非法 external 名称不得通过");
    // 诊断必须说明 Rust 标识符约束。
    assert!(invalid.message.contains("不是合法 Rust 标识符"));
    // 解析重复 external 名称。
    let duplicate = parse_document(
        // 相同名称出现两次。
        r#"<Widget name="Bad" external="format, format"><Text>A</Text></Widget><Bad />"#,
    )
    // 重复名称必须在声明阶段失败。
    .expect_err("重复 external 名称不得通过");
    // 诊断必须包含具体重复名称。
    assert!(duplicate.message.contains("external 符号 format 重复声明"));
}

// 验证四类 props 与私有 state 按源码顺序结构化解析。
#[test]
fn parses_widget_props_and_state_contract() {
    // 构造基础值、共享 State、回调与私有状态声明。
    let source = r#"<Widget name="Editor" props="title: String, count: number, enabled: bool, shared: State<number>, onSave: (String) -> bool" state="draft: 'new', retries: 0, dirty: false, items: []"><Text>{title}</Text></Widget><Editor title="A" count={1} enabled={true} shared={shared} onSave={save} />"#;
    // 解析完整文档。
    let document = parse_document(source).expect("组件声明契约应成功解析");
    // 提取组件声明。
    let Declaration::Widget(widget) = &document.declarations[0] else {
        // 结构不匹配时失败。
        panic!("首个声明应为 Widget");
    };
    // 组件名必须保存。
    assert_eq!(widget.name, "Editor");
    // 五个 props 必须全部保留。
    assert_eq!(widget.props.len(), 5);
    // String 映射为基础值 prop。
    assert_eq!(
        // 检查首个 prop 类型。
        widget.props[0].kind,
        // 要求 String 基础值。
        WidgetPropType::Value(WidgetValueType::String)
    );
    // number 映射为基础值 prop。
    assert_eq!(
        // 检查第二个 prop 类型。
        widget.props[1].kind,
        // 要求 number 基础值。
        WidgetPropType::Value(WidgetValueType::Number)
    );
    // bool 映射为基础值 prop。
    assert_eq!(
        // 检查第三个 prop 类型。
        widget.props[2].kind,
        // 要求 bool 基础值。
        WidgetPropType::Value(WidgetValueType::Bool)
    );
    // State<number> 映射为共享状态 prop。
    assert_eq!(
        // 检查第四个 prop 类型。
        widget.props[3].kind,
        // 要求 number 状态引用。
        WidgetPropType::State(WidgetValueType::Number)
    );
    // 回调必须保存参数与返回类型。
    assert!(matches!(
        // 借用第五个 prop 类型。
        &widget.props[4].kind,
        // 验证 String 参数和 bool 返回值。
        WidgetPropType::Callback { parameters, returns }
            if parameters == &[WidgetValueType::String]
                && *returns == Some(WidgetValueType::Bool)
    ));
    // 四个私有状态必须保留顺序。
    assert_eq!(widget.states.len(), 4);
    // 最后一个状态必须识别为空数组初始值。
    assert_eq!(
        // 检查空数组状态。
        widget.states[3].initial,
        // 要求专用空数组事实。
        WidgetStateInitial::EmptyArray
    );
}

// 验证非法 prop 类型得到声明级诊断。
#[test]
fn rejects_unsupported_prop_type() {
    // 解析超出白名单的类型。
    let error = parse_document(
        r#"<Widget name="Bad" props="value: Vec<String>"><Text>A</Text></Widget><Bad value={value} />"#,
    )
    // 未知类型必须失败。
    .expect_err("未知 prop 类型不得通过");
    // 诊断必须指出类型不支持。
    assert!(error.message.contains("不支持 prop 类型"));
}

// 验证 props 内重复字段得到确定诊断。
#[test]
fn rejects_duplicate_prop_name() {
    // 解析重复 prop。
    let error = parse_document(
        r#"<Widget name="Bad" props="value: String, value: bool"><Text>A</Text></Widget><Bad value="A" />"#,
    )
    // 重复字段必须失败。
    .expect_err("重复 prop 不得通过");
    // 诊断必须包含重复名称。
    assert!(error.message.contains("prop value 重复声明"));
}

// 验证 state 内重复字段得到确定诊断。
#[test]
fn rejects_duplicate_state_name() {
    // 解析重复 state。
    let error = parse_document(
        r#"<Widget name="Bad" state="count: 0, count: 1"><Text>A</Text></Widget><Bad />"#,
    )
    // 重复状态必须失败。
    .expect_err("重复 state 不得通过");
    // 诊断必须包含重复名称。
    assert!(error.message.contains("state count 重复声明"));
}

// 验证 prop 与 state 不能共用组件体名称。
#[test]
fn rejects_prop_state_name_collision() {
    // 解析跨类别重名字段。
    let error = parse_document(
        r#"<Widget name="Bad" props="count: number" state="count: 0"><Text>A</Text></Widget><Bad count={1} />"#,
    )
    // 重名字段必须失败。
    .expect_err("prop/state 重名不得通过");
    // 诊断必须说明两个类别。
    assert!(error.message.contains("同时声明为 prop 与 state"));
}

// 验证顶层组件名称唯一。
#[test]
fn rejects_duplicate_widget_name() {
    // 解析两个同名组件。
    let error = parse_document(
        r#"<Widget name="Card"><Text>A</Text></Widget><Widget name="Card"><Text>B</Text></Widget><Card />"#,
    )
    // 重复组件必须失败。
    .expect_err("重复组件名不得通过");
    // 诊断必须包含组件名。
    assert!(error.message.contains("组件 Card 重复声明"));
}

// 验证类型化私有 state 注解解析为 TypedExpression。
#[test]
fn parses_typed_private_state_annotations() {
    // 构造带 u32 与 usize 类型注解的状态声明。
    let source = r#"<Widget name="Typed" state="rating: u32 = 7, current: usize = 1, offset: f32 = 20, signed: i32 = -3, labeled: String = 'hi', selected: Option<String> = None"><Text>A</Text></Widget><Typed />"#;
    // 解析完整文档。
    let document = parse_document(source).expect("类型化 state 声明应成功解析");
    // 提取组件声明。
    let Declaration::Widget(widget) = &document.declarations[0] else {
        // 结构不匹配时失败。
        panic!("首个声明应为 Widget");
    };
    // 六个类型化状态必须全部保留。
    assert_eq!(widget.states.len(), 6);
    // u32 注解必须保存。
    assert!(matches!(
        // 检查首个状态初始值。
        &widget.states[0].initial,
        // 要求 u32 类型化表达式。
        WidgetStateInitial::TypedExpression(WidgetValueType::U32, _)
    ));
    // usize 注解必须保存。
    assert!(matches!(
        // 检查第二个状态初始值。
        &widget.states[1].initial,
        // 要求 usize 类型化表达式。
        WidgetStateInitial::TypedExpression(WidgetValueType::USize, _)
    ));
    // f32 注解必须保存。
    assert!(matches!(
        // 检查第三个状态初始值。
        &widget.states[2].initial,
        // 要求 f32 类型化表达式。
        WidgetStateInitial::TypedExpression(WidgetValueType::F32, _)
    ));
    // i32 注解与负数初始值必须保存。
    assert!(matches!(
        // 检查第四个状态初始值。
        &widget.states[3].initial,
        // 要求 i32 类型化表达式。
        WidgetStateInitial::TypedExpression(WidgetValueType::I32, _)
    ));
    // String 注解必须保存。
    assert!(matches!(
        // 检查第五个状态初始值。
        &widget.states[4].initial,
        // 要求 String 类型化表达式。
        WidgetStateInitial::TypedExpression(WidgetValueType::String, _)
    ));
    // Option<String> 注解必须保存。
    assert!(matches!(
        // 检查第六个状态初始值。
        &widget.states[5].initial,
        // 要求可空字符串类型化表达式。
        WidgetStateInitial::TypedExpression(WidgetValueType::OptionalString, _)
    ));
}

// 验证未知 state 类型注解得到专用诊断。
#[test]
fn rejects_unknown_typed_state_annotation() {
    // 解析超出白名单的非 PascalCase 类型注解。
    let error = parse_document(
        r#"<Widget name="Bad" state="when: dateTime = 1"><Text>A</Text></Widget><Bad />"#,
    )
    // 未知类型必须失败。
    .expect_err("未知 state 类型不得通过");
    // 诊断必须指出类型不支持。
    assert!(error.message.contains("不支持 state 类型"));
    // 诊断必须给出白名单。
    assert!(error.suggestion.contains("u32"));
    // PascalCase 未知类型按 record 引用处理，后验校验必须拒绝未声明 record。
    let error = parse_document(
        r#"<Widget name="Bad" state="when: DateTime = 1"><Text>A</Text></Widget><Bad />"#,
    )
    // 未声明 record 必须失败。
    .expect_err("未声明 record 引用不得通过");
    // 诊断必须指出 record 未声明。
    assert!(error.message.contains("未在当前文档声明"));
}

// 验证比较运算符不会误识别为类型注解分隔符。
#[test]
fn keeps_comparison_expressions_as_plain_state_initials() {
    // 构造包含相等比较的普通状态初始值。
    let document = parse_document(
        r#"<Widget name="Eq" state="done: count == 0"><Text>A</Text></Widget><Eq />"#,
    )
    // 比较表达式必须是普通表达式状态。
    .expect("比较表达式状态应成功解析");
    // 提取组件声明。
    let Declaration::Widget(widget) = &document.declarations[0] else {
        // 结构不匹配时失败。
        panic!("首个声明应为 Widget");
    };
    // 状态必须保持普通表达式而非类型化注解。
    assert!(matches!(
        // 检查状态初始值。
        widget.states[0].initial,
        // 要求普通表达式。
        WidgetStateInitial::Expression(_)
    ));
}
