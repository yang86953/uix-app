// 引入文档解析入口及声明、样式 AST。
use super::{
    AttributeValue, Declaration, Node, StyleClassResolver, StyleHashKind, StylePseudoState,
    parser::parse_document,
};

// 验证顶层声明保持源码顺序并解析全部本 Gate 结构。
#[test]
fn parses_top_level_declarations_in_source_order() {
    // 构造导入、导出、样式、主题、组件和根元素文档。
    let source = "@import('./card.uix', 'Card')\n@export('LocalCard', 'Panel')\nbaseCard { color: #primaryColor; padding: 8px; }\nfeatureCard { extends: baseCard; borderLeft: 4px solid #2196F3; }\n@theme light { primaryColor: #2196F3; backgroundColor: #fff; }\n<Widget name=\"LocalCard\"><Text>卡片</Text></Widget>\n<App theme=\"light\"><LocalCard /></App>";
    // 解析完整文档。
    let document = parse_document(source).expect("顶层声明文档应成功解析");
    // 六个声明必须全部保留。
    assert_eq!(document.declarations.len(), 6);
    // 第一个声明必须是具名导入。
    assert!(matches!(
        // 借用首个声明。
        &document.declarations[0],
        // 验证路径与组件名。
        Declaration::Import(value) if value.path == "./card.uix" && value.widget.as_deref() == Some("Card")
    ));
    // 第二个声明必须保留导出顺序。
    assert!(matches!(
        // 借用导出声明。
        &document.declarations[1],
        // 验证两个名称顺序。
        Declaration::Export(value) if value.widgets == ["LocalCard", "Panel"]
    ));
    // 第三个声明必须是基础样式类。
    assert!(matches!(
        // 借用基础样式类。
        &document.declarations[2],
        // 验证名称和属性数量。
        Declaration::StyleClass(value) if value.name == "baseCard" && value.properties.len() == 2
    ));
    // 第四个样式类必须保留继承目标。
    assert!(matches!(
        // 借用派生样式类。
        &document.declarations[3],
        // 验证继承名称。
        Declaration::StyleClass(value) if value.extends.as_deref() == Some("baseCard")
    ));
    // 第五个声明必须是 light 主题。
    assert!(matches!(
        // 借用主题声明。
        &document.declarations[4],
        // 验证主题名。
        Declaration::Theme(value) if value.name == "light"
    ));
    // 第六个声明必须是完成声明级验证的 Widget。
    assert!(matches!(
        // 借用组件声明。
        &document.declarations[5],
        // 验证组件声明名。
        Declaration::Widget(value) if value.name == "LocalCard"
    ));
    // 根元素必须在声明之后独立保存。
    assert_eq!(document.root.name, "App");
}

// 验证关键帧声明解析、偏移规范化与排序。
#[test]
fn parses_keyframes_with_named_and_percentage_offsets() {
    // 构造乱序百分比与具名端点声明。
    let document = parse_document(
        "@keyframes fade { to { opacity: 1; } 40% { opacity: 0.4; } from { opacity: 0; } } <Text />",
    )
    // 合法关键帧必须成功解析。
    .expect("关键帧声明应成功解析");
    // 提取关键帧声明。
    let Declaration::Keyframes(keyframes) = &document.declarations[0] else {
        // 结构不匹配时失败。
        panic!("首个声明应为关键帧");
    };
    // 名称必须保留。
    assert_eq!(keyframes.name, "fade");
    // 帧必须按规范化偏移排序。
    assert_eq!(
        // 收集排序后的偏移。
        keyframes
            // 遍历帧。
            .frames
            // 借用迭代器。
            .iter()
            // 提取百万分比偏移。
            .map(|frame| frame.offset_millionths)
            // 收集为向量。
            .collect::<Vec<_>>(),
        // 对比预期顺序。
        vec![0, 400_000, 1_000_000]
    );
}

