// 元素、属性与文本解析器拆分到独立子模块。
mod control_binding;
// 控制绑定解析器拆分到独立子模块。
mod element_parser;
// 引入核心 AST、词法游标和诊断。
use super::{
    Cursor, Declaration, Diagnostic, Document, SourceSpan, WidgetStateInitial, WidgetValueType,
    parse_at_declaration, parse_record_declaration, parse_style_class, parse_visual_declaration,
    parse_widget_declaration, register_declaration_name, starts_record_declaration,
    starts_visual_declaration, starts_widget_declaration,
};
// 引入拆分后的元素解析入口。
use element_parser::parse_element;
// 测试模块观测解析产出的 AST 值联合。
#[cfg(test)]
use super::{AttributeValue, Node};
// 引入顶层名称去重集合。
use std::collections::HashSet;

// 把 UIX 源码解析为具有唯一根元素的核心文档 AST。
pub(crate) fn parse_document(source: &str) -> Result<Document, Diagnostic> {
    // 创建 UTF-8 安全词法游标。
    let mut cursor = Cursor::new(source);
    // 跳过文档起始 trivia。
    cursor.skip_trivia()?;
    // 保存源码顺序中的顶层声明。
    let mut declarations = Vec::new();
    // 保存已声明样式类名。
    let mut style_names = HashSet::new();
    // 保存已声明主题名。
    let mut theme_names = HashSet::new();
    // 保存已声明关键帧名。
    let mut keyframe_names = HashSet::new();
    // 保存已声明组件名。
    let mut widget_names = HashSet::new();
    // 保存已声明 Visual 常量名。
    let mut visual_names = HashSet::new();
    // 解析根元素之前的声明区。
    loop {
        // @ 前缀开始导入、导出或主题声明。
        let declaration = if cursor.starts_with("@") {
            // 解析 @ 顶层声明。
            Some(parse_at_declaration(&mut cursor)?)
        // 裸标识符开始样式类声明。
        } else if cursor
            // 查看当前字符。
            .peek()
            // 样式类必须以标识符首字符开始。
            .is_some_and(|value| value.is_ascii_alphabetic() || value == '_')
        {
            // 解析具名样式类。
            Some(parse_style_class(&mut cursor)?)
        // 解析并验证顶层 Widget 声明。
        } else if starts_widget_declaration(&cursor) {
            // 先复用通用元素解析器读取组件体。
            let element = parse_element(&mut cursor, true)?;
            // 再验证 Widget 元数据与字段类型。
            Some(Declaration::Widget(parse_widget_declaration(element)?))
        // 解析并验证顶层 Record 声明。
        } else if starts_record_declaration(&cursor) {
            // 先复用通用元素解析器读取自闭合 record 标签。
            let element = parse_element(&mut cursor, true)?;
            // 再验证 Record 元数据与字段类型。
            Some(Declaration::Record(parse_record_declaration(element)?))
        // 解析并验证顶层 Visual 静态记录声明。
        } else if starts_visual_declaration(&cursor) {
            let element = parse_element(&mut cursor, true)?;
            Some(Declaration::Visual(parse_visual_declaration(element)?))
        } else {
            // 当前输入应为文档根元素。
            None
        };
        // 没有声明时结束声明区。
        let Some(declaration) = declaration else {
            // 退出声明循环。
            break;
        };
        // 验证具名声明唯一性。
        register_declaration_name(
            // 传递当前声明。
            &declaration,
            // 传递样式名称集合。
            &mut style_names,
            // 传递主题名称集合。
            &mut theme_names,
            // 传递关键帧名称集合。
            &mut keyframe_names,
            // 传递组件名称集合。
            &mut widget_names,
            &mut visual_names,
        )?;
        // 保存声明顺序。
        declarations.push(declaration);
        // 跳过声明间 trivia。
        cursor.skip_trivia()?;
    }
    // 空文档或只有声明的文档缺少根元素。
    if cursor.is_eof() {
        // 返回带修复建议的空文档诊断。
        return Err(Diagnostic::new(
            // 指向文件起点。
            cursor.point_span(),
            // 陈述失败原因。
            "UIX 文档缺少根元素",
            // 给出确定修复动作。
            "添加一个根元素，例如 <App />",
        ));
    }
    // 解析唯一根元素。
    let root = parse_element(&mut cursor, false)?;
    // 跳过根元素后的 trivia。
    cursor.skip_trivia()?;
    // 任何剩余内容都违反唯一根约束。
    if !cursor.is_eof() {
        // 根元素后的声明违反声明区顺序。
        let declaration_after_root = cursor.starts_with("@")
            // 裸标识符可能开始样式类。
            || cursor
                // 查看尾随首字符。
                .peek()
                // 验证标识符首字符。
                .is_some_and(|value| value.is_ascii_alphabetic() || value == '_')
            // Widget 也属于顶层声明。
            || starts_widget_declaration(&cursor)
            || starts_record_declaration(&cursor)
            || starts_visual_declaration(&cursor);
        // 为声明顺序提供专用诊断。
        if declaration_after_root {
            // 返回声明位置诊断。
            return Err(Diagnostic::new(
                // 指向尾随声明。
                cursor.point_span(),
                // 陈述失败原因。
                "顶层声明必须位于根元素之前",
                // 给出修复建议。
                "把 @import、@export、@theme、样式类、Widget、Record 或 Visual 移到文档开头",
            ));
        }
        // 返回第二根或尾随内容诊断。
        return Err(Diagnostic::new(
            // 指向未消费内容。
            cursor.point_span(),
            // 陈述失败原因。
            "UIX 文档只能包含一个根元素",
            // 给出结构修复建议。
            "把其余元素移动到当前根元素内部",
        ));
    }
    // 构建已验证文档。
    let document = Document {
        // 保存声明顺序。
        declarations,
        // 保存唯一根元素。
        root,
    };
    // 校验全部 record 类型引用都指向已声明 record。
    validate_record_references(&document)?;
    // 返回已验证文档。
    Ok(document)
}

