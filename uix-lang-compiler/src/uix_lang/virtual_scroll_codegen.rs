// 引入卫生局部标识符与生成令牌类型。
use proc_macro2::{Ident, Span, TokenStream};
// 引入 Rust 令牌拼接宏。
use quote::quote;

// 引入共享 View 属性、单节点生成与可渲染判断。
use super::codegen::{apply_common_attributes, generate_node_view, is_renderable_node};
// 引入 VirtualScroll 映射所需的语言 AST、值生成与诊断类型。
use super::{
    Attribute, AttributeValue, ControlBinding, Diagnostic, Element, Expression, ExpressionKind,
    ExpressionNode, Node, generate_expression, generate_expression_without_source_marker,
    numeric_value, rust_identifier,
};

// 生成文档定义的定高 VirtualScroll 数据窗口。
pub(crate) fn generate_virtual_scroll(element: &Element) -> Result<TokenStream, Diagnostic> {
    // data 是决定物化总数与行值的唯一数据源。
    let data_attribute = required_attribute(element, "data")?;
    // data 必须是可由调用方类型检查的数组表达式。
    let data = expression_attribute(data_attribute, "VirtualScroll data")?;
    // rowHeight 是运行时固定行高算法的必要输入。
    let row_height_attribute = required_attribute(element, "rowHeight")?;
    // 复用共享长度与受限数值表达式解析。
    let row_height = numeric_value(row_height_attribute)?;
    // VirtualScroll 只接受一个直接 For 作为惰性行模板。
    let template = virtual_template(element)?;
    // 解析器保证 For 标签携带对应控制绑定。
    let Some(ControlBinding::For {
        // 借用当前行绑定名称。
        binding,
        // 借用当前行绑定位置。
        binding_span,
        // 借用可选索引绑定。
        index_binding,
        // 借用可选索引绑定位置。
        index_span,
        // 借用 For 声明的数据源。
        iterable,
        // 借用可选稳定 key。
        key,
    }) = template.control.as_ref()
    else {
        // 防御解析后 AST 被错误构造的内部形状。
        return Err(Diagnostic::new(
            // 指向完整 For 模板。
            template.span,
            // 说明缺少控制绑定。
            "VirtualScroll 的 <For> 缺少循环绑定",
            // 引导重新使用规范语法。
            "使用 <For {item} in {data}>...</For>",
        ));
    };
    // 先把 data 转换为最终 Rust 令牌，后续直接复用以保证只求值一次。
    let data_tokens = generate_expression(&data.expression, None)?;
    // 生成不含来源标记的 data 规范令牌用于结构比较。
    let data_comparison = generate_expression_without_source_marker(&data.expression, None)?;
    // 把 For in 转换为同一规范令牌形式以忽略源码位置与无意义空白。
    let iterable_tokens = generate_expression_without_source_marker(&iterable.expression, None)?;
    // data 与 For in 必须描述同一规范表达式，避免双数据源漂移。
    if data_comparison.to_string() != iterable_tokens.to_string() {
        // 返回指向 For 数据源的冲突诊断。
        return Err(Diagnostic::new(
            // 指向冲突的 in 表达式。
            iterable.span,
            // 说明两个声明必须一致。
            "VirtualScroll data 与直接 <For> 的 in 数据源不一致",
            // 给出统一数据源写法。
            "让 data={items} 与 <For ... in {items}> 使用同一表达式",
        ));
    }
    // 一致性验证后使用包含可选来源标记的最终 data 令牌。
    let data = data_tokens;
    // 可选 item 只作为对 For 行变量的显式一致性断言。
    validate_item_attribute(element, binding)?;
    // renderer 每次只能返回一个 View 行根。
    let row = single_row_view(template)?;
    // 递归生成当前行的公开 ViewNode。
    let row = generate_node_view(row)?;
    // 在把绑定名转换成 Rust Ident 前验证并生成可选业务 key。
    let key = key
        // 借用 key 表达式。
        .as_ref()
        // VirtualScroll 键工厂只能依赖当前行与索引，不能读取捕获外的响应式状态。
        .map(|value| {
            // 用源 AST 中的字符串绑定名验证自由变量与调用边界。
            validate_virtual_key_expression(
                // 传入受限业务 key 表达式。
                &value.expression,
                // 传入当前 For 行变量名。
                binding,
                // 传入可选的当前 For 索引变量名。
                index_binding.as_deref(),
            )?;
            // 返回已经通过纯身份约束的表达式令牌。
            generate_expression(&value.expression, None)
        })
        // 把 Option<Result> 转换为 Result<Option>。
        .transpose()?;
    // 生成调用方可见的行绑定标识符。
    let binding = rust_identifier(binding, *binding_span)?;
    // 生成可选的调用方索引绑定标识符。
    let index_binding = index_binding
        // 验证存在的索引名称。
        .as_deref()
        // 转换为带诊断的 Rust 标识符。
        .map(|name| rust_identifier(name, index_span.unwrap_or(*binding_span)))
        // 把 Option<Result> 转换为 Result<Option>。
        .transpose()?;
    // 使用混合卫生名称保存拥有所有权的数据快照。
    let data_snapshot = Ident::new("__uix_virtual_data", Span::mixed_site());
    // 使用混合卫生名称保存一次性计算的项目总数。
    let item_count = Ident::new("__uix_virtual_count", Span::mixed_site());
    // 使用混合卫生名称接收运行时请求的绝对索引。
    let item_index = Ident::new("__uix_virtual_index", Span::mixed_site());
    // 可选索引绑定直接复制运行时绝对索引。
    let index_statement = index_binding.map(|index_binding| {
        // 返回用户索引局部变量声明。
        quote! { let #index_binding = #item_index; }
    });
    // 为业务键闭包保存独立数据快照，避免两个 static 闭包争用所有权。
    let key_data_snapshot = Ident::new("__uix_virtual_key_data", Span::mixed_site());
    // 根据 For 是否声明 key 生成独立快照准备语句与一致身份 renderer。
    let (key_snapshot_setup, renderer) = if let Some(key) = key {
        // 带 key 时必须在行 View 构建前计算业务身份并交给运行时捕获。
        (
            // 复制已经求值的数据快照供业务键闭包独立拥有。
            quote! { let #key_data_snapshot = ::std::sync::Arc::clone(&#data_snapshot); },
            // 用同一业务键同时拥有 keyed reconcile 与组件私有状态命名空间。
            quote! {
                .render_keyed(
                    // 先按绝对索引计算当前业务项的稳定键。
                    move |#item_index| {
                        // 克隆当前项以复用 For 的行绑定与 key 表达式语义。
                        let #binding = (#key_data_snapshot)[#item_index].clone();
                        // 建立可选绝对索引绑定供 key 表达式消费。
                        #index_statement
                    // 返回受限业务表达式，由运行时统一执行一次字符串规范化。
                    #key
                    },
                    // 只在运行时请求的索引进入窗口时构建实际行 View。
                    move |#item_index| {
                        // 克隆当前行值，避免事件闭包借用数据快照。
                        let #binding = (#data_snapshot)[#item_index].clone();
                        // 建立可选绝对索引绑定。
                        #index_statement
                        // 返回唯一行根 View。
                        #row
                    },
                )
            },
        )
    } else {
        // 未声明业务 key 时由普通 render 使用绝对索引统一拥有行身份。
        (
            // 普通索引身份只需要 renderer 已拥有的数据快照。
            quote! {},
            // 生成普通绝对索引 renderer 调用。
            quote! {
                .render(move |#item_index| {
                    // 克隆当前行值，避免事件闭包借用数据快照。
                    let #binding = (#data_snapshot)[#item_index].clone();
                    // 建立可选绝对索引绑定。
                    #index_statement
                    // 返回唯一行根 View。
                    #row
                })
            },
        )
    };
    // 生成只声明数据、行高和 renderer 的公开运行时组合。
    let base = quote! {{
        // 把一次求值的数据快照放入共享所有权，避免 keyed 双闭包深拷贝大列表。
        let #data_snapshot = ::std::sync::Arc::new((#data).clone());
        // 在快照移动进 renderer 前计算稳定项目总数。
        let #item_count = (#data_snapshot).len();
        // 带业务键时为键闭包准备同一已求值数据的独立所有权快照。
        #key_snapshot_setup
        // 运行时 VirtualScroll 继续唯一拥有滚动与物化状态。
        ::uix::prelude::VirtualScroll::new()
            // 声明本次数据快照的项目总数。
            .item_count(#item_count)
            // 声明固定行高。
            .item_height(#row_height)
            // 按是否存在业务 key 选择一致身份的延迟 renderer。
            #renderer
    }};
    // 专有结构属性消费后，公共样式继续走统一 View 契约。
    apply_common_attributes(base, &element.attributes, &["data", "rowHeight", "item"])
}

// 验证 VirtualScroll 业务 key 只依赖当前项、可选索引与纯组合表达式。
fn validate_virtual_key_expression(
    // 接收解析后的受限 key 表达式。
    expression: &Expression,
    // 接收当前 For 行绑定名称。
    binding: &str,
    // 接收可选绝对索引绑定名称。
    index_binding: Option<&str>,
    // 返回纯身份表达式通过或定位诊断。
) -> Result<(), Diagnostic> {
    // 递归检查表达式树中的所有自由变量和调用节点。
    match &expression.kind {
        // action 语句块不属于 VirtualScroll 稳定 key 子语言。
        ExpressionKind::LoweredAction(_) => Err(Diagnostic::new(
            expression.span,
            "VirtualScroll 的 For key 不能调用 action",
            "使用 key={item.id} 或 item/index 的纯成员与算术表达式",
        )),
        // 当前行绑定与显式索引绑定是唯一允许的标识符根。
        ExpressionKind::Identifier(name)
            if name == binding || index_binding.is_some_and(|index| index == name) =>
        {
            // 合法局部标识符不需要进一步检查。
            Ok(())
        }
        // 其他标识符会在两个 move 闭包间产生所有权或响应式语义分裂。
        ExpressionKind::Identifier(_) => Err(Diagnostic::new(
            // 指向实际外部引用位置。
            expression.span,
            // 说明稳定键不能从 renderer 外部捕获值。
            "VirtualScroll 的 For key 只能依赖当前 item 或 index",
            // 引导把稳定 id 放入当前业务项。
            "使用 key={item.id}，不要在 key 中引用外部状态",
        )),
        // 数字、字符串和布尔字面量本身没有捕获或副作用。
        ExpressionKind::Number(_) | ExpressionKind::String(_) | ExpressionKind::Boolean(_) => {
            // 保留字面量供更高层纯组合表达式消费。
            Ok(())
        }
        // 一元表达式只需验证其操作数。
        ExpressionKind::Unary { operand, .. } => {
            // 递归沿用同一局部绑定白名单。
            validate_virtual_key_expression(operand, binding, index_binding)
        }
        // 二元表达式要求左右两侧都保持纯局部依赖。
        ExpressionKind::Binary { left, right, .. } => {
            // 先验证左侧，再验证右侧并返回首个诊断。
            validate_virtual_key_expression(left, binding, index_binding)?;
            // 验证右侧局部依赖。
            validate_virtual_key_expression(right, binding, index_binding)
        }
        // 三元表达式的条件与两条分支都必须是纯局部表达式。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 验证条件不会读取外部状态。
            validate_virtual_key_expression(condition, binding, index_binding)?;
            // 验证真分支不会读取外部状态。
            validate_virtual_key_expression(then_branch, binding, index_binding)?;
            // 验证假分支并返回最终结果。
            validate_virtual_key_expression(else_branch, binding, index_binding)
        }
        // 成员访问只沿对象根判断是否来自当前项。
        ExpressionKind::Member { object, .. } => {
            // 成员名不是自由变量，只验证对象表达式。
            validate_virtual_key_expression(object, binding, index_binding)
        }
        // 下标访问要求对象与索引两侧都保持局部纯依赖。
        ExpressionKind::Index { object, index } => {
            // 验证被索引对象归属当前项。
            validate_virtual_key_expression(object, binding, index_binding)?;
            // 验证索引表达式归属当前项或显式 index。
            validate_virtual_key_expression(index, binding, index_binding)
        }
        // 调用、数组与对象字面量可能产生副作用或隐藏外部依赖，不属于稳定身份子语言。
        ExpressionKind::Call { .. }
        | ExpressionKind::Object(_)
        | ExpressionKind::Array(_)
        | ExpressionKind::Closure { .. } => {
            // 返回不允许的复杂结构诊断。
            Err(Diagnostic::new(
                // 指向不允许的复杂结构。
                expression.span,
                // 说明 key 工厂必须保持无副作用。
                "VirtualScroll 的 For key 不能包含调用、数组、对象字面量或闭包",
                // 引导使用当前项的稳定字段或纯组合。
                "使用 key={item.id} 或 item/index 的纯成员与算术表达式",
            ))
        }
    }
}

