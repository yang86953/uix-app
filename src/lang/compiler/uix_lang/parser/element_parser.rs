// 拆分自 parser.rs：元素、属性与文本解析（含表达式节点构造）。
// 引入元素解析所需的 AST、词法游标和诊断。
use super::super::{
    Attribute, AttributeValue, ControlBinding, Cursor, Diagnostic, Element, ExpressionNode, Node,
    SourceSpan, TextNode, WidgetMemberBlock, WidgetMemberKind, parse_expression,
    parse_style_properties,
};
// 引入拆分后的控制绑定解析入口。
use super::control_binding::{parse_control_binding, require_control_binding};

pub(super) fn parse_element(
    cursor: &mut Cursor<'_>,
    allow_widget_declaration: bool,
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
    // Widget 只能作为顶层声明出现。
    if name == "Widget" && !allow_widget_declaration {
        // 返回非法嵌套或根位置诊断。
        return Err(Diagnostic::new(
            // 指向保留标签名。
            name_span,
            // 陈述失败原因。
            "<Widget> 只能出现在顶层声明区",
            // 给出修复建议。
            "把 Widget 定义移动到文档根元素之前",
        ));
    }
    // Record 只能作为顶层声明出现。
    if name == "Record" && !allow_widget_declaration {
        // 返回非法嵌套或根位置诊断。
        return Err(Diagnostic::new(
            // 指向保留标签名。
            name_span,
            // 陈述失败原因。
            "<Record> 只能出现在顶层声明区",
            // 给出修复建议。
            "把 Record 定义移动到文档根元素之前",
        ));
    }
    // Visual 只能作为顶层声明出现。
    if name == "Visual" && !allow_widget_declaration {
        return Err(Diagnostic::new(
            name_span,
            "<Visual> 只能出现在顶层声明区",
            "把 Visual 定义移动到文档根元素之前",
        ));
    }
    // 保存声明顺序中的属性。
    let mut attributes = Vec::new();
    // 保存 If、ElseIf 或 For 专用控制绑定。
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
                widget_scopes: Vec::new(),
                // 源码元素尚未经过组件展开，因此没有循环事件捕获契约。
                for_iteration_clones: Vec::new(),
                // 源码元素尚未经过组件展开，因此没有逐迭代组件准备语句。
                for_iteration_setup: Vec::new(),
                for_iteration_outer_captures: Vec::new(),
            reactive_setup: None,
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
        // 花括号在开始标签内只允许作为 If、ElseIf 或 For 控制绑定。
        if cursor.starts_with("{") {
            // 控制元素不能把普通属性放在绑定之前。
            if !attributes.is_empty() {
                // 返回绑定顺序诊断。
                return Err(Diagnostic::new(
                    // 指向控制绑定起点。
                    cursor.point_span(),
                    // 陈述失败原因。
                    "If/ElseIf/For 控制绑定必须紧跟标签名",
                    // 给出规范顺序。
                    "使用 <If {condition}>、<ElseIf {condition}> 或 <For {item} in {items}>",
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
        // If 与 ElseIf 控制绑定后不允许普通属性。
        if control.is_some() {
            // 返回控制结构形状诊断。
            return Err(Diagnostic::new(
                // 指向多余内容。
                cursor.point_span(),
                // 陈述失败原因。
                "If/ElseIf/For 控制绑定后不允许普通属性",
                // 给出规范结构。
                "把条件或循环逻辑完整写在控制绑定中",
            ));
        }
        // Else 分支不接受条件或普通属性。
        if name == "Else" {
            // 返回 Else 属性形状诊断。
            return Err(Diagnostic::new(
                // 指向非法属性起点。
                cursor.point_span(),
                // 陈述 Else 只承担兜底分支。
                "Else 不接受控制绑定或普通属性",
                // 给出规范空属性结构。
                "使用 <Else>...</Else>",
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
                widget_scopes: Vec::new(),
                // 源码元素尚未经过组件展开，因此没有循环事件捕获契约。
                for_iteration_clones: Vec::new(),
                // 源码元素尚未经过组件展开，因此没有逐迭代组件准备语句。
                for_iteration_setup: Vec::new(),
                for_iteration_outer_captures: Vec::new(),
            reactive_setup: None,
            });
        }
        // Widget 模板允许 @props/@state/@computed/@actions 成员声明块。
        if name == "Widget" {
            // 尝试解析成员块；前瞻未命中时保持普通文本语义。
            let member_block = try_parse_widget_member_block(cursor)?;
            // 命中成员块时保存节点。
            if let Some(block) = member_block {
                // 保存成员声明块节点。
                children.push(Node::WidgetMember(block));
                // 继续解析下一节点。
                continue;
            }
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

// 尝试把 @名称 { ... } 形状解析为 Widget 成员声明块。
// 先做无副作用的前瞻（允许前置空白），命中已登记成员且随后为花括号块
// 时才消费输入；其余情况返回 None 并保持普通文本语义，避免破坏模板文本。
fn try_parse_widget_member_block(
    cursor: &mut Cursor<'_>,
) -> Result<Option<WidgetMemberBlock>, Diagnostic> {
    // 读取当前源码余量做只读前瞻。
    let remaining = &cursor.source()[cursor.offset()..];
    // 去除成员块前的缩进与换行。
    let trimmed = remaining.trim_start();
    // 快速排除非 @ 形状。
    let Some(after_at) = trimmed.strip_prefix('@') else {
        // 回退到普通文本路径。
        return Ok(None);
    };
    // 提取连续标识符字符作为候选成员名。
    let name_end = after_at
        // 寻找首个非标识符字符位置。
        .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        // 全部剩余都是标识符时取整体长度。
        .unwrap_or(after_at.len());
    // 截取候选名称。
    let candidate = &after_at[..name_end];
    // 名称必须是已登记成员，防止把普通文本中的 @ 提及误判为声明。
    let member = match candidate {
        // props 声明块。
        "props" => WidgetMemberKind::Props,
        // state 声明块。
        "state" => WidgetMemberKind::State,
        // computed 声明块。
        "computed" => WidgetMemberKind::Computed,
        // actions 声明块。
        "actions" => WidgetMemberKind::Actions,
        // 其他名称保持普通文本语义。
        _ => return Ok(None),
    };
    // 名称之后必须存在花括号块起点；否则可能是行内提及文本。
    if !after_at[name_end..].trim_start().starts_with('{') {
        // 回退到普通文本路径。
        return Ok(None);
    }
    // 前瞻命中；保存 @ 起点。
    let start = cursor.offset();
    // 真正消费前置空白并读取规范名与跨度。
    cursor.skip_trivia()?;
    // 消费事件样式前缀。
    cursor.consume("@");
    // 读取成员名；词法形状已由前瞻保证，跨度仅作占位。
    let _ = cursor
        .identifier()
        // 前瞻已保证可读；此处仅防御词法边界变化。
        .ok_or_else(|| {
            // 返回非法成员名诊断。
            Diagnostic::new(
                // 指向 @ 后位置。
                cursor.point_span(),
                // 说明成员名不可读。
                "成员块缺少合法名称",
                // 给出允许集合。
                "只使用 @props、@state、@computed 或 @actions",
            )
        })?;
    // 跳过名称与花括号之间的空白。
    cursor.skip_trivia()?;
    // 读取容错花括号体。
    let (body, _) = cursor.braced_member_block()?;
    // 返回成员块节点并保留 @ 起点的完整跨度。
    Ok(Some(WidgetMemberBlock {
        // 保存成员类别。
        member,
        // 保存规范化声明体。
        body,
        // 保存从 @ 开始的完整跨度。
        span: cursor.span_from(start),
    }))
}

// 解析一个具名属性。
pub(super) fn parse_attribute(cursor: &mut Cursor<'_>) -> Result<Attribute, Diagnostic> {
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
pub(super) fn parse_braced_expression_node(
    cursor: &mut Cursor<'_>,
) -> Result<ExpressionNode, Diagnostic> {
    // 读取规范化源码、外围跨度与内容位置。
    let (source, span, content_span) = cursor.braced_expression()?;
    // 解析并返回表达式节点。
    parse_expression_node(source, span, content_span)
}

// 把表达式源码和位置组合成确定性节点。
pub(super) fn parse_expression_node(
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
pub(super) fn parse_text(cursor: &mut Cursor<'_>) -> Result<TextNode, Diagnostic> {
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
