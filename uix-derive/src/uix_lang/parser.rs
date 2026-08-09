// 引入核心 AST、词法游标和诊断。
use super::{
    Attribute, AttributeValue, Cursor, Diagnostic, Document, Element, ExpressionNode, Node,
    TextNode,
};

// 把 UIX 源码解析为具有唯一根元素的核心文档 AST。
pub(crate) fn parse_document(source: &str) -> Result<Document, Diagnostic> {
    // 创建 UTF-8 安全词法游标。
    let mut cursor = Cursor::new(source);
    // 跳过文档起始 trivia。
    cursor.skip_trivia()?;
    // 空文档缺少规范要求的根元素。
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
    let root = parse_element(&mut cursor)?;
    // 跳过根元素后的 trivia。
    cursor.skip_trivia()?;
    // 任何剩余内容都违反唯一根约束。
    if !cursor.is_eof() {
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
    // 返回已验证文档。
    Ok(Document { root })
}

// 递归解析一个普通或自闭合元素。
fn parse_element(cursor: &mut Cursor<'_>) -> Result<Element, Diagnostic> {
    // 保存元素起点。
    let start = cursor.offset();
    // 要求开始标签标记。
    if !cursor.consume("<") || cursor.starts_with("/") {
        // 返回缺少开始标签诊断。
        return Err(Diagnostic::new(
            // 指向当前结构起点。
            cursor.point_span(),
            // 陈述失败原因。
            "期望元素开始标签",
            // 给出合法标签示例。
            "使用 PascalCase 标签，例如 <Button />",
        ));
    }
    // 解析标签名。
    let (name, name_span) = cursor.identifier().ok_or_else(|| {
        // 构造缺失标签名诊断。
        Diagnostic::new(
            // 指向标签名位置。
            cursor.point_span(),
            // 陈述失败原因。
            "开始标签缺少合法名称",
            // 给出命名规范。
            "标签名必须以 ASCII 大写字母开头",
        )
    })?;
    // 标签名必须遵循 PascalCase 起始约束。
    if !name.starts_with(|value: char| value.is_ascii_uppercase()) {
        // 返回命名规范诊断。
        return Err(Diagnostic::new(
            // 指向标签名。
            name_span,
            // 陈述失败原因。
            "标签名必须使用 PascalCase",
            // 给出修复示例。
            "把标签名首字母改为大写，例如 <Button />",
        ));
    }
    // 保存声明顺序中的属性。
    let mut attributes = Vec::new();
    // 解析到标签结束标记。
    loop {
        // 跳过标签内空白和注释。
        cursor.skip_trivia()?;
        // 自闭合标签立即完成元素。
        if cursor.consume("/>") {
            // 返回无子节点元素。
            return Ok(Element {
                // 保存标签名。
                name,
                // 保存属性。
                attributes,
                // 自闭合元素没有子节点。
                children: Vec::new(),
                // 保存完整跨度。
                span: cursor.span_from(start),
            });
        }
        // 普通开始标签进入子节点阶段。
        if cursor.consume(">") {
            // 结束属性循环。
            break;
        }
        // 文件末尾表示开始标签未闭合。
        if cursor.is_eof() {
            // 返回未闭合标签诊断。
            return Err(Diagnostic::new(
                // 覆盖当前元素。
                cursor.span_from(start),
                // 陈述失败原因。
                "开始标签缺少 > 或 />",
                // 给出确定修复动作。
                "在开始标签末尾添加 > 或 />",
            ));
        }
        // 解析一个属性。
        attributes.push(parse_attribute(cursor)?);
    }
    // 保存有序子节点。
    let mut children = Vec::new();
    // 解析到匹配的结束标签。
    loop {
        // 文件末尾表示元素未闭合。
        if cursor.is_eof() {
            // 返回缺少结束标签诊断。
            return Err(Diagnostic::new(
                // 覆盖当前元素。
                cursor.span_from(start),
                // 陈述失败原因。
                format!("元素 <{name}> 缺少结束标签"),
                // 给出匹配结束标签。
                format!("添加 </{name}>"),
            ));
        }
        // 跳过子节点之间的规范注释。
        if cursor.starts_with("//") || cursor.starts_with("/*") {
            // 消费注释。
            cursor.skip_trivia()?;
            // 继续解析下一节点。
            continue;
        }
        // 匹配结束标签。
        if cursor.starts_with("</") {
            // 消费结束标签前缀。
            cursor.consume("</");
            // 解析结束标签名。
            let (closing, closing_span) = cursor.identifier().ok_or_else(|| {
                // 构造缺失结束标签名诊断。
                Diagnostic::new(
                    // 指向名称位置。
                    cursor.point_span(),
                    // 陈述失败原因。
                    "结束标签缺少名称",
                    // 给出当前元素需要的名称。
                    format!("使用 </{name}>"),
                )
            })?;
            // 跳过结束标签内空白。
            cursor.skip_trivia()?;
            // 要求结束尖括号。
            if !cursor.consume(">") {
                // 返回结束标签未闭合诊断。
                return Err(Diagnostic::new(
                    // 指向当前位置。
                    cursor.point_span(),
                    // 陈述失败原因。
                    "结束标签缺少 >",
                    // 给出正确结束标签。
                    format!("使用 </{name}>"),
                ));
            }
            // 开始与结束标签名必须完全一致。
            if closing != name {
                // 返回配对失败诊断。
                return Err(Diagnostic::new(
                    // 指向错误结束标签名。
                    closing_span,
                    // 陈述实际和期望名称。
                    format!("结束标签 </{closing}> 与 <{name}> 不匹配"),
                    // 给出正确结束标签。
                    format!("把结束标签改为 </{name}>"),
                ));
            }
            // 返回完整普通元素。
            return Ok(Element {
                // 保存标签名。
                name,
                // 保存属性。
                attributes,
                // 保存有序子节点。
                children,
                // 保存完整跨度。
                span: cursor.span_from(start),
            });
        }
        // 小于号开始嵌套元素。
        if cursor.starts_with("<") {
            // 递归解析并保存元素。
            children.push(Node::Element(parse_element(cursor)?));
            // 继续解析下一节点。
            continue;
        }
        // 花括号开始文本插值。
        if cursor.starts_with("{") {
            // 解析表达式源码与跨度。
            let (source, span) = cursor.braced_expression()?;
            // 保存表达式节点。
            children.push(Node::Interpolation(ExpressionNode { source, span }));
            // 继续解析下一节点。
            continue;
        }
        // 其余内容按文本节点解析。
        children.push(Node::Text(parse_text(cursor)?));
    }
}

// 解析一个具名属性。
fn parse_attribute(cursor: &mut Cursor<'_>) -> Result<Attribute, Diagnostic> {
    // 保存属性起点。
    let start = cursor.offset();
    // 解析属性名。
    let (name, _) = cursor.identifier().ok_or_else(|| {
        // 构造非法属性名诊断。
        Diagnostic::new(
            // 指向当前位置。
            cursor.point_span(),
            // 陈述失败原因。
            "标签内存在非法属性名",
            // 给出命名规则。
            "属性名必须以 ASCII 字母或下划线开头",
        )
    })?;
    // 跳过等号前空白。
    cursor.skip_trivia()?;
    // 属性必须显式赋值。
    if !cursor.consume("=") {
        // 返回缺少等号诊断。
        return Err(Diagnostic::new(
            // 覆盖属性名。
            cursor.span_from(start),
            // 陈述失败原因。
            format!("属性 {name} 缺少 = 和属性值"),
            // 给出合法写法。
            format!("使用 {name}=\"value\" 或 {name}={{expression}}"),
        ));
    }
    // 跳过等号后空白。
    cursor.skip_trivia()?;
    // 按首字符选择属性值类型。
    let value = if cursor.starts_with("\"") {
        // 解析双引号字面量。
        AttributeValue::Literal(cursor.quoted_literal()?)
    // 花括号属性值保存表达式源码。
    } else if cursor.starts_with("{") {
        // 解析表达式源码。
        let (source, _) = cursor.braced_expression()?;
        // 保存表达式值。
        AttributeValue::Expression(source)
    // 其他写法违反属性值语法。
    } else {
        // 返回非法属性值诊断。
        return Err(Diagnostic::new(
            // 指向属性值位置。
            cursor.point_span(),
            // 陈述失败原因。
            format!("属性 {name} 的值格式无效"),
            // 给出合法写法。
            "属性值必须使用双引号或花括号",
        ));
    };
    // 返回完整属性。
    Ok(Attribute {
        // 保存属性名。
        name,
        // 保存属性值。
        value,
        // 保存完整跨度。
        span: cursor.span_from(start),
    })
}

// 解析到下一结构边界的文本并处理花括号转义。
fn parse_text(cursor: &mut Cursor<'_>) -> Result<TextNode, Diagnostic> {
    // 保存文本起点。
    let start = cursor.offset();
    // 保存解码后的文本。
    let mut value = String::new();
    // 消费到标签、插值或注释起点。
    while !cursor.is_eof()
        // 标签起点终止文本。
        && !cursor.starts_with("<")
        // 插值起点终止文本。
        && !cursor.starts_with("{")
        // 单行注释起点终止文本。
        && !cursor.starts_with("//")
        // 块注释起点终止文本。
        && !cursor.starts_with("/*")
    {
        // 反斜杠只允许转义花括号或反斜杠。
        if cursor.consume("\\") {
            // 读取被转义字符。
            let Some(escaped) = cursor.bump() else {
                // 返回悬空转义诊断。
                return Err(Diagnostic::new(
                    // 覆盖文本节点。
                    cursor.span_from(start),
                    // 陈述失败原因。
                    "文本以未完成的转义结尾",
                    // 给出修复建议。
                    "删除末尾反斜杠或补充被转义字符",
                ));
            };
            // 只接受规范声明的转义字符。
            if !matches!(escaped, '{' | '}' | '\\') {
                // 返回非法转义诊断。
                return Err(Diagnostic::new(
                    // 覆盖非法转义。
                    cursor.span_from(start),
                    // 陈述失败原因。
                    format!("文本不支持 \\{escaped} 转义"),
                    // 给出允许集合。
                    "只使用 \\{、\\} 或 \\\\ 转义",
                ));
            }
            // 保存被转义字符。
            value.push(escaped);
        // 普通字符原样保存。
        } else if let Some(next) = cursor.bump() {
            // 追加普通字符。
            value.push(next);
        }
    }
    // 返回文本节点。
    Ok(TextNode {
        // 保存解码文本。
        value,
        // 保存原始跨度。
        span: cursor.span_from(start),
    })
}

// 集中验证核心解析 Gate 的结构与诊断契约。
#[cfg(test)]
mod tests {
    // 引入待测解析入口和 AST 联合。
    use super::{parse_document, AttributeValue, Node};

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
        assert_eq!(
            text.attributes[0].value,
            AttributeValue::Expression("titleSize".to_string())
        );
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