// 查找必需结构属性并生成定位诊断。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "VirtualScroll",
        "使用 <VirtualScroll data={items} rowHeight=\"32px\"><For {item} in {items}>...</For></VirtualScroll>",
    )
}

// 提取必须使用花括号声明的表达式属性。
fn expression_attribute<'a>(
    // 接收待验证属性。
    attribute: &'a Attribute,
    // 接收诊断中的属性用途。
    label: &str,
) -> Result<&'a ExpressionNode, Diagnostic> {
    // 只接受已经通过受限语法解析的表达式。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 字符串和内联样式都不能充当数据源。
        return Err(Diagnostic::new(
            // 指向非法属性值。
            attribute.span,
            // 说明表达式形状要求。
            format!("{label} 必须使用花括号表达式"),
            // 给出最小合法写法。
            "使用 data={items}",
        ));
    };
    // 返回已验证表达式节点。
    Ok(expression)
}

// 提取唯一直接 For 行模板。
fn virtual_template(element: &Element) -> Result<&Element, Diagnostic> {
    // 收集排除排版空白后的直接子节点。
    let children = element
        // 遍历源顺序子节点。
        .children
        // 借用每个节点。
        .iter()
        // 排除纯空白文本。
        .filter(|child| is_renderable_node(child))
        // 收集用于数量和形状验证。
        .collect::<Vec<_>>();
    // VirtualScroll 必须恰好声明一个直接模板。
    if children.len() != 1 {
        // 返回父子形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 VirtualScroll。
            element.span,
            // 说明唯一模板要求。
            "<VirtualScroll> 必须恰好包含一个直接 <For> 行模板",
            // 给出规范形状。
            "用一个 <For {item} in {items}>...</For> 包裹行内容",
        ));
    }
    // 唯一子节点必须是元素。
    let Node::Element(template) = children[0] else {
        // 文本或插值不能描述惰性行模板。
        return Err(Diagnostic::new(
            // 指向非元素子节点。
            element.span,
            // 说明需要 For。
            "VirtualScroll 的直接子节点必须是 <For>",
            // 给出规范形状。
            "使用 <For {item} in {items}>...</For>",
        ));
    };
    // 其他元素不能被宏静默解释为全量列表。
    if template.name != "For" {
        // 返回具体标签诊断。
        return Err(Diagnostic::new(
            // 指向错误直接子元素。
            template.span,
            // 保留实际标签名称。
            format!("VirtualScroll 的直接子节点不能是 <{}>", template.name),
            // 给出所需模板标签。
            "改用直接 <For> 声明惰性行模板",
        ));
    }
    // 返回唯一 For 模板。
    Ok(template)
}

