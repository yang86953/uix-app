// 引入卫生局部标识符与生成令牌类型。
use proc_macro2::{Ident, Span, TokenStream};
// 引入 Rust 令牌拼接宏。
use quote::quote;

// 引入共享 View 属性、单节点生成与可渲染判断。
use super::codegen::{apply_common_attributes, generate_node_view, is_renderable_node};
// 引入 VirtualScroll 映射所需的语言 AST、值生成与诊断类型。
use super::{
    Attribute, AttributeValue, ControlBinding, Diagnostic, Element, ExpressionKind, ExpressionNode,
    Node, generate_expression, numeric_value, rust_identifier,
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
    // 先把 data 转换为稳定 Rust 令牌，后续直接复用以保证只求值一次。
    let data = generate_expression(&data.expression, None)?;
    // 把 For in 转换为同一规范令牌形式以忽略源码位置与无意义空白。
    let iterable_tokens = generate_expression(&iterable.expression, None)?;
    // data 与 For in 必须描述同一规范表达式，避免双数据源漂移。
    if data.to_string() != iterable_tokens.to_string() {
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
    // 可选 item 只作为对 For 行变量的显式一致性断言。
    validate_item_attribute(element, binding)?;
    // renderer 每次只能返回一个 View 行根。
    let row = single_row_view(template)?;
    // 递归生成当前行的公开 ViewNode。
    let row = generate_node_view(row)?;
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
    // 可选 key 在行绑定与索引绑定建立后生成。
    let key = key
        // 借用 key 表达式。
        .as_ref()
        // 生成 Rust key 令牌。
        .map(|value| generate_expression(&value.expression, None))
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
    // 带 key 的行根复用公开 ViewNode 身份契约。
    let row = if let Some(key) = key {
        // 生成带稳定字符串 key 的行 View。
        quote! {
            // 先构建当前行根节点。
            let __uix_virtual_view = #row;
            // 把业务 key 转换为公开字符串身份。
            __uix_virtual_view.key(::std::format!("{}", #key))
        }
    } else {
        // 未声明 key 时保留运行时按绝对索引生成的后备身份。
        row
    };
    // 生成只声明数据、行高和 renderer 的公开运行时组合。
    let base = quote! {{
        // 克隆数据快照以满足 renderer 的静态生命周期。
        let #data_snapshot = (#data).clone();
        // 在快照移动进 renderer 前计算稳定项目总数。
        let #item_count = (#data_snapshot).len();
        // 运行时 VirtualScroll 继续唯一拥有滚动与物化状态。
        ::uix::prelude::VirtualScroll::new()
            // 声明本次数据快照的项目总数。
            .item_count(#item_count)
            // 声明固定行高。
            .item_height(#row_height)
            // 只在运行时请求的索引进入物化窗口时构建行 View。
            .render(move |#item_index| {
                // 克隆当前行值，避免事件闭包借用数据快照。
                let #binding = (#data_snapshot)[#item_index].clone();
                // 建立可选绝对索引绑定。
                #index_statement
                // 返回唯一行根 View。
                #row
            })
    }};
    // 专有结构属性消费后，公共样式继续走统一 View 契约。
    apply_common_attributes(base, &element.attributes, &["data", "rowHeight", "item"])
}

// 查找必需结构属性并生成定位诊断。
fn required_attribute<'a>(
    // 接收 VirtualScroll 元素。
    element: &'a Element,
    // 接收必需属性名称。
    name: &str,
) -> Result<&'a Attribute, Diagnostic> {
    // 在源顺序属性中查找目标名称。
    element
        // 借用属性列表。
        .attributes
        // 逐项遍历。
        .iter()
        // 匹配精确属性名。
        .find(|attribute| attribute.name == name)
        // 缺失时构造完整元素诊断。
        .ok_or_else(|| {
            // 返回必需属性诊断。
            Diagnostic::new(
                // 指向完整 VirtualScroll。
                element.span,
                // 说明缺失的具体属性。
                format!("<VirtualScroll> 缺少必需的 {name} 属性"),
                // 给出文档规范的最小结构。
                "使用 <VirtualScroll data={items} rowHeight=\"32px\"><For {item} in {items}>...</For></VirtualScroll>",
            )
        })
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
