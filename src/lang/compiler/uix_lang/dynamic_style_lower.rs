// 引入确定性目标类映射。
use std::collections::{BTreeMap, VecDeque};

// 引入组件展开器与字段绑定。
use super::widget_codegen::{Bindings, WidgetExpander};
// 引入动态样式与表达式 AST。
use super::{
    Attribute, AttributeValue, Diagnostic, DynamicStyleBinding, DynamicStyleKey,
    DynamicStyleVariant, Element, Expression, ExpressionKind, Node, SourceSpan, StyleProperty,
};

// 判断组件声明子树是否需要动态样式私有状态作用域。
pub(super) fn nodes_use_set_style(nodes: &[Node]) -> bool {
    // 任一元素或插值包含内置调用即可建立组件作用域。
    nodes.iter().any(node_uses_set_style)
}

// 判断单个节点是否包含 setStyle 调用。
fn node_uses_set_style(node: &Node) -> bool {
    // 按节点形状递归检查。
    match node {
        // 元素同时检查属性、控制表达式与子节点。
        Node::Element(element) => {
            // 检查全部属性表达式。
            element.attributes.iter().any(|attribute| {
                // 只处理表达式属性。
                matches!(&attribute.value, AttributeValue::Expression(node) if expression_uses_set_style(&node.expression))
            }) || element.children.iter().any(node_uses_set_style)
        }
        // 普通文本没有调用。
        Node::Text(_) => false,
        // 插值表达式按表达式树检查。
        Node::Interpolation(node) => expression_uses_set_style(&node.expression),
        // 成员块由声明解析器剥离；其内部语句不经过节点树。
        Node::WidgetMember(_) => false,
    }
}

// 判断表达式树是否包含框架内置 setStyle 调用。
pub(super) fn expression_uses_set_style(expression: &Expression) -> bool {
    // 当前节点为直接调用时立即命中。
    if matches!(
        &expression.kind,
        ExpressionKind::Call { callee, .. }
            if matches!(&callee.kind, ExpressionKind::Identifier(name) if name == "setStyle")
    ) {
        // 报告命中。
        return true;
    }
    // 递归检查全部复合表达式分支。
    match &expression.kind {
        // action 内联块中的全部表达式仍归当前实际事件 View。
        ExpressionKind::LoweredAction(action) => {
            let mut found = false;
            let _ = super::action_semantic::visit_action_block_expressions(
                &action.block,
                &mut |expression| {
                    found |= expression_uses_set_style(expression);
                    Ok(())
                },
            );
            found
        }
        // 一元表达式检查操作数。
        ExpressionKind::Unary { operand, .. } => expression_uses_set_style(operand),
        // 二元表达式检查任一操作数。
        ExpressionKind::Binary { left, right, .. } => {
            expression_uses_set_style(left) || expression_uses_set_style(right)
        }
        // 三元表达式检查条件与两个结果分支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_uses_set_style(condition)
                || expression_uses_set_style(then_branch)
                || expression_uses_set_style(else_branch)
        }
        // 成员访问检查对象。
        ExpressionKind::Member { object, .. } => expression_uses_set_style(object),
        // 下标访问检查对象与索引。
        ExpressionKind::Index { object, index } => {
            expression_uses_set_style(object) || expression_uses_set_style(index)
        }
        // 普通调用检查目标与全部参数。
        ExpressionKind::Call { callee, arguments } => {
            expression_uses_set_style(callee)
                || arguments
                    .iter()
                    .any(|argument| expression_uses_set_style(&argument.value))
        }
        // 对象检查全部字段值。
        ExpressionKind::Object(fields) => fields
            .iter()
            .any(|field| expression_uses_set_style(&field.value)),
        // 数组检查全部成员。
        ExpressionKind::Array(items) => items.iter().any(expression_uses_set_style),
        // 受限闭包检查唯一表达式体。
        ExpressionKind::Closure { body, .. } => expression_uses_set_style(body),
        // 叶表达式没有内部调用。
        ExpressionKind::Identifier(_)
        | ExpressionKind::Number(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Boolean(_) => false,
    }
}