// 验证相同关键帧名称不能重复声明。
#[test]
fn rejects_duplicate_keyframe_names() {
    // 解析两个同名声明。
    let error = parse_document(
        "@keyframes fade { from { opacity: 0; } } @keyframes fade { to { opacity: 1; } } <Text />",
    )
    // 重复名称必须失败。
    .expect_err("重复关键帧名称必须失败");
    // 诊断必须指出动画名冲突。
    assert!(error.message.contains("关键帧 fade 重复声明"));
}

// 验证越界关键帧偏移在解析期失败。
#[test]
fn rejects_out_of_range_keyframe_offset() {
    // 解析超出闭区间的偏移。
    let error = parse_document("@keyframes fade { 120% { opacity: 1; } } <Text />")
        // 越界偏移必须失败。
        .expect_err("越界关键帧偏移必须失败");
    // 诊断必须保留原始选择器。
    assert!(error.message.contains("120%"));
}

// 验证空关键帧块不能形成伪支持声明。
#[test]
fn rejects_empty_keyframe_block() {
    // 解析没有属性的单帧。
    let error = parse_document("@keyframes fade { from { } } <Text />")
        // 空帧必须失败。
        .expect_err("空关键帧必须失败");
    // 诊断必须指出缺少动画值。
    assert!(error.message.contains("不能为空"));
}

// 验证十六进制颜色和主题属性引用具有不同 AST。
#[test]
fn distinguishes_hex_colors_from_theme_references() {
    // 构造同时包含颜色和主题引用的样式类。
    let document = parse_document(
        "sample { color: #ccc; borderColor: #FF5722; backgroundColor: #primaryColor; } <App />",
    )
    // 样式文档必须成功。
    .expect("哈希样式值应成功解析");
    // 提取样式类。
    let Declaration::StyleClass(style) = &document.declarations[0] else {
        // 结构不匹配时失败。
        panic!("首个声明应为样式类");
    };
    // 三位十六进制必须识别为颜色。
    assert!(matches!(
        // 检查第一个哈希片段。
        style.properties[0].value.hashes[0].kind,
        // 要求颜色值 ccc。
        StyleHashKind::HexColor(ref value) if value == "ccc"
    ));
    // 六位十六进制必须识别为颜色。
    assert!(matches!(
        // 检查第二个哈希片段。
        style.properties[1].value.hashes[0].kind,
        // 要求颜色值 FF5722。
        StyleHashKind::HexColor(ref value) if value == "FF5722"
    ));
    // 非纯十六进制标识符必须识别为主题引用。
    assert!(matches!(
        // 检查第三个哈希片段。
        style.properties[2].value.hashes[0].kind,
        // 要求主题属性 primaryColor。
        StyleHashKind::ThemeReference(ref value) if value == "primaryColor"
    ));
}

// 验证内联样式复用样式块语法及状态属性。
#[test]
fn parses_inline_style_with_shared_grammar() {
    // 解析包含连字符、状态后缀和函数值的内联样式。
    let document = parse_document(
        "<Text style=\"z-index: 2; backgroundColor:hover: rgba(0,0,0,0.1); color: #primaryColor;\" />",
    )
    // 内联样式必须成功。
    .expect("内联样式应成功解析");
    // 提取 style 属性。
    let AttributeValue::InlineStyle(properties) = &document.root.attributes[0].value else {
        // 结构不匹配时失败。
        panic!("style 应生成结构化内联属性");
    };
    // 三个属性必须保持顺序。
    assert_eq!(properties.len(), 3);
    // 连字符属性名必须完整保留。
    assert_eq!(properties[0].name, "z-index");
    // 状态后缀必须并入属性名。
    assert_eq!(properties[1].name, "backgroundColor:hover");
    // 函数值必须保持未改写源码。
    assert_eq!(properties[1].value.source, "rgba(0,0,0,0.1)");
    // 主题引用必须保留语义分类。
    assert!(matches!(
        // 检查内联主题引用。
        properties[2].value.hashes[0].kind,
        // 要求 primaryColor 引用。
        StyleHashKind::ThemeReference(ref value) if value == "primaryColor"
    ));
}