// 校验组件 state 与 record 字段中的 record 引用都存在对应声明。
fn validate_record_references(document: &Document) -> Result<(), Diagnostic> {
    // 收集全部已声明 record 名。
    let record_names = document
        // 遍历声明。
        .declarations
        // 借用声明序列。
        .iter()
        // 只保留 record 声明名。
        .filter_map(|declaration| match declaration {
            // 提取 record 名。
            Declaration::Record(record) => Some(record.name.as_str()),
            // Visual 不参与 Record 类型命名空间。
            Declaration::Visual(_) => None,
            // 其余声明不占用 record 命名空间。
            _ => None,
        })
        // 收集为去重集合。
        .collect::<HashSet<_>>();
    // 逐一声明校验类型引用。
    for declaration in &document.declarations {
        // 按声明类别校验。
        match declaration {
            // 组件私有 state 的类型注解可能引用 record。
            Declaration::Widget(widget) => {
                // 遍历组件 state。
                for state in &widget.states {
                    // 只有类型化初始值带类型引用。
                    if let WidgetStateInitial::TypedExpression(value_type, _) = &state.initial {
                        // 校验类型中的 record 引用。
                        validate_value_type_record(value_type, &record_names, state.span)?;
                    }
                }
            }
            // record 字段可能引用其他 record。
            Declaration::Record(record) => {
                // 遍历 record 字段。
                for field in &record.fields {
                    // 校验字段类型中的 record 引用。
                    validate_value_type_record(&field.kind, &record_names, field.span)?;
                }
            }
            // Visual 字段由 Rust const 类型检查，不引用 UIX Record 类型系统。
            Declaration::Visual(_) => {}
            // 其余声明不含类型引用。
            _ => {}
        }
    }
    // 全部引用已兑底。
    Ok(())
}

// 递归校验单个类型中的 record 引用。
fn validate_value_type_record(
    // 接收待校验类型。
    value_type: &WidgetValueType,
    // 接收已声明 record 名集合。
    record_names: &HashSet<&str>,
    // 接收诊断定位跨度。
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    // 提取直接 record 或 Vec<Record> 的引用名称。
    let name = match value_type {
        // 直接 record 引用。
        WidgetValueType::Record(name) => name,
        // record 集合元素引用。
        WidgetValueType::VecOfRecord(name) => name,
        // 基础类型没有嵌套引用。
        _ => return Ok(()),
    };
    // 名称必须存在对应声明。
    if record_names.contains(name.as_str()) {
        // 引用已兑底。
        return Ok(());
    }
    // 返回未声明 record 诊断。
    Err(Diagnostic::new(
        // 指向类型引用。
        span,
        // 陈述失败原因。
        format!("record 类型 {name} 未在当前文档声明"),
        // 给出修复建议。
        "在顶层声明区添加 <Record name=\"...\" fields=\"...\" />，或改用基础类型",
    ))
}

// 递归解析一个普通或自闭合元素。
#[cfg(test)]
mod tests {
    // 引入待测解析入口和 AST 联合。
    use super::{AttributeValue, Node, parse_document};