// 实现动态样式目标解析、作用域校验与 setter 降低。
impl WidgetExpander {
    // 把当前核心 View 的 setStyle 调用改写为闭合类型化状态分支。
    pub(super) fn prepare_dynamic_style(
        // 可变借用文档展开状态。
        &mut self,
        // 接收尚未做静态样式展开的元素。
        element: &mut Element,
        // 接收当前组件字段绑定以改写显式 key。
        bindings: &Bindings,
    ) -> Result<bool, Diagnostic> {
        // 保存按首次出现顺序收集的目标类与调用跨度。
        let mut targets = Vec::<(String, SourceSpan)>::new();
        // 扫描全部属性并限制副作用位置。
        for attribute in &element.attributes {
            // 只检查表达式属性。
            let AttributeValue::Expression(node) = &attribute.value else {
                // 继续处理下一属性。
                continue;
            };
            // 收集当前表达式中的目标类。
            let mut current = Vec::new();
            // 提取全部直接 setStyle 调用。
            collect_set_style_targets(&node.expression, &mut current)?;
            // 没有调用时继续。
            if current.is_empty() {
                // 继续处理下一属性。
                continue;
            }
            // setStyle 只允许由当前 View 的事件触发。
            if !attribute.name.starts_with('@') {
                // 返回组件外观副作用位置诊断。
                return Err(Diagnostic::new(
                    attribute.span,
                    "setStyle 只能在 Widget 的事件处理器中使用",
                    "把 setStyle('className') 移入当前 View 的 @click、@mouseEnter 或 @mouseLeave",
                ));
            }
            // 合并当前事件引用的目标类。
            targets.extend(current);
        }
        // 没有动态调用时保留静态样式路径。
        if targets.is_empty() {
            // 报告未消费动态样式。
            return Ok(false);
        }
        // 控制标签没有可被直接换肤的实际 View。
        if matches!(element.name.as_str(), "If" | "For") {
            // 返回目标形状诊断。
            return Err(Diagnostic::new(
                element.span,
                "setStyle 的目标必须是声明事件的实际 View",
                "把事件与 setStyle 放到 If 或 For 内的 Button、Container 等实际元素上",
            ));
        }
        // 动态样式必须归属最近的 UIX Widget。
        let Some(widget_scope) = self.widget_scope_stack.last().cloned() else {
            // 返回组件边界诊断。
            return Err(Diagnostic::new(
                element.span,
                "setStyle 只能在 UIX Widget 内使用",
                "声明 Widget，并把事件 View 放入该 Widget 的直接或嵌套核心子树",
            ));
        };
        // 去重目标类并保持首次出现顺序。
        let mut unique_targets = Vec::<(String, SourceSpan)>::new();
        // 按源码顺序处理目标。
        for (name, span) in &targets {
            // 非空目标必须是单一类名。
            if !name.is_empty() && name.split_whitespace().count() != 1 {
                // 返回单类契约诊断。
                return Err(Diagnostic::new(
                    *span,
                    "setStyle 每次只能选择一个样式类",
                    "使用 setStyle('className')；空字符串用于恢复原始 class",
                ));
            }
            // 只保留首次出现的相同目标。
            if !unique_targets.iter().any(|(known, _)| known == name) {
                // 保存目标与精确调用跨度。
                unique_targets.push((name.clone(), *span));
            }
        }
        // 保留尚未改写的原始 class 与 inline style 供每个分支独立合并。
        let source = element.clone();
        // 先生成原始 class 分支。
        let mut expanded = source.clone();
        // 使用现有解析器完成继承、类优先级与内联覆盖。
        self.styles.apply(&mut expanded)?;
        // 取出原始分支样式，避免普通代码生成重复应用。
        let original_properties = take_inline_style(&mut expanded);
        // 取出显式 key，保证身份计算与 ViewNode::key 只求值一次。
        let key = take_dynamic_key(&mut expanded, bindings, self)?;
        // 为闭合枚举创建卫生名称。
        let enum_ident = self.fresh_ident("dynamic_style", &element.name);
        // 保存目标到逐调用点 setter 队列的确定性映射。
        let mut setters = BTreeMap::<String, VecDeque<String>>::new();
        // 为每个空字符串恢复调用创建独占 setter。
        let original_setters = allocate_setters(self, &targets, "", "original");
        // 登记空字符串恢复入口队列。
        setters.insert(String::new(), VecDeque::from(original_setters.clone()));
        // 建立原始分支元数据。
        let mut variants = vec![DynamicStyleVariant {
            // 原始分支使用固定枚举名称。
            variant_name: "Original".to_string(),
            // 保存全部恢复调用点的独占闭包名称。
            setter_names: original_setters,
            // 保存原始 class 与内联层合并结果。
            properties: original_properties,
        }];
        // 为每个非空目标生成闭合样式分支。
        for (index, (name, span)) in unique_targets
            .iter()
            .filter(|(name, _)| !name.is_empty())
            .enumerate()
        {
            // 克隆原始元素以保留 inline style 作者覆盖层。
            let mut target = source.clone();
            // 删除原始 class，动态层按替换语义选择单一目标类。
            target
                .attributes
                .retain(|attribute| attribute.name != "class");
            // 追加目标类交给现有类解析器验证与展开。
            target.attributes.push(Attribute {
                // 使用静态样式类属性入口。
                name: "class".to_string(),
                // 目标已验证为单一编译期字符串。
                value: AttributeValue::Literal(name.clone()),
                // 沿用精确 setStyle 调用跨度。
                span: *span,
            });
            // 合并目标类、继承与原始 inline style。
            self.styles.apply(&mut target)?;
            // 创建确定性的枚举分支名称。
            let variant_name = format!("Class{index}");
            // 为相同目标的每个调用点创建独占 setter。
            let target_setters = allocate_setters(self, &targets, name, name);
            // 登记目标到逐调用点 setter 队列。
            setters.insert(name.clone(), VecDeque::from(target_setters.clone()));
            // 保存完整类型化分支。
            variants.push(DynamicStyleVariant {
                // 保存枚举分支名。
                variant_name,
                // 保存每个调用点独占的闭包局部变量名。
                setter_names: target_setters,
                // 取出合并后的完整样式字段列表。
                properties: take_inline_style(&mut target),
            });
        }
        // 动态 setStyle 分支暂不允许隐式重启动画状态。
        if let Some(property) = variants
            // 遍历全部闭合样式分支。
            .iter()
            // 遍历每个分支的最终级联属性。
            .flat_map(|variant| variant.properties.iter())
            // 定位任一 animation 简写。
            .find(|property| property.name == "animation")
        {
            // 返回组合边界诊断，避免普通 Style 生成器误报规划中属性。
            return Err(Diagnostic::new(
                // 指向实际 animation 属性。
                property.span,
                // 陈述失败原因。
                "setStyle 动态分支暂不支持切换 animation 播放实例",
                // 给出当前可执行写法。
                "把 animation 放到不使用 setStyle 的静态节点；状态变化动画使用 transition",
            ));
        }
        // 把全部 setStyle 调用替换为已生成 setter 调用。
        for attribute in &mut expanded.attributes {
            // 只改写事件表达式。
            if let AttributeValue::Expression(node) = &mut attribute.value {
                // 递归替换当前事件中的调用目标与参数。
                lower_set_style_calls(&mut node.expression, &mut setters)?;
            }
        }
        // 使用节点类型与静态跨度形成跨构建稳定声明身份。
        let declaration_id = Self::stable_widget_id(&format!(
            "dynamic-style:{}:{}:{}",
            element.name, element.span.start, element.span.end
        ));
        // 动态样式元数据作为最终 View 装饰包裹事件表达式。
        expanded
            .widget_scopes
            .push(super::WidgetScopeMarker::DynamicStyle(
                // 保存闭合状态、身份与分支。
                DynamicStyleBinding {
                    // 保存最近组件作用域名称。
                    widget_scope_name: widget_scope.to_string(),
                    // 保存稳定子作用域声明标识。
                    declaration_id,
                    // 子作用域内只需要一个动态样式字段。
                    field_id: 0,
                    // 保存卫生枚举名称。
                    enum_name: enum_ident.to_string(),
                    // 继承最近 For 的实际实例路径。
                    instance_path_name: self.for_path_stack.last().map(ToString::to_string),
                    // 保存只求值一次的显式 key。
                    key,
                    // 保存闭合类型化分支。
                    variants,
                },
            ));
        // 写回完成动态样式降低的元素。
        *element = expanded;
        // 报告已经消费动态样式。
        Ok(true)
    }
}