// 验证可选 item 与 For 行绑定一致。
fn validate_item_attribute(element: &Element, binding: &str) -> Result<(), Diagnostic> {
    // item 缺省时由 For 绑定承担唯一权威。
    let Some(attribute) = element
        // 借用属性列表。
        .attributes
        // 逐项遍历。
        .iter()
        // 查找可选 item。
        .find(|attribute| attribute.name == "item")
    else {
        // 没有冗余声明时直接通过。
        return Ok(());
    };
    // 兼容字符串标识符与文档登记的表达式标识符。
    let item = match &attribute.value {
        // 字符串形式直接读取变量名。
        AttributeValue::Literal(value) => value.as_str(),
        // 表达式形式只允许单一标识符。
        AttributeValue::Expression(expression) => {
            // 提取标识符名称。
            let ExpressionKind::Identifier(value) = &expression.expression.kind else {
                // 复合表达式不能声明变量名。
                return Err(Diagnostic::new(
                    // 指向非法 item。
                    attribute.span,
                    // 说明 item 只作变量一致性声明。
                    "VirtualScroll item 只能是行变量标识符",
                    // 给出与 For 一致的示例。
                    "使用 item={row}，并让 <For {row} ...> 使用同名绑定",
                ));
            };
            // 返回表达式中的标识符。
            value.as_str()
        }
        // 内联样式不能声明变量名。
        AttributeValue::InlineStyle(_) => {
            // 返回属性类型诊断。
            return Err(Diagnostic::new(
                // 指向非法 item。
                attribute.span,
                // 说明 item 形状。
                "VirtualScroll item 不能使用内联样式",
                // 给出合法表达式形式。
                "使用 item={row}",
            ));
        }
    };
    // 两个绑定源必须完全一致。
    if item != binding {
        // 返回冲突诊断。
        return Err(Diagnostic::new(
            // 指向冗余 item 声明。
            attribute.span,
            // 保留两个冲突名称。
            format!("VirtualScroll item={item:?} 与 <For> 行变量 {binding:?} 不一致"),
            // 引导删除冗余声明或统一名称。
            "删除 item 属性，或让 item 与 <For> 使用同一行变量名",
        ));
    }
    // 一致性断言通过。
    Ok(())
}