    // 验证注释、属性、嵌套、自闭合、文本与插值保持顺序。
    #[test]
    fn parses_core_document_in_source_order() {
        // 构造覆盖核心语法的文档。
        let source = "/* header */\n<App theme=\"light\"><Text size={titleSize}>你好，{name}!</Text><Icon name=\"star\" /></App>";
        // 解析文档并要求成功。
        let document = parse_document(source).expect("核心文档应成功解析");
        // 根元素名称必须保留。
        assert_eq!(document.root.name, "App");
        // 根属性字面值必须去除双引号。
        assert_eq!(
            document.root.attributes[0].value,
            AttributeValue::Literal("light".to_string())
        );
        // 根元素必须保留两个有序子元素。
        assert_eq!(document.root.children.len(), 2);
        // 第一个子节点必须是 Text。
        let Node::Element(text) = &document.root.children[0] else {
            // 结构不匹配时主动失败。
            panic!("第一个子节点应为元素");
        };
        // 表达式属性必须保留源码。
        assert!(matches!(
            // 借用表达式属性以检查源码。
            &text.attributes[0].value,
            // 要求表达式节点保留原始内容。
            AttributeValue::Expression(node) if node.source == "titleSize"
        ));
        // 文本、插值、文本必须保持源顺序。
        assert!(matches!(&text.children[0], Node::Text(node) if node.value == "你好，"));
        // 中间节点必须是名称表达式。
        assert!(matches!(&text.children[1], Node::Interpolation(node) if node.source == "name"));
        // 尾部文本必须保留标点。
        assert!(matches!(&text.children[2], Node::Text(node) if node.value == "!"));
    }
    // 验证文本花括号转义不会被误判为插值。
    #[test]
    fn decodes_escaped_text_braces() {
        // 解析带转义花括号的文本。
        let document = parse_document("<Text>签名: \\{name\\}</Text>").expect("转义文本应成功");
        // 提取唯一文本节点。
        let Node::Text(text) = &document.root.children[0] else {
            // 结构不匹配时主动失败。
            panic!("应生成文本节点");
        };
        // 转义结果必须是字面花括号。
        assert_eq!(text.value, "签名: {name}");
    }
    // 验证 UTF-8 文本后的错误位置使用字符列而不是字节列。
    #[test]
    fn reports_utf8_line_and_column_for_mismatched_tag() {
        // 构造第二行中文文本后的错误结束标签。
        let source = "<App>\n  <Text>中文</Wrong>\n</App>";
        // 解析并取得预期诊断。
        let error = parse_document(source).expect_err("标签不匹配必须失败");
        // 错误必须定位到第二行。
        assert_eq!(error.span.line, 2);
        // 错误列必须按 Unicode 字符计算。
        assert_eq!(error.span.column, 13);
        // 原因必须包含实际与期望标签。
        assert!(error.message.contains("Wrong") && error.message.contains("Text"));
        // 修复建议必须给出正确结束标签。
        assert!(error.suggestion.contains("</Text>"));
    }
    // 验证文档严格执行唯一根元素约束。
    #[test]
    fn rejects_multiple_roots() {
        // 解析包含两个根元素的文档。
        let error = parse_document("<App /><Text />").expect_err("多个根必须失败");
        // 失败原因必须明确唯一根约束。
        assert!(error.message.contains("一个根元素"));
        // 解析缺失根元素的空文档。
        let error = parse_document("").expect_err("缺失根元素必须失败");
        // 失败原因必须明确指出根元素缺失。
        assert!(error.message.contains("缺少根元素"));
    }
    // 验证三类未闭合词法结构都提供修复建议。
    #[test]
    fn rejects_unclosed_lexical_structures() {
        // 逐项覆盖注释、字符串和表达式。
        for source in ["/* open", "<App name=\"open />", "<App name={value />"] {
            // 解析并取得诊断。
            let error = parse_document(source).expect_err("未闭合结构必须失败");
            // 每个诊断都必须包含原因。
            assert!(!error.message.is_empty());
            // 每个诊断都必须包含修复建议。
            assert!(!error.suggestion.is_empty());
        }
    }

    // 验证标签名必须遵循 PascalCase 起始约束。
    #[test]
    fn rejects_lowercase_tag_name() {
        // 解析小写标签并取得诊断。
        let error = parse_document("<button />").expect_err("小写标签必须失败");
        // 失败原因必须指出 PascalCase。
        assert!(error.message.contains("PascalCase"));
    }
}
