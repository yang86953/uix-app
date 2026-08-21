// 引入文档与表达式解析入口及 AST 联合。
use super::{
    AttributeValue, BinaryOperator, ControlBinding, ExpressionKind, Node, SourceSpan,
    UnaryOperator, generate_expression, parse_expression, parser::parse_document,
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

// 验证全部扩展数组操作与单参数闭包形成确定 AST。
#[test]
fn parses_extended_array_operations_and_restricted_closures() {
    // 两参数不可变更新操作必须成功解析。
    parse_expression("items.insertAt(index, value)", origin()).expect("insertAt 应成功解析");
    // 替换操作同样接受索引和值。
    parse_expression("items.updateAt(index, value)", origin()).expect("updateAt 应成功解析");
    // 逐一验证开放闭包的数组操作集合。
    for source in [
        // 删除首个谓词匹配项。
        "items.removeBy(|it| it.id == target)",
        // 过滤并允许捕获外层阈值。
        "items.filter(|it| it.score >= threshold)",
        // 映射为字段值。
        "items.map(|it| it.name)",
        // 按字段稳定排序。
        "items.sortBy(|it| it.order)",
        // 查找首个谓词匹配项。
        "items.find(|it| it.id == target)",
    ] {
        // 每个规范操作都必须通过解析与位置验证。
        parse_expression(source, origin()).unwrap_or_else(|error| panic!("{source}: {error:?}"));
    }
    // 提取 filter 的闭包 AST 验证参数与表达式体。
    let filter = parse_expression("items.filter(|entry| entry.done)", origin())
        // 规范过滤表达式必须成功。
        .expect("filter 闭包应成功解析");
    // 顶层必须是成员调用。
    let ExpressionKind::Call { arguments, .. } = &filter.kind else {
        // 结构不符时立即失败。
        panic!("filter 应生成调用节点");
    };
    // 唯一参数必须保留闭包参数名称。
    assert!(matches!(
        // 检查闭包结构。
        &arguments[0].value.kind,
        // 参数名必须与源码一致。
        ExpressionKind::Closure { parameter, .. } if parameter == "entry"
    ));
}

// 验证闭包位置、参数形状与数组操作参数数量在解析期关闭。
#[test]
fn rejects_invalid_array_closure_positions_and_shapes() {
    // 独立闭包不属于通用表达式位置。
    let standalone = parse_expression("|it| it.done", origin())
        // 必须返回闭包位置诊断。
        .expect_err("独立闭包必须失败");
    // 诊断必须指向数组操作参数边界。
    assert!(standalone.message.contains("只能用于数组操作"));
    // 普通回调不能接收语言闭包。
    let callback = parse_expression("consume(|it| it.done)", origin())
        // 普通调用闭包必须失败。
        .expect_err("普通回调闭包必须失败");
    // 诊断应复用同一闭包位置契约。
    assert!(callback.message.contains("只能用于数组操作"));
    // filter 必须直接接收闭包而不能接收回调标识符。
    let filter = parse_expression("items.filter(predicate)", origin())
        // 非闭包谓词必须失败。
        .expect_err("filter 回调标识符必须失败");
    // 诊断必须说明受限闭包形状。
    assert!(filter.message.contains("受限闭包"));
    // insertAt 缺少值参数必须失败。
    let insert = parse_expression("items.insertAt(index)", origin())
        // 参数数量不符必须失败。
        .expect_err("insertAt 单参数必须失败");
    // 诊断必须说明索引和值两个参数。
    assert!(insert.message.contains("索引和值"));
    // 多参数闭包语法在第二个参数位置得到闭合竖线诊断。
    let multiple = parse_expression("items.map(|left, right| left + right)", origin())
        // 多参数闭包必须失败。
        .expect_err("多参数闭包必须失败");
    // 诊断必须明确单参数定界形状。
    assert!(multiple.message.contains("缺少 |"));
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
    // 覆盖闭包、match、类型标注、语句与赋值。
    for (source, expected) in [
        // 闭包必须失败。
        ("|value| value", "闭包"),
        // 零参数闭包必须失败。
        ("|| value", "闭包"),
        // match 必须失败。
        ("match value", "match"),
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

// 验证数组字面量保存元素顺序与拒绝尾随逗号。
#[test]
fn parses_array_literals_with_ordered_elements() {
    // 解析包含嵌套调用与字符串的数组。
    let expression = parse_expression("[SelectOption('中国', 'cn'), 'b', 3]", origin())
        // 合法数组必须成功。
        .expect("数组字面量应成功解析");
    // 提取数组元素序列。
    let ExpressionKind::Array(items) = &expression.kind else {
        // 非数组结构立即失败。
        panic!("应生成数组节点");
    };
    // 元素必须保持源码顺序与形状。
    assert_eq!(items.len(), 3);
    // 首元素必须是数据构造调用。
    assert!(matches!(items[0].kind, ExpressionKind::Call { .. }));
    // 第二元素必须是字符串字面量。
    assert!(matches!(items[1].kind, ExpressionKind::String(_)));
    // 第三元素必须是数字字面量。
    assert!(matches!(items[2].kind, ExpressionKind::Number(_)));
    // 尾随逗号必须给出专用诊断。
    let trailing = parse_expression("[1, 2,]", origin()).expect_err("尾随逗号必须失败");
    // 原因必须指向尾随逗号。
    assert!(
        trailing.message.contains("尾随逗号"),
        "{}",
        trailing.message
    );
    // 空数组保持合法。
    let empty = parse_expression("[]", origin()).expect("空数组应成功解析");
    // 空数组没有元素。
    assert!(matches!(empty.kind, ExpressionKind::Array(items) if items.is_empty()));
}

// 验证受限对象字面量保存字段顺序、值结构与 UTF-8 位置。
#[test]
fn parses_ordered_object_fields_for_registered_attributes() {
    // 解析包含中文字符串和动态字段值的单层对象。
    let expression = parse_expression("{ count: total + 1, dot: false, label: '新' }", origin())
        // 合法对象必须成功。
        .expect("受限对象应成功解析");
    // 提取对象字段序列。
    let ExpressionKind::Object(fields) = &expression.kind else {
        // 非对象结构立即失败。
        panic!("应生成对象节点");
    };
    // 字段必须保持源码顺序。
    assert_eq!(
        // 收集字段名称。
        fields
            // 遍历字段借用。
            .iter()
            // 借用字段名。
            .map(|field| field.name.as_str())
            // 收集成稳定序列。
            .collect::<Vec<_>>(),
        // 对照源顺序。
        vec!["count", "dot", "label"]
    );
    // 首字段值必须复用现有二元表达式 AST。
    assert!(matches!(
        fields[0].value.kind,
        ExpressionKind::Binary { .. }
    ));
    // 中文字符串前的字段跨度必须保持绝对 UTF-8 字节位置。
    assert_eq!(fields[2].span.start, 132);
    // 对象起点沿用外部表达式绝对位置。
    assert_eq!(expression.span.start, 100);
}

// 验证双花括号属性扫描完整保留内部对象。
#[test]
fn parses_object_attribute_with_nested_brace_scanning() {
    // 使用文档要求的 badge 双花括号写法。
    let document = parse_document("<FloatButton badge={{ count: 7, dot: false }} />")
        // 外层属性扫描必须成功。
        .expect("对象属性应成功解析");
    // 提取根元素属性。
    let attribute = &document.root.attributes[0];
    // 属性表达式必须保存对象 AST。
    assert!(matches!(
        // 检查属性值形状。
        attribute.value,
        // 要求对象表达式。
        AttributeValue::Expression(ref node)
            if matches!(node.expression.kind, ExpressionKind::Object(ref fields) if fields.len() == 2)
    ));
}

// 验证对象字面量拒绝歧义结构并报告精确原因。
#[test]
fn rejects_invalid_object_literal_shapes() {
    // 覆盖重复、分隔符、非法键、方法和赋值结构。
    for (source, expected) in [
        // 重复字段必须失败。
        ("{ count: 1, count: 2 }", "重复声明"),
        // 缺少冒号必须失败。
        ("{ count 1 }", "缺少 :"),
        // 缺少逗号必须失败。
        ("{ count: 1 dot: false }", "缺少逗号"),
        // 缺少右花括号必须失败。
        ("{ count: 1", "缺少 }"),
        // 计算键必须失败。
        ("{ [name]: 1 }", "键必须是标识符"),
        // spread 必须失败。
        ("{ ...value }", "键必须是标识符"),
        // 方法结构必须失败。
        ("{ count() }", "缺少 :"),
        // 赋值结构必须失败。
        ("{ count = 1 }", "赋值"),
    ] {
        // 解析并取得预期诊断。
        let error = parse_expression(source, origin()).expect_err("非法对象必须失败");
        // 原因必须命中对应结构名称。
        assert!(error.message.contains(expected), "{}", error.message);
        // 每个错误必须携带修复建议。
        assert!(!error.suggestion.is_empty());
    }
}

// 验证普通表达式生成路径不会猜测对象目标类型。
#[test]
fn rejects_object_in_generic_expression_codegen() {
    // 解析一个合法的单层对象。
    let expression = parse_expression("{ count: 7 }", origin()).expect("对象应成功解析");
    // 普通生成器必须返回结构属性边界诊断。
    let error = generate_expression(&expression, None).expect_err("普通生成路径必须拒绝对象");
    // 诊断必须指向已登记结构属性。
    assert!(error.message.contains("已登记的结构属性"));
}

// 验证 setTheme 生成框架主题请求通道调用而非调用方同名函数。
#[test]
fn generates_set_theme_builtin_call() {
    // 解析 setTheme 内置操作。
    let expression = parse_expression("setTheme('dark')", origin())
        // 合法调用必须成功。
        .expect("setTheme 应成功解析");
    // 生成确定令牌流。
    let tokens = generate_expression(&expression, None)
        // 生成必须成功。
        .expect("setTheme 生成应成功")
        // 转为快照文本。
        .to_string();
    // 快照必须指向框架主题请求通道。
    assert!(
        tokens.contains("uix_set_theme") && tokens.contains("theme"),
        "{tokens}"
    );
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