// 提取 renderer 唯一的直接行根 View。
fn single_row_view(template: &Element) -> Result<&Node, Diagnostic> {
    // 收集排除排版空白后的行模板子节点。
    let rows = template
        // 遍历 For 内部节点。
        .children
        // 借用每个节点。
        .iter()
        // 排除纯空白文本。
        .filter(|child| is_renderable_node(child))
        // 收集用于数量验证。
        .collect::<Vec<_>>();
    // renderer 必须返回恰好一个 View。
    if rows.len() != 1 {
        // 返回行根形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 For。
            template.span,
            // 说明唯一行根要求。
            "VirtualScroll 的 <For> 必须恰好生成一个直接行根 View",
            // 引导显式选择行内布局。
            "用 Container、Column、Row 或 Grid 包裹多个行内节点",
        ));
    }
    // 控制元素不能直接充当 renderer 返回值。
    if matches!(rows[0], Node::Element(element) if matches!(element.name.as_str(), "If" | "For")) {
        // 返回控制流形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 For 模板。
            template.span,
            // 说明行根必须是稳定 View。
            "VirtualScroll 行根不能是 If 或嵌套 For",
            // 引导把控制流放进稳定容器。
            "用 Container 包裹行内控制内容",
        ));
    }
    // 返回唯一行根节点。
    Ok(rows[0])
}