// 为指定目标的每个源码调用点分配独占 setter 名称。
fn allocate_setters(
    // 接收展开器以生成卫生名称。
    expander: &mut WidgetExpander,
    // 接收保留重复项的源码顺序目标列表。
    targets: &[(String, SourceSpan)],
    // 接收需要匹配的目标类名。
    target: &str,
    // 接收卫生名称的可读片段。
    source_name: &str,
) -> Vec<String> {
    // 每个调用点独立分配，确保不同事件闭包不会移动同一闭包值。
    targets
        .iter()
        .filter(|(name, _)| name == target)
        .map(|_| expander.fresh_ident("set_style", source_name).to_string())
        .collect()
}

// 从元素取出静态解析器生成的合并 style。
fn take_inline_style(element: &mut Element) -> Vec<StyleProperty> {
    // 查找内部生成的唯一 style 属性。
    let Some(index) = element
        .attributes
        .iter()
        .position(|attribute| attribute.name == "style")
    else {
        // 没有任何样式时返回空分支。
        return Vec::new();
    };
    // 移除 style 以避免普通属性阶段再次应用。
    let attribute = element.attributes.remove(index);
    // 静态样式解析器保证形状为结构化属性列表。
    match attribute.value {
        // 返回完整合并结果。
        AttributeValue::InlineStyle(properties) => properties,
        // 其他形状表示内部阶段损坏。
        _ => unreachable!("样式类解析器只能生成 InlineStyle"),
    }
}