// 验证重复声明和重复属性返回带位置诊断。
#[test]
fn rejects_duplicate_names_and_properties() {
    // 覆盖重复样式类和重复主题。
    for (source, expected) in [
        // 重复样式类必须失败。
        (
            "same { color: red; } same { color: blue; } <App />",
            "样式类 same 重复",
        ),
        // 重复主题必须失败。
        (
            "@theme light { color: red; } @theme light { color: blue; } <App />",
            "主题 light 重复",
        ),
        // 同块重复属性必须失败。
        (
            "sample { color: red; color: blue; } <App />",
            "属性 color 重复",
        ),
    ] {
        // 解析并取得预期错误。
        let error = parse_document(source).expect_err("重复名称必须失败");
        // 诊断原因必须明确重复对象。
        assert!(error.message.contains(expected), "{}", error.message);
        // 诊断必须携带非零或零宽确定跨度。
        assert!(error.span.end >= error.span.start);
        // 诊断必须提供修复建议。
        assert!(!error.suggestion.is_empty());
    }
}

// 验证声明区边界不允许声明嵌套或后置。
#[test]
fn rejects_declarations_outside_top_level_zone() {
    // Widget 嵌套在根元素中必须失败。
    let error = parse_document("<App><Widget name=\"Nested\" /></App>")
        // 非法嵌套必须失败。
        .expect_err("嵌套 Widget 必须失败");
    // 诊断必须指出顶层声明区。
    assert!(error.message.contains("顶层声明区"));
    // 主题指令嵌套在根元素中必须失败。
    let error = parse_document("<App>@theme dark { color: white; }</App>")
        // 非法嵌套必须失败。
        .expect_err("嵌套主题必须失败");
    // 诊断必须指出顶层声明区。
    assert!(error.message.contains("顶层声明区"));
    // 根元素后的主题声明必须失败。
    let error = parse_document("<App /> @theme dark { color: white; }")
        // 后置声明必须失败。
        .expect_err("根后主题必须失败");
    // 诊断必须指出声明顺序。
    assert!(error.message.contains("根元素之前"));
    // 根元素后的样式类同样必须失败。
    let error = parse_document("<App /> lateStyle { color: red; }")
        // 后置样式必须失败。
        .expect_err("根后样式必须失败");
    // 诊断必须指出声明顺序。
    assert!(error.message.contains("根元素之前"));
}

// 验证非法样式值、引用与继承位置返回确定诊断。
#[test]
fn rejects_invalid_style_values_and_references() {
    // 覆盖缺失分号、括号、颜色位数和内联继承。
    for (source, expected) in [
        // 缺少分号必须失败。
        ("sample { color: red } <App />", "分号"),
        // 函数缺少右括号必须失败。
        ("sample { color: rgba(0,0,0,0.1; } <App />", "函数调用缺少"),
        // 非标准十六进制位数必须失败。
        ("sample { color: #12; } <App />", "十六进制颜色"),
        // 井号后非法名称必须失败。
        ("sample { color: #1g; } <App />", "既不是十六进制"),
        // 内联样式不得声明继承。
        ("<Text style=\"extends: baseText;\" />", "extends 只允许"),
    ] {
        // 解析并取得预期诊断。
        let error = parse_document(source).expect_err("非法样式必须失败");
        // 诊断必须命中对应规则。
        assert!(error.message.contains(expected), "{}", error.message);
        // 修复建议不能为空。
        assert!(!error.suggestion.is_empty());
    }
}

// 验证导入导出参数规则。
#[test]
fn validates_import_and_export_arguments() {
    // 非 .uix 路径必须失败。
    let error = parse_document("@import('./card.rs') <App />")
        // 错误路径必须失败。
        .expect_err("非 uix 导入必须失败");
    // 诊断必须指出后缀要求。
    assert!(error.message.contains(".uix"));
    // 导出名称必须使用 PascalCase。
    let error = parse_document("@export('card') <App />")
        // 小写组件名必须失败。
        .expect_err("小写导出名必须失败");
    // 诊断必须指出 PascalCase。
    assert!(error.message.contains("PascalCase"));
    // 重复导出名必须失败。
    let error = parse_document("@export('Card', 'Card') <App />")
        // 重复导出必须失败。
        .expect_err("重复导出必须失败");
    // 诊断必须指出重复。
    assert!(error.message.contains("重复"));
}

