// 引入文档与表达式解析入口及 AST 联合。
use super::{
    parse_expression, parser::parse_document, AttributeValue, BinaryOperator, ControlBinding,
    ExpressionKind, Node, SourceSpan, UnaryOperator,
};

// 构造独立表达式测试使用的绝对起点。
fn origin() -> SourceSpan {
    // 返回非零位置以验证跨度映射。
    SourceSpan {
        // 模拟文档内绝对字节起点。
        start: 100,
        // 表达式入口只使用起点定位。
        end: 100,
        // 模拟第十行。
        line: 10,
        // 模拟第五列。
        column: 5,
    }
}

// 验证算术、比较、逻辑和一元运算使用规范优先级。
#[test]
fn parses_operator_precedence_deterministically() {
    // 解析覆盖全部主要优先级的表达式。
    let expression = parse_expression("a + b * 2 > 4 && !done || ready", origin())
        // 测试输入必须成功。
        .expect("优先级表达式应成功解析");
    // 最外层必须是逻辑或。
    let ExpressionKind::Binary {
        // 借用逻辑或左树。
        left,
        // 提取逻辑或运算符。
        operator: BinaryOperator::Or,
        // 借用逻辑或右树。
        right,
    } = &expression.kind
    else {
        // 结构不匹配时失败。
        panic!("最外层应为逻辑或");
    };
    // 右侧必须保留 ready 标识符。
    assert!(matches!(right.kind, ExpressionKind::Identifier(ref value) if value == "ready"));
    // 左侧必须是逻辑与。
    let ExpressionKind::Binary {
        // 借用比较树。
        left: comparison,
        // 要求逻辑与。
        operator: BinaryOperator::And,
        // 借用一元非树。
        right: negation,
    } = &left.kind
    else {
        // 结构不匹配时失败。
        panic!("逻辑或左侧应为逻辑与");
    };
    // 右操作数必须是一元逻辑非。
    assert!(matches!(
        // 检查一元节点。
        negation.kind,
        // 要求逻辑非运算符。
        ExpressionKind::Unary {
            operator: UnaryOperator::Not,
            ..
        }
    ));
    // 比较层必须位于逻辑层内部。
    let ExpressionKind::Binary {
        // 借用加法树。
        left: addition,
        // 要求大于比较。
        operator: BinaryOperator::Greater,
        // 忽略数字右侧。
        ..
    } = &comparison.kind
    else {
        // 结构不匹配时失败。
        panic!("逻辑与左侧应为大于比较");
    };
    // 加法必须包住更高优先级乘法右侧。
    let ExpressionKind::Binary {
        // 要求加法。
        operator: BinaryOperator::Add,
        // 借用加法右侧。
        right: multiplication,
        // 忽略加法左侧。
        ..
    } = &addition.kind
    else {
        // 结构不匹配时失败。
        panic!("比较左侧应为加法");
    };
    // 乘法必须先于加法结合。
    assert!(matches!(
        // 检查乘法节点。
        multiplication.kind,
        // 要求乘法运算符。
        ExpressionKind::Binary {
            operator: BinaryOperator::Multiply,
            ..
        }
    ));
}

// 验证三元条件保持右结合结构。
#[test]
fn parses_ternary_as_right_associative() {
    // 解析嵌套三元表达式。
    let expression = parse_expression("ready ? 'A' : pending ? 'B' : 'C'", origin())
        // 测试输入必须成功。
        .expect("嵌套三元应成功解析");
    // 最外层必须是三元。
    let ExpressionKind::Ternary { else_branch, .. } = &expression.kind else {
        // 结构不匹配时失败。
        panic!("最外层应为三元");
    };
    // 假分支必须包含第二个三元。
    assert!(matches!(else_branch.kind, ExpressionKind::Ternary { .. }));
}

