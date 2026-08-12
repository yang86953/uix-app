// 引入核心 AST、词法游标和诊断。
use super::{
    parse_at_declaration, parse_component_declaration, parse_expression, parse_style_class,
    parse_style_properties, register_declaration_name, starts_component_declaration, Attribute,
    AttributeValue, ControlBinding, Cursor, Declaration, Diagnostic, Document, Element,
    ExpressionKind, ExpressionNode, Node, SourceSpan, TextNode,
};
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
    // 保存已声明组件名。
    let mut component_names = HashSet::new();
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
        // 解析并验证顶层 Component 声明。
        } else if starts_component_declaration(&cursor) {
            // 先复用通用元素解析器读取组件体。
            let element = parse_element(&mut cursor, true)?;
            // 再验证 Component 元数据与字段类型。
            Some(Declaration::Component(parse_component_declaration(
                element,
            )?))
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
            // 传递组件名称集合。
            &mut component_names,
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
            // Component 也属于顶层声明。
            || starts_component_declaration(&cursor);
        // 为声明顺序提供专用诊断。
        if declaration_after_root {
            // 返回声明位置诊断。
            return Err(Diagnostic::new(
                // 指向尾随声明。
                cursor.point_span(),
                // 陈述失败原因。
                "顶层声明必须位于根元素之前",
                // 给出修复建议。
                "把 @import、@export、@theme、样式类或 Component 移到文档开头",
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
    // 返回已验证文档。
    Ok(Document { declarations, root })
}

// 递归解析一个普通或自闭合元素。
fn parse_element(
    cursor: &mut Cursor<'_>,
    allow_component_declaration: bool,
) -> Result<Element, Diagnostic> {
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
    // Component 只能作为顶层声明出现。
    if name == "Component" && !allow_component_declaration {
        // 返回非法嵌套或根位置诊断。
        return Err(Diagnostic::new(
            // 指向保留标签名。
            name_span,
            // 陈述失败原因。
            "<Component> 只能出现在顶层声明区",
            // 给出修复建议。
            "把 Component 定义移动到文档根元素之前",
        ));
    }
    // 保存声明顺序中的属性。
    let mut attributes = Vec::new();
    // 保存 If 或 For 专用控制绑定。
    let mut control = None;
    // 解析到标签结束标记。
    loop {
        // 跳过标签内空白和注释。
        cursor.skip_trivia()?;
        // 自闭合标签立即完成元素。
        if cursor.consume("/>") {
            // 控制元素必须声明专用绑定。
            require_control_binding(&name, control.as_ref(), cursor.point_span())?;
            // 返回无子节点元素。
            return Ok(Element {
                // 保存标签名。
                name,
                // 保存属性。
                attributes,
                // 自闭合元素没有子节点。
                children: Vec::new(),
                // 保存可选控制绑定。
                control,
                // 保存完整跨度。
                span: cursor.span_from(start),
                // 源码元素尚未经过组件展开，因此没有私有状态作用域标记。
                component_scopes: Vec::new(),
            });
        }
        // 普通开始标签进入子节点阶段。
        if cursor.consume(">") {
            // 控制元素必须声明专用绑定。
            require_control_binding(&name, control.as_ref(), cursor.point_span())?;
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
        // 花括号在开始标签内只允许作为 If 或 For 控制绑定。
        if cursor.starts_with("{") {
            // 控制元素不能把普通属性放在绑定之前。
            if !attributes.is_empty() {
                // 返回绑定顺序诊断。
                return Err(Diagnostic::new(
                    // 指向控制绑定起点。
                    cursor.point_span(),
                    // 陈述失败原因。
                    "If/For 控制绑定必须紧跟标签名",
                    // 给出规范顺序。
                    "使用 <If {condition}> 或 <For {item} in {items}>",
                ));
            }
            // 同一元素只能有一个控制绑定。
            if control.is_some() {
                // 返回重复绑定诊断。
                return Err(Diagnostic::new(
                    // 指向重复绑定。
                    cursor.point_span(),
                    // 陈述失败原因。
                    "控制元素只能声明一个绑定",
                    // 给出规范结构。
                    "保留一个 If 条件或一个 For 绑定",
                ));
            }
            // 解析对应控制绑定。
            control = Some(parse_control_binding(cursor, &name)?);
            // 继续读取标签结束标记。
            continue;
        }
        // For 控制绑定后只允许一个 key 表达式属性。
        if let Some(ControlBinding::For { key, .. }) = control.as_mut() {
            // 解析候选 key 属性。
            let attribute = parse_attribute(cursor)?;
            // 其他属性不属于 For 控制语法。
            if attribute.name != "key" {
                // 返回非法 For 属性诊断。
                return Err(Diagnostic::new(
                    // 指向完整属性。
                    attribute.span,
                    // 陈述失败原因。
                    "For 控制绑定后只允许 key 表达式属性",
                    // 给出规范示例。
                    "使用 <For {item} in {items} key={item.id}>",
                ));
            }
            // key 必须且只能声明一次。
            if key.is_some() {
                // 返回重复 key 诊断。
                return Err(Diagnostic::new(
                    // 指向重复属性。
                    attribute.span,
                    // 陈述失败原因。
                    "For 的 key 属性重复声明",
                    // 给出修复建议。
                    "只保留一个 key={expression}",
                ));
            }
            // key 必须使用花括号表达式。
            let AttributeValue::Expression(expression) = attribute.value else {
                // 返回 key 值形状诊断。
                return Err(Diagnostic::new(
                    // 指向完整属性。
                    attribute.span,
                    // 陈述失败原因。
                    "For 的 key 必须是花括号表达式",
                    // 给出规范示例。
                    "使用 key={item.id}",
                ));
            };
            // 保存稳定行身份表达式。
            *key = Some(expression);
            // 继续读取标签结束标记。
            continue;
        }
        // If 控制绑定后不允许普通属性。
        if control.is_some() {
            // 返回控制结构形状诊断。
            return Err(Diagnostic::new(
                // 指向多余内容。
                cursor.point_span(),
                // 陈述失败原因。
                "If/For 控制绑定后不允许普通属性",
                // 给出规范结构。
                "把条件或循环逻辑完整写在控制绑定中",
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
        // 顶层指令不得嵌套在元素树中。
        if ["@import", "@export", "@theme"]
            // 遍历保留指令前缀。
            .iter()
            // 检查当前位置是否匹配。
            .any(|directive| cursor.starts_with(directive))
        {
            // 返回非法嵌套诊断。
            return Err(Diagnostic::new(
                // 指向嵌套指令。
                cursor.point_span(),
                // 陈述失败原因。
                "@import、@export 与 @theme 只能出现在顶层声明区",
                // 给出修复建议。
                "把顶层指令移动到文档根元素之前",
            ));
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
                // 保存可选控制绑定。
                control,
                // 保存完整跨度。
                span: cursor.span_from(start),
                // 源码元素尚未经过组件展开，因此没有私有状态作用域标记。
                component_scopes: Vec::new(),
            });
        }
        // 小于号开始嵌套元素。
        if cursor.starts_with("<") {
            // 递归解析并保存元素。
            children.push(Node::Element(parse_element(cursor, false)?));
            // 继续解析下一节点。
            continue;
        }
        // 花括号开始文本插值。
        if cursor.starts_with("{") {
            // 解析并验证插值表达式。
            let expression = parse_braced_expression_node(cursor)?;
            // 保存表达式节点。
            children.push(Node::Interpolation(expression));
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
    // 事件属性以 @ 前缀区分。
    let is_event = cursor.consume("@");
    // 解析属性名。
    let (base_name, _) = cursor.identifier().ok_or_else(|| {
        // 构造非法属性名诊断。
        Diagnostic::new(
            // 指向当前位置。
            cursor.point_span(),
            // 陈述失败原因。
            "标签内存在非法属性名",
            // 给出命名规则。
            "属性名必须以 ASCII 字母或下划线开头，事件使用 @click",
        )
    })?;
    // 恢复事件属性的语义前缀。
    let name = if is_event {
        // 保存带 @ 的事件名。
        format!("@{base_name}")
    } else {
        // 普通属性保持原名。
        base_name
    };
    // 跳过等号前空白。
    cursor.skip_trivia()?;
    // 记录属性是否显式提供值。
    let has_explicit_value = cursor.consume("=");
    // 普通无值属性生成等价 true 的布尔简写。
    let value = if !has_explicit_value {
        // 事件处理器不能省略回调表达式。
        if is_event {
            // 返回缺少事件值诊断。
            return Err(Diagnostic::new(
                // 覆盖事件属性名。
                cursor.span_from(start),
                // 陈述失败原因。
                format!("事件属性 {name} 缺少 = 和处理器"),
                // 给出合法事件写法。
                format!("使用 {name}=\"handler()\""),
            ));
        }
        // 统一交给后续布尔属性所有者验证类型。
        AttributeValue::Literal("true".to_string())
    } else {
        // 跳过等号后空白。
        cursor.skip_trivia()?;
        // 事件属性的双引号内容按受限表达式解析。
        if is_event {
            // 读取未解码的事件表达式源码和内容跨度。
            let (source, content_span) = cursor.quoted_expression_source()?;
            // 使用完整属性跨度作为外围节点跨度。
            let span = cursor.span_from(start);
            // 保存已验证事件表达式。
            AttributeValue::Expression(parse_expression_node(source, span, content_span)?)
        // style 双引号属性使用共享样式语法。
        } else if name == "style" && cursor.starts_with("\"") {
            // 读取内联样式源码和内容位置。
            let (source, content_span) = cursor.quoted_style_source()?;
            // 解析内联属性并禁止 extends。
            let (_, properties) = parse_style_properties(&source, content_span, false)?;
            // 保存结构化内联样式。
            AttributeValue::InlineStyle(properties)
        // 普通双引号属性保持字面量。
        } else if cursor.starts_with("\"") {
            // 解析双引号字面量。
            AttributeValue::Literal(cursor.quoted_literal()?)
        // 花括号属性值保存已验证表达式。
        } else if cursor.starts_with("{") {
            // 解析并验证表达式节点。
            let expression = parse_braced_expression_node(cursor)?;
            // 保存表达式值。
            AttributeValue::Expression(expression)
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
        }
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

// 解析 If 或 For 开始标签中的专用控制绑定。
fn parse_control_binding(
    cursor: &mut Cursor<'_>,
    element_name: &str,
) -> Result<ControlBinding, Diagnostic> {
    // If 直接保存一个条件表达式。
    if element_name == "If" {
        // 解析并返回条件绑定。
        return Ok(ControlBinding::If(parse_braced_expression_node(cursor)?));
    }
    // 其他标签除 For 外不允许匿名花括号绑定。
    if element_name != "For" {
        // 返回控制绑定位置诊断。
        return Err(Diagnostic::new(
            // 指向花括号起点。
            cursor.point_span(),
            // 陈述失败原因。
            format!("元素 <{element_name}> 不支持控制绑定"),
            // 给出支持结构。
            "只在 <If {condition}> 或 <For {item} in {items}> 中使用",
        ));
    }
    // 解析 For 的单标识符绑定声明。
    let binding_expression = parse_braced_expression_node(cursor)?;
    // 绑定必须是没有成员或运算的单标识符。
    let binding = match &binding_expression.expression.kind {
        // 提取合法标识符。
        ExpressionKind::Identifier(value) => value.clone(),
        // 其他表达式不能声明循环项。
        _ => {
            // 返回绑定形状诊断。
            return Err(Diagnostic::new(
                // 指向绑定表达式。
                binding_expression.expression.span,
                // 陈述失败原因。
                "For 绑定必须是单个标识符",
                // 给出合法示例。
                "使用 <For {item} in {items}>",
            ));
        }
    };
    // 跳过绑定后的空白或注释。
    cursor.skip_trivia()?;
    // 可选解析第二个索引绑定。
    let (index_binding, index_span) = if cursor.starts_with("{") {
        // 解析索引绑定表达式。
        let index_expression = parse_braced_expression_node(cursor)?;
        // 索引绑定同样必须是单标识符。
        let index_name = match &index_expression.expression.kind {
            // 提取合法索引标识符。
            ExpressionKind::Identifier(value) => value.clone(),
            // 其他表达式不能声明索引。
            _ => {
                // 返回索引绑定形状诊断。
                return Err(Diagnostic::new(
                    // 指向索引绑定表达式。
                    index_expression.expression.span,
                    // 陈述失败原因。
                    "For 索引绑定必须是单个标识符",
                    // 给出合法示例。
                    "使用 <For {item} {index} in {items}>",
                ));
            }
        };
        // 循环项和索引不能声明为同名绑定。
        if index_name == binding {
            // 返回重复绑定诊断。
            return Err(Diagnostic::new(
                // 指向重复索引绑定。
                index_expression.expression.span,
                // 陈述失败原因。
                "For 的循环项与索引绑定不能同名",
                // 给出不同名称示例。
                "使用 <For {item} {index} in {items}>",
            ));
        }
        // 保存名称与声明跨度。
        (
            // 保存索引名称。
            Some(index_name),
            // 保存索引跨度。
            Some(index_expression.expression.span),
        )
    } else {
        // 没有第二绑定时保持空值。
        (None, None)
    };
    // 跳过可选索引绑定后的空白或注释。
    cursor.skip_trivia()?;
    // 要求小写 in 关键字。
    let Some((keyword, keyword_span)) = cursor.identifier() else {
        // 返回缺少 in 诊断。
        return Err(Diagnostic::new(
            // 指向当前位置。
            cursor.point_span(),
            // 陈述失败原因。
            "For 绑定缺少 in",
            // 给出合法示例。
            "使用 <For {item} in {items}>",
        ));
    };
    // 拒绝其他标识符充当 in。
    if keyword != "in" {
        // 返回错误关键字诊断。
        return Err(Diagnostic::new(
            // 指向错误关键字。
            keyword_span,
            // 陈述失败原因。
            format!("For 绑定期望 in，但得到 {keyword}"),
            // 给出合法示例。
            "使用 <For {item} in {items}>",
        ));
    }
    // 跳过 in 后空白或注释。
    cursor.skip_trivia()?;
    // 数据源必须使用花括号表达式。
    if !cursor.starts_with("{") {
        // 返回缺失数据源诊断。
        return Err(Diagnostic::new(
            // 指向数据源位置。
            cursor.point_span(),
            // 陈述失败原因。
            "For 的数据源必须是花括号表达式",
            // 给出合法示例。
            "使用 <For {item} in {items}>",
        ));
    }
    // 解析并验证数据源表达式。
    let iterable = parse_braced_expression_node(cursor)?;
    // 返回完整 For 绑定。
    Ok(ControlBinding::For {
        // 保存绑定名。
        binding,
        // 保存单标识符跨度。
        binding_span: binding_expression.expression.span,
        // 保存可选索引绑定。
        index_binding,
        // 保存可选索引跨度。
        index_span,
        // 保存数据源。
        iterable,
        // key 属性由开始标签后续解析。
        key: None,
    })
}

// 验证 If 与 For 元素没有遗漏规范要求的绑定。
fn require_control_binding(
    element_name: &str,
    control: Option<&ControlBinding>,
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    // 普通元素或已有绑定的控制元素直接通过。
    if !matches!(element_name, "If" | "For") || control.is_some() {
        // 报告结构有效。
        return Ok(());
    }
    // 返回缺失控制绑定诊断。
    Err(Diagnostic::new(
        // 指向开始标签结束位置。
        span,
        // 陈述失败原因。
        format!("<{element_name}> 缺少控制绑定"),
        // 给出对应合法结构。
        if element_name == "If" {
            // 返回 If 修复示例。
            "使用 <If {condition}>"
        } else {
            // 返回 For 修复示例。
            "使用 <For {item} in {items}>"
        },
    ))
}

// 读取花括号内容并构造已验证表达式节点。
fn parse_braced_expression_node(cursor: &mut Cursor<'_>) -> Result<ExpressionNode, Diagnostic> {
    // 读取规范化源码、外围跨度与内容位置。
    let (source, span, content_span) = cursor.braced_expression()?;
    // 解析并返回表达式节点。
    parse_expression_node(source, span, content_span)
}

// 把表达式源码和位置组合成确定性节点。
fn parse_expression_node(
    source: String,
    span: SourceSpan,
    content_span: SourceSpan,
) -> Result<ExpressionNode, Diagnostic> {
    // 解析受限表达式 AST。
    let expression = parse_expression(&source, content_span)?;
    // 返回同时保留源码与 AST 的节点。
    Ok(ExpressionNode {
        // 保存源码供后续诊断和代码生成。
        source,
        // 保存确定性 AST。
        expression,
        // 保存外围结构跨度。
        span,
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