// 取出并改写动态节点的显式 key。
fn take_dynamic_key(
    // 接收已经完成静态样式展开的元素。
    element: &mut Element,
    // 接收组件字段绑定。
    bindings: &Bindings,
    // 接收展开器以复用表达式字段降低。
    expander: &mut WidgetExpander,
) -> Result<Option<DynamicStyleKey>, Diagnostic> {
    // 查找可选 key 属性。
    let Some(index) = element
        .attributes
        .iter()
        .position(|attribute| attribute.name == "key")
    else {
        // 无显式 key 时由 For 路径或静态位置提供身份。
        return Ok(None);
    };
    // 移除 key 以保证后续只求值一次。
    let attribute = element.attributes.remove(index);
    // 按属性值形状保存身份表达式。
    match attribute.value {
        // 字面量直接拥有字符串。
        AttributeValue::Literal(value) => Ok(Some(DynamicStyleKey::Literal(value))),
        // 表达式先完成组件字段读取改写。
        AttributeValue::Expression(mut node) => {
            // key 不允许状态副作用且读取普通值。
            expander.transform_expression(&mut node.expression, bindings, false, false)?;
            // 保存改写后的表达式。
            Ok(Some(DynamicStyleKey::Expression(node)))
        }
        // key 不接受样式内部值。
        _ => Err(Diagnostic::new(
            attribute.span,
            "动态样式节点的 key 必须是字符串或表达式",
            "使用 key=\"stable-id\" 或 key={stableId}",
        )),
    }
}

// 收集表达式中的 setStyle 字符串目标。
fn collect_set_style_targets(
    // 接收待检查表达式。
    expression: &Expression,
    // 接收源码顺序输出列表。
    output: &mut Vec<(String, SourceSpan)>,
) -> Result<(), Diagnostic> {
    // 直接 setStyle 调用由解析器保证单个字符串位置参数。
    if let ExpressionKind::Call { callee, arguments } = &expression.kind {
        // 只匹配框架内置名称。
        if matches!(&callee.kind, ExpressionKind::Identifier(name) if name == "setStyle") {
            // 防御性取得唯一参数。
            let Some(argument) = arguments.first() else {
                // 返回内部形状诊断。
                return Err(Diagnostic::new(
                    expression.span,
                    "setStyle 缺少样式类名称",
                    "使用 setStyle('className')",
                ));
            };
            // 参数必须保持解析期字符串字面量形状。
            let ExpressionKind::String(name) = &argument.value.kind else {
                // 返回编译期目标诊断。
                return Err(Diagnostic::new(
                    argument.span,
                    "setStyle 只接受编译期字符串字面量",
                    "使用 setStyle('className')",
                ));
            };
            // 保存目标与调用跨度。
            output.push((name.clone(), expression.span));
            // 当前调用参数无需继续递归。
            return Ok(());
        }
    }
    // 递归访问全部复合表达式。
    visit_expression_children(expression, &mut |child| {
        collect_set_style_targets(child, output)
    })
}