// 验证成员、索引、回调与 setState 命名参数结构。
#[test]
fn parses_paths_indexes_and_allowed_calls() {
    // 解析成员与索引链。
    let path = parse_expression("items[0].name", origin()).expect("索引路径应成功解析");
    // 最外层必须是成员访问。
    assert!(matches!(
        // 检查路径结构。
        path.kind,
        // 要求成员名为 name。
        ExpressionKind::Member { ref member, .. } if member == "name"
    ));
    // 解析 setState 多命名参数。
    let call = parse_expression("setState(count: count + 1, done: true)", origin())
        // 测试输入必须成功。
        .expect("setState 命名参数应成功解析");
    // 提取调用参数。
    let ExpressionKind::Call { arguments, .. } = &call.kind else {
        // 结构不匹配时失败。
        panic!("应生成调用节点");
    };
    // 两个参数必须保持源码顺序。
    assert_eq!(arguments.len(), 2);
    // 第一个名称必须是 count。
    assert_eq!(arguments[0].name.as_deref(), Some("count"));
    // 第二个名称必须是 done。
    assert_eq!(arguments[1].name.as_deref(), Some("done"));
    // 普通成员回调允许位置参数。
    parse_expression("props.onConfirm(items[0])", origin())
        // 回调结构必须成功。
        .expect("成员回调应成功解析");
    // 规范数组不可变更新操作允许成员调用。
    parse_expression("todos.removeAt(i)", origin()).expect("removeAt 应成功解析");
    // push 同样属于允许的数组操作。
    parse_expression("todos.push(item)", origin()).expect("push 应成功解析");
    // 保留事件参数允许成员访问。
    parse_expression("onClick($event.x)", origin()).expect("$event 应成功解析");
    // 其他美元前缀标识符必须失败。
    let error = parse_expression("$other", origin()).expect_err("未知美元标识符必须失败");
    // 诊断必须指出 $event 保留规则。
    assert!(error.message.contains("$event"));
}

// 验证 If、For 与事件属性直接携带已验证表达式 AST。
#[test]
fn parses_if_for_bindings_and_event_calls() {
    // 构造覆盖两类控制元素与事件调用的文档。
    let source = "<App><If {count > 0}><Text>{count}</Text></If><For {item} {index} in {items} key={item.id}><Button @click=\"setState(items: items.removeAt(index))\">{item.name}</Button></For></App>";
    // 解析完整文档。
    let document = parse_document(source).expect("控制绑定文档应成功解析");
    // 提取 If 元素。
    let Node::Element(if_element) = &document.root.children[0] else {
        // 结构不匹配时失败。
        panic!("第一个子节点应为 If");
    };
    // If 控制绑定必须是大于比较。
    assert!(matches!(
        // 借用 If 控制绑定。
        if_element.control,
        // 要求条件表达式为二元大于。
        Some(ControlBinding::If(ref node)) if matches!(node.expression.kind, ExpressionKind::Binary { operator: BinaryOperator::Greater, .. })
    ));
    // 提取 For 元素。
    let Node::Element(for_element) = &document.root.children[1] else {
        // 结构不匹配时失败。
        panic!("第二个子节点应为 For");
    };
    // For 必须保留单标识符绑定和 iterable AST。
    let Some(ControlBinding::For {
        // 借用绑定名。
        binding,
        // 借用数据源。
        iterable,
        // 借用可选索引名。
        index_binding,
        // 借用稳定行身份。
        key,
        // 忽略已单独验证的绑定跨度。
        ..
    }) = &for_element.control
    else {
        // 结构不匹配时失败。
        panic!("For 应包含循环绑定");
    };
    // 绑定名必须是 item。
    assert_eq!(binding, "item");
    // 第二绑定必须保留 index。
    assert_eq!(index_binding.as_deref(), Some("index"));
    // key 必须解析为成员访问表达式。
    assert!(matches!(
        // 借用 key 表达式。
        key,
        // 要求成员访问 AST。
        Some(node) if matches!(node.expression.kind, ExpressionKind::Member { .. })
    ));
    // 数据源必须是 items 标识符。
    assert!(
        matches!(iterable.expression.kind, ExpressionKind::Identifier(ref value) if value == "items")
    );
    // 提取 For 内 Button。
    let Node::Element(button) = &for_element.children[0] else {
        // 结构不匹配时失败。
        panic!("For 子节点应为 Button");
    };
    // 事件属性必须转换为调用表达式而非普通字面量。
    assert!(matches!(
        // 借用事件属性值。
        button.attributes[0].value,
        // 要求已验证调用节点。
        AttributeValue::Expression(ref node) if matches!(node.expression.kind, ExpressionKind::Call { .. })
    ));
}