// 验证 UTF-8 声明后的样式诊断保持文档绝对位置。
#[test]
fn preserves_utf8_style_diagnostic_position() {
    // 中文注释位于错误样式之前。
    let source = "// 中文说明\ninvalid {\n  color: #12;\n}\n<App />";
    // 解析并取得颜色诊断。
    let error = parse_document(source).expect_err("非法颜色必须失败");
    // 错误应位于第三行。
    assert_eq!(error.span.line, 3);
    // 井号位于两个空格和 color 冒号空格后的第十列。
    assert_eq!(error.span.column, 10);
}

// 验证 Widget 声明中的普通子元素仍保持顺序。
#[test]
fn preserves_widget_children_after_declaration_validation() {
    // 解析顶层组件定义和根元素。
    let document = parse_document(
        "<Widget name=\"Card\"><Container><Text>内容</Text></Container></Widget><App><Card /></App>",
    )
    // 组件结构必须成功。
    .expect("顶层 Widget 应完成声明级验证");
    // 提取组件声明。
    let Declaration::Widget(widget) = &document.declarations[0] else {
        // 结构不匹配时失败。
        panic!("首个声明应为 Widget");
    };
    // 组件必须保留一个 Container 子节点。
    assert!(matches!(
        // 借用首个子节点。
        &widget.children[0],
        // 验证标签名。
        Node::Element(value) if value.name == "Container"
    ));
}

// 验证三个状态伪类使用独立登记键并保留差异属性。
#[test]
fn parses_registered_style_pseudo_states() {
    // 声明基础类及全部批准状态。
    let document = parse_document(
        "baseButton { padding: 8px; } baseButton:hover { color: blue; } baseButton:disabled { opacity: 0.5; } baseButton:checked { borderColor: red; } <App />",
    )
    .expect("三个批准状态应完成解析");
    // 四个样式声明必须全部保留。
    assert_eq!(document.declarations.len(), 4);
    // 状态声明应按源码顺序映射为闭合枚举。
    for (index, expected) in [
        StylePseudoState::Hover,
        StylePseudoState::Disabled,
        StylePseudoState::Checked,
    ]
    .into_iter()
    .enumerate()
    {
        // 取得对应状态声明。
        let Declaration::StyleClass(style) = &document.declarations[index + 1] else {
            // 非样式声明表示解析结构损坏。
            panic!("状态声明应保持为样式类");
        };
        // 基础前缀与状态必须分别保存。
        assert_eq!(style.name, "baseButton");
        // 状态必须精确匹配白名单枚举。
        assert_eq!(style.state, Some(expected));
    }
}

// 验证状态伪类拒绝未知状态、显式继承、重复项和缺失基础类。
#[test]
fn rejects_invalid_style_pseudo_state_contracts() {
    // 解析期拒绝三类非法伪类声明。
    for (source, expected) in [
        (
            "base { color: red; } base:visited { color: blue; } <App />",
            "不支持样式伪类",
        ),
        (
            "base { color: red; } base:hover { extends: base; color: blue; } <App />",
            "不能显式声明 extends",
        ),
        (
            "base { color: red; } base:hover { color: blue; } base:hover { color: green; } <App />",
            "重复声明",
        ),
    ] {
        // 非法声明必须返回定向诊断。
        let error = parse_document(source).expect_err("非法伪类声明必须失败");
        // 诊断必须命中对应规则。
        assert!(error.message.contains(expected), "{}", error.message);
    }
    // 缺失基础类在样式注册阶段失败。
    let document = parse_document("missing:hover { color: blue; } <App />")
        .expect("缺失基础类不影响声明结构解析");
    // 注册表必须拒绝无法隐含继承的状态变体。
    let error = match StyleClassResolver::new(&document) {
        // 成功表示错误接受了孤立状态变体。
        Ok(_) => panic!("状态变体必须有基础类"),
        // 保存预期诊断。
        Err(error) => error,
    };
    // 诊断必须说明基础类缺失。
    assert!(error.message.contains("缺少同前缀基础类"));
}