// 把 setStyle 调用原地替换为闭合 setter 调用。
fn lower_set_style_calls(
    // 接收待改写表达式。
    expression: &mut Expression,
    // 接收目标类到 setter 名称映射。
    setters: &mut BTreeMap<String, VecDeque<String>>,
) -> Result<(), Diagnostic> {
    // 直接调用可以原地替换目标与参数。
    if let ExpressionKind::Call { callee, arguments } = &mut expression.kind {
        // 检查框架内置名称。
        if matches!(&callee.kind, ExpressionKind::Identifier(name) if name == "setStyle") {
            // 提取解析期保证的字符串参数。
            let target = arguments
                .first()
                .and_then(|argument| match &argument.value.kind {
                    // 返回字符串借用。
                    ExpressionKind::String(value) => Some(value.as_str()),
                    // 其他形状由收集阶段诊断。
                    _ => None,
                })
                .expect("setStyle 参数已完成字符串验证");
            // 取得当前调用点独占的 setter 名称。
            let setter = setters
                .get_mut(target)
                .and_then(VecDeque::pop_front)
                .expect("每个 setStyle 调用点均已登记独占 setter");
            // 把调用目标替换为卫生闭包。
            callee.kind = ExpressionKind::Identifier(setter);
            // setter 已闭合目标枚举，不再接收字符串参数。
            arguments.clear();
            // 当前调用已经完成降低。
            return Ok(());
        }
    }
    // 递归改写全部子表达式。
    visit_expression_children_mut(expression, &mut |child| {
        lower_set_style_calls(child, setters)
    })
}

// 只读访问表达式的全部直接子节点。
fn visit_expression_children(
    // 接收父表达式。
    expression: &Expression,
    // 接收可能返回诊断的访问器。
    visitor: &mut impl FnMut(&Expression) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    // 按表达式形状访问直接子节点。
    match &expression.kind {
        // action 独立语句块按源码顺序暴露全部直接表达式根。
        ExpressionKind::LoweredAction(action) => {
            super::action_semantic::visit_action_block_expressions(
                &action.block,
                &mut |expression| visitor(expression),
            )?;
        }
        ExpressionKind::Unary { operand, .. } => visitor(operand)?,
        ExpressionKind::Binary { left, right, .. } => {
            visitor(left)?;
            visitor(right)?;
        }
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            visitor(condition)?;
            visitor(then_branch)?;
            visitor(else_branch)?;
        }
        ExpressionKind::Member { object, .. } => visitor(object)?,
        ExpressionKind::Index { object, index } => {
            visitor(object)?;
            visitor(index)?;
        }
        ExpressionKind::Call { callee, arguments } => {
            visitor(callee)?;
            for argument in arguments {
                visitor(&argument.value)?;
            }
        }
        ExpressionKind::Object(fields) => {
            for field in fields {
                visitor(&field.value)?;
            }
        }
        ExpressionKind::Array(items) => {
            for item in items {
                visitor(item)?;
            }
        }
        // 受限闭包只包含一个直接表达式子节点。
        ExpressionKind::Closure { body, .. } => visitor(body)?,
        ExpressionKind::Identifier(_)
        | ExpressionKind::Number(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Boolean(_) => {}
    }
    // 报告访问完成。
    Ok(())
}

// 可变访问表达式的全部直接子节点。
fn visit_expression_children_mut(
    // 接收父表达式。
    expression: &mut Expression,
    // 接收可能返回诊断的访问器。
    visitor: &mut impl FnMut(&mut Expression) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    // 按表达式形状访问直接子节点。
    match &mut expression.kind {
        // action 语句块中的调用按源码顺序参与同一 setter 队列。
        ExpressionKind::LoweredAction(action) => {
            super::action_semantic::visit_action_block_expressions_mut(
                &mut action.block,
                &mut |expression| visitor(expression),
            )?;
        }
        ExpressionKind::Unary { operand, .. } => visitor(operand)?,
        ExpressionKind::Binary { left, right, .. } => {
            visitor(left)?;
            visitor(right)?;
        }
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            visitor(condition)?;
            visitor(then_branch)?;
            visitor(else_branch)?;
        }
        ExpressionKind::Member { object, .. } => visitor(object)?,
        ExpressionKind::Index { object, index } => {
            visitor(object)?;
            visitor(index)?;
        }
        ExpressionKind::Call { callee, arguments } => {
            visitor(callee)?;
            for argument in arguments {
                visitor(&mut argument.value)?;
            }
        }
        ExpressionKind::Object(fields) => {
            for field in fields {
                visitor(&mut field.value)?;
            }
        }
        ExpressionKind::Array(items) => {
            for item in items {
                visitor(item)?;
            }
        }
        // 受限闭包只包含一个直接可变表达式子节点。
        ExpressionKind::Closure { body, .. } => visitor(body)?,
        ExpressionKind::Identifier(_)
        | ExpressionKind::Number(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Boolean(_) => {}
    }
    // 报告访问完成。
    Ok(())
}