// 验证规范列出的非法结构返回稳定原因与修复建议。
#[test]
fn rejects_forbidden_expression_structures() {
    // 覆盖闭包、match、数组、对象、类型标注、语句与赋值。
    for (source, expected) in [
        // 闭包必须失败。
        ("|value| value", "闭包"),
        // 零参数闭包必须失败。
        ("|| value", "闭包"),
        // match 必须失败。
        ("match value", "match"),
        // 数组字面量必须失败。
        ("[1, 2]", "数组字面量"),
        // 对象字面量必须失败。
        ("{name: 1}", "对象字面量"),
        // 类型标注必须失败。
        ("value: i32", "类型标注"),
        // 语句必须失败。
        ("value; other", "语句"),
        // 赋值必须失败。
        ("value = 1", "赋值"),
    ] {
        // 解析并取得预期诊断。
        let error = parse_expression(source, origin()).expect_err("非法结构必须失败");
        // 原因必须命中对应结构名称。
        assert!(error.message.contains(expected), "{}", error.message);
        // 每个错误必须携带修复建议。
        assert!(!error.suggestion.is_empty());
    }
}

// 验证内置调用和 For 绑定执行专用形状约束。
#[test]
fn rejects_invalid_call_and_for_shapes() {
    // setState 必须使用命名参数。
    let error = parse_expression("setState(count + 1)", origin())
        // 该调用必须失败。
        .expect_err("setState 位置参数必须失败");
    // 诊断必须指向命名参数规则。
    assert!(error.message.contains("命名参数"));
    // setTheme 必须使用字符串。
    let error = parse_expression("setTheme(theme)", origin())
        // 该调用必须失败。
        .expect_err("setTheme 非字符串参数必须失败");
    // 诊断必须指出字符串要求。
    assert!(error.message.contains("字符串"));
    // 普通回调不能使用 setState 命名参数语法。
    let error = parse_expression("onConfirm(value: 1)", origin())
        // 该调用必须失败。
        .expect_err("普通回调命名参数必须失败");
    // 诊断必须指出命名参数范围。
    assert!(error.message.contains("只允许用于 setState"));
    // For 绑定不能是成员路径。
    let error = parse_document("<For {item.name} in {items} />")
        // 非单标识符绑定必须失败。
        .expect_err("For 复杂绑定必须失败");
    // 诊断必须明确单标识符约束。
    assert!(error.message.contains("单个标识符"));
    // If 不能遗漏条件绑定。
    let error = parse_document("<If />").expect_err("If 缺失条件必须失败");
    // 诊断必须明确控制绑定缺失。
    assert!(error.message.contains("缺少控制绑定"));
    // For 的循环项和索引不能重名。
    let error = parse_document("<For {item} {item} in {items} />")
        // 重复绑定必须失败。
        .expect_err("For 重复绑定必须失败");
    // 诊断必须明确重名约束。
    assert!(error.message.contains("不能同名"));
    // For key 必须使用表达式而非双引号字面量。
    let error = parse_document("<For {item} in {items} key=\"item.id\" />")
        // 字面 key 必须失败。
        .expect_err("For 字面 key 必须失败");
    // 诊断必须明确花括号要求。
    assert!(error.message.contains("花括号表达式"));
    // For key 只能声明一次。
    let error = parse_document("<For {item} in {items} key={item.id} key={item.id} />")
        // 重复 key 必须失败。
        .expect_err("For 重复 key 必须失败");
    // 诊断必须明确重复属性。
    assert!(error.message.contains("重复声明"));
}

// 验证多行表达式跨度使用文档绝对行列。
#[test]
fn preserves_multiline_expression_spans() {
    // 解析首行为空白的多行表达式。
    let expression = parse_expression("\n  items[0]", origin())
        // 测试输入必须成功。
        .expect("多行表达式应成功解析");
    // 首个标记应位于下一行。
    assert_eq!(expression.span.line, 11);
    // 换行后两个空格使标识符位于第三列。
    assert_eq!(expression.span.column, 3);
    // 绝对字节起点必须包含换行和空格。
    assert_eq!(expression.span.start, 103);
}
