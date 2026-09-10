// 引入过程宏令牌与卫生标识符。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;
// 引入 Compiler System 的稳定源码身份。
use crate::source_graph::SourceId;
// 引入解析后的核心语法树与诊断类型。
use super::{
    Attribute, ControlBinding, Diagnostic, Element, ExpressionNode, Node, SOURCE_ID_ATTRIBUTE,
    SourceSpan, WidgetScopeMarker, mark_source_tokens, with_source_marker_id,
};
// 引入独立元素分派入口。
use super::element_codegen::generate_element;
// 引入动态 Text 插值的延迟求值生成入口。
use super::dynamic_text_codegen::{generate_dynamic_label, has_interpolation};
// 引入通用事件属性生成入口。
use super::event_codegen::apply_event;
// 引入相邻条件链生成入口。
use super::conditional_chain_codegen::generate_conditional_chain;
// 引入 For 实际实例路径内部标识符读取。
use super::for_identity_codegen::{internal_control_ident, optional_internal_control_ident};
// 引入最终 ViewNode 的组件状态装饰应用。
use super::view_decoration_codegen::apply_widget_scopes;
// 引入受限表达式与事件处理器生成入口。
use super::generate_expression;
// 引入 Rust 基础节点列表到 UIX 容器的无克隆追加边界。
use super::generate_kernel_children;
// 引入属性值与绑定名称的共享生成入口。
use super::{
    align_value, apply_inline_style, boolean_value, deferred_style_diagnostic, justify_value,
    literal_string, numeric_value, rust_identifier, string_value, typography_value,
};
// 拆分 For 逐实例准备语句恢复，保持核心代码生成器规模受控。
mod for_setup;
// 引入只解析编译器内部令牌文本的恢复入口；对 VirtualScroll 映射重导出。
pub(super) use for_setup::parse_for_iteration_setup;
// 引入 reactive 组件作用域准备语句恢复入口。
use for_setup::parse_reactive_setup;
// 生成一个可直接消费的公开 UIX View 表达式。
pub(crate) fn generate_view(element: &Element) -> Result<TokenStream, Diagnostic> {
    let source_id = element
        .attributes
        .iter()
        .find(|attribute| attribute.name == SOURCE_ID_ATTRIBUTE)
        .and_then(|attribute| match &attribute.value {
            super::AttributeValue::Literal(value) => {
                value.parse::<u64>().ok().map(SourceId::from_value)
            }
            super::AttributeValue::Expression(_) | super::AttributeValue::InlineStyle(_) => None,
        });
    with_source_marker_id(source_id, || generate_view_for_source(element))
        .map_err(|diagnostic| diagnostic.at_source_if_missing(source_id))
}

fn generate_view_for_source(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 控制节点只能在父元素的有序子节点列表中展开。
    if matches!(
        element.name.as_str(),
        "If" | "ElseIf" | "Else" | "For" | "KernelChildren"
    ) {
        // 返回根控制节点形状诊断。
        return Err(Diagnostic::new(
            // 指向完整控制元素。
            element.span,
            // 说明单根 View 要求。
            format!("<{}> 不能作为独立 View 根节点生成", element.name),
            // 给出父容器修复建议。
            "把该控制元素放入 Container、Row 或 Column 内",
        ));
    }
    // 普通元素委托映射矩阵生成。
    let mut generated_element = element.clone();
    generated_element
        .attributes
        .retain(|attribute| attribute.name != SOURCE_ID_ATTRIBUTE);
    let view = generate_element(&generated_element)?;
    // 通过公开 View trait 统一物化为 ViewNode。
    let view = quote! { ::uix_app::prelude::View::build(#view) };
    // 把所有嵌套组件的私有状态作用域依次附加到同一个实际根节点。
    let view = apply_widget_scopes(view, &element.widget_scopes)?;
    // reactive 组件根包进独立捕获帧的 scoped 闭包：收归的准备语句在
    // 闭包内重建时重跑，组件体读取的 State 只登记为本子树结构依赖。
    let view = if let Some(reactive_setup) = &element.reactive_setup {
        // 恢复本子树作用域的准备语句。
        let setup = parse_reactive_setup(reactive_setup, element.span)?;
        // 复用公开 scoped 组合器建立子树作用域。
        quote! {
            // 先重建 props、状态与回调适配器，再构建只含核心元素的 View。
            ::uix_app::prelude::scoped(move || { #(#setup)* #view })
        }
    } else {
        // 普通根保持既有表达式。
        view
    };
    Ok(mark_source_tokens(view, element.span))
}

// 生成文本元素。
pub(super) fn generate_text(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Text 插值由 DynamicLabel 在测量、绘制和语义读取时延迟求值。
    let ellipsis = element
        .attributes
        .iter()
        .find(|a| a.name == "ellipsis")
        .map(boolean_value)
        .transpose()?;
    if element.name == "Text" && (has_interpolation(&element.children) || ellipsis.is_some()) {
        // 生成拥有全部表达式捕获的动态文本 View。
        let base = generate_dynamic_label(&element.children, element.span, ellipsis)?;
        // 公共样式、字号、身份与事件继续走统一 View 装饰链。
        return apply_common_attributes(base, &element.attributes, &["ellipsis"]);
    }
    // 把有序文本与插值组合成单一内容表达式。
    let content = generate_text_content(&element.children, element.span)?;
    // Label 专属 selectable 在物化 ViewNode 前配置底层组件。
    if element.name == "Label" {
        // 查找可选的文本选择布尔属性。
        if let Some(attribute) = element
            // 遍历当前元素属性。
            .attributes
            // 借用属性迭代器。
            .iter()
            // 只匹配 Label 专属 selectable。
            .find(|attribute| attribute.name == "selectable")
        {
            // 接受布尔简写、字面量或受限表达式。
            let selectable = boolean_value(attribute)?;
            // 使用公开 Label 构造器在布尔两支中保持同一组件类型。
            let base = quote! {
                ::uix_app::prelude::ViewNode::leaf({
                    // 构造既有 Label 组件。
                    let __uix_label = ::uix_app::prelude::Label::new(#content);
                    // 为真时启用既有选择语义，否则保留默认不可选。
                    if #selectable { __uix_label.selectable() } else { __uix_label }
                })
            };
            // 消费 selectable 后继续应用文本公共样式与事件。
            return apply_common_attributes(base, &element.attributes, &["selectable"]);
        }
    }
    // Text 的字面量与插值共享折行契约；Label 保持既有不折行标签语义。
    let base = if element.name == "Text" {
        quote! { ::uix_app::prelude::View::build(::uix_app::prelude::Label::new(#content).word_wrap(true)) }
    } else {
        quote! { ::uix_app::prelude::label(#content) }
    };
    // 应用文本支持的公共属性与事件。
    apply_common_attributes(base, &element.attributes, &[])
}

// 生成按钮元素。
pub(super) fn generate_button(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 普通按钮不声明 ButtonGroup 连体位置。
    generate_button_with_group_position(element, None)
}

// 生成按钮，并在公共样式物化前应用可选 ButtonGroup 连体位置。
pub(super) fn generate_button_with_group_position(
    // 接收 Button 元素。
    element: &Element,
    // 接收可选的公开 ButtonGroupPosition 表达式。
    group_position: Option<TokenStream>,
) -> Result<TokenStream, Diagnostic> {
    // 按钮当前公开 API 只接收文本内容。
    let content = generate_text_content(&element.children, element.span)?;
    // 构造公开按钮构建器。
    let mut view = quote! { ::uix_app::prelude::button(#content) };
    // 先处理必须在 StyleExt 物化前调用的按钮专有属性。
    for attribute in &element.attributes {
        // 按钮类型选择公开变体方法。
        if attribute.name == "type" {
            // 要求类型为编译期字面量。
            let kind = literal_string(attribute, "Button type")?;
            // 按登记类型应用构建器变体。
            view = match kind.as_str() {
                // 默认类型不增加链式调用。
                "default" => view,
                // 主按钮调用 primary。
                "primary" => quote! { (#view).primary() },
                // 幽灵按钮调用 ghost。
                "ghost" => quote! { (#view).ghost() },
                // 危险按钮调用 danger。
                "danger" => quote! { (#view).danger() },
                // 未登记类型返回编译期诊断。
                _ => {
                    // 返回按钮类型映射诊断。
                    return Err(Diagnostic::new(
                        // 指向完整属性。
                        attribute.span,
                        // 说明未知类型。
                        format!("Button type={kind:?} 尚无公开构建器映射"),
                        // 给出已登记类型集合。
                        "使用 default、primary、ghost 或 danger",
                    ));
                }
            };
        }
        // 禁用状态映射到按钮构建器。
        if attribute.name == "disabled" {
            // 生成布尔属性表达式。
            let value = boolean_value(attribute)?;
            // 应用禁用状态。
            view = quote! { (#view).disabled(#value) };
        }
        // 块级按钮状态映射到按钮构建器。
        if attribute.name == "block" {
            // 生成布尔属性表达式。
            let value = boolean_value(attribute)?;
            // 应用块级状态。
            view = quote! { (#view).block(#value) };
        }
        // 标准尺寸关键字映射到公开 ControlSize。
        if attribute.name == "size" {
            // 尺寸必须在编译期确定以拒绝未登记关键字。
            let size = literal_string(attribute, "Button size")?;
            // 选择公开尺寸枚举变体。
            let size = match size.as_str() {
                // 小尺寸映射到 Small。
                "small" => quote! { ::uix_app::prelude::ControlSize::Small },
                // 文档默认中尺寸映射到 Medium。
                "middle" => quote! { ::uix_app::prelude::ControlSize::Medium },
                // 大尺寸映射到 Large。
                "large" => quote! { ::uix_app::prelude::ControlSize::Large },
                // 未登记关键字返回定向诊断。
                _ => {
                    // 指出实际非法尺寸与合法集合。
                    return Err(Diagnostic::new(
                        // 指向完整 size 属性。
                        attribute.span,
                        // 说明非法值。
                        format!("Button size={size:?} 尚无公开尺寸映射"),
                        // 给出文档登记的三个关键字。
                        "使用 small、middle 或 large",
                    ));
                }
            };
            // 在 View 物化前应用 ButtonBuilder 尺寸。
            view = quote! { (#view).size(#size) };
        }
        // 加载态映射到公开 ButtonBuilder 生命周期入口。
        if attribute.name == "loading" {
            // 接受布尔简写、字面量或受限表达式。
            let loading = boolean_value(attribute)?;
            // 复用既有旋转器与交互禁用实现。
            view = quote! { (#view).loading(#loading) };
        }
    }
    // ButtonGroup 子按钮在公共样式与事件物化前写入连体位置。
    if let Some(position) = group_position {
        // 调用 ButtonBuilder 的公开连体位置物化入口。
        view = quote! { (#view).group_position(#position) };
    }
    // 在按钮专有属性之后应用公共 View 属性与事件。
    apply_common_attributes(
        // 传入已经完成专属配置的按钮构建链。
        view,
        // 保留原始属性供公共样式与事件处理。
        &element.attributes,
        // 标记所有已由按钮生成器消费的专属属性。
        &["type", "disabled", "block", "size", "loading"],
    )
}

// 生成图标元素。
pub(super) fn generate_icon(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 图标不能声明子内容。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶子组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整图标元素。
            element.span,
            // 说明 Icon 是叶子组件。
            "<Icon> 不接受子节点",
            // 给出自闭合写法。
            "使用 <Icon name=\"star\" />",
        ));
    }
    // name 是图标构造器的必需参数。
    let name_attribute = element
        // 借用属性列表。
        .attributes
        // 遍历属性。
        .iter()
        // 查找 name。
        .find(|attribute| attribute.name == "name")
        // 缺失时生成结构化诊断。
        .ok_or_else(|| {
            // 构造必需属性诊断。
            Diagnostic::new(
                // 指向完整图标元素。
                element.span,
                // 说明缺少构造参数。
                "<Icon> 缺少必需的 name 属性",
                // 给出规范示例。
                "使用 <Icon name=\"star\" />",
            )
        })?;
    // 生成字符串或表达式图标名称。
    let name = string_value(name_attribute)?;
    // 构造公开 Icon 组件。
    let mut icon = quote! { ::uix_app::prelude::Icon::new(#name) };
    // 可选 size 必须在包装为 ViewNode 前应用。
    if let Some(attribute) = element
        // 借用属性列表。
        .attributes
        // 遍历属性。
        .iter()
        // 查找 size。
        .find(|attribute| attribute.name == "size")
    {
        // 生成像素或表达式数值。
        let size = numeric_value(attribute)?;
        // 应用图标尺寸。
        icon = quote! { (#icon).size(#size) };
    }
    // 把 Widget 包装为公开 ViewNode。
    let base = quote! { ::uix_app::prelude::ViewNode::leaf(#icon) };
    // 应用其余公共属性与事件。
    apply_common_attributes(base, &element.attributes, &["name", "size"])
}

// 应用所有核心元素共享的公开 View 属性与事件。
pub(super) fn apply_common_attributes(
    // 接收已经生成的基础 View 表达式。
    mut view: TokenStream,
    // 接收源顺序属性。
    attributes: &[Attribute],
    // 接收元素专有阶段已经消费的属性名。
    consumed: &[&str],
) -> Result<TokenStream, Diagnostic> {
    // 先应用非事件属性，避免提前捕获构建器。
    for attribute in attributes {
        // 专用父子组件可能直接复用子元素生成器；内部源码身份绝不能进入公开属性矩阵。
        if attribute.name == SOURCE_ID_ATTRIBUTE {
            // 当前生成上下文已经持有同一 SourceId，只需消费内部属性。
            continue;
        }
        // 跳过元素专有阶段已经消费的属性。
        if consumed.contains(&attribute.name.as_str()) {
            // 继续处理下一属性。
            continue;
        }
        // 事件在第二轮统一处理。
        if attribute.name.starts_with('@') {
            // 继续处理下一普通属性。
            continue;
        }
        // 按公开 View API 映射登记属性。
        view = match attribute.name.as_str() {
            // 间距映射到 ViewNode::gap。
            "gap" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用子节点间距。
                quote! { (#view).gap(#value) }
            }
            // 内边距映射到 ViewNode::padding。
            "padding" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用统一内边距。
                quote! { (#view).padding(#value) }
            }
            // 外边距映射到 ViewNode::margin。
            "margin" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用统一外边距。
                quote! { (#view).margin(#value) }
            }
            // 宽度映射到 ViewNode::width。
            "width" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用固定宽度。
                quote! { (#view).width(#value) }
            }
            // 高度映射到 ViewNode::height。
            "height" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用固定高度。
                quote! { (#view).height(#value) }
            }
            // 动态定位四边复用公开数值属性契约，几何仍由 ViewNode 统一求解。
            "top" | "right" | "bottom" | "left" => {
                let value = numeric_value(attribute)?;
                let method = Ident::new(&attribute.name, Span::call_site());
                quote! { (#view).#method(#value) }
            }
            // 主轴扩张映射到 ViewNode::flex_grow。
            "flexGrow" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用扩张因子。
                quote! { (#view).flex_grow(#value) }
            }
            // 主轴收缩映射到 ViewNode::flex_shrink。
            "flexShrink" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用收缩因子。
                quote! { (#view).flex_shrink(#value) }
            }
            // 文本颜色映射到公开 color 样式。
            "color" => {
                // 生成字符串或 Rust 表达式。
                let value = string_value(attribute)?;
                // 应用前景色。
                quote! { (#view).color(#value) }
            }
            // 背景色映射到公开 bg 样式。
            "backgroundColor" => {
                // 生成字符串或 Rust 表达式。
                let value = string_value(attribute)?;
                // 应用背景色。
                quote! { (#view).bg(#value) }
            }
            // 字号映射到公开 TypographyToken 或数值。
            "fontSize" => {
                // 生成字号表达式。
                let value = typography_value(attribute)?;
                // 应用字号。
                quote! { (#view).font_size(#value) }
            }
            // 自动化标识映射到公开 automation_id。
            "automationId" => {
                // 生成字符串或表达式。
                let value = string_value(attribute)?;
                // 应用稳定自动化标识。
                quote! { (#view).automation_id(#value) }
            }
            // 普通 key 映射到公开 ViewNode::key。
            "key" => {
                // 生成字符串或表达式。
                let value = string_value(attribute)?;
                // 统一格式化为公开 ViewNode 接受的稳定字符串身份。
                quote! { (#view).key(::std::format!("{}", #value)) }
            }
            // 交叉轴对齐映射到公开枚举。
            "align" => {
                // 生成登记的对齐枚举。
                let value = align_value(attribute)?;
                // 应用交叉轴对齐。
                quote! { (#view).align(#value) }
            }
            // 主轴对齐映射到公开枚举。
            "justify" => {
                // 生成登记的对齐枚举。
                let value = justify_value(attribute)?;
                // 应用主轴对齐。
                quote! { (#view).justify(#value) }
            }
            // 样式类由完整样式映射 Gate 处理。
            "class" => {
                // 返回明确阶段边界诊断。
                return Err(deferred_style_diagnostic(attribute, "class"));
            }
            // 内联样式由完整样式映射 Gate 处理。
            "style" => {
                // 把结构化样式属性精确写入当前 ViewNode 的 Style。
                apply_inline_style(view, attribute)?
            }
            // 未登记属性禁止静默丢弃。
            _ => {
                // 返回未知属性映射诊断。
                return Err(Diagnostic::new(
                    // 指向完整属性。
                    attribute.span,
                    // 说明缺少公开 API 映射。
                    format!("属性 {} 尚无已登记的 Rust API 映射", attribute.name),
                    // 指向后续映射矩阵或可用集合。
                    "使用当前核心属性，或在内置组件映射矩阵 Gate 中登记后再使用",
                ));
            }
        };
    }
    // 在全部普通属性物化后应用事件。
    for attribute in attributes {
        // 编译器内部源码身份不是事件，不得进入事件映射。
        if attribute.name == SOURCE_ID_ATTRIBUTE {
            // 继续处理下一属性。
            continue;
        }
        // 专用生成器已经消费的事件不能再次进入核心点击映射。
        if consumed.contains(&attribute.name.as_str()) {
            // 继续处理下一事件属性。
            continue;
        }
        // 只处理事件属性。
        if attribute.name.starts_with('@') {
            // 生成事件链式调用。
            view = apply_event(view, attribute)?;
        }
    }
    // 返回完整 View 表达式。
    Ok(view)
}

// 生成保持 painter 与组合顺序的子节点向量。
pub(super) fn generate_children(children: &[Node]) -> Result<TokenStream, Diagnostic> {
    // 创建卫生的子节点向量名称。
    let output = Ident::new("__uix_children", Span::mixed_site());
    // 生成有序节点追加语句。
    let statements = generate_child_statements(children, &output)?;
    // 返回构建完成的 ViewNode 向量。
    Ok(quote! {{
        // 创建公开 ViewNode 子节点向量。
        let mut #output = ::std::vec::Vec::<::uix_app::prelude::ViewNode>::new();
        // 按源码顺序执行节点追加与控制流。
        #statements
        // 返回有序子节点。
        #output
    }})
}

// 生成一组节点的顺序追加语句。
pub(super) fn generate_child_statements(
    // 接收有序语言节点。
    children: &[Node],
    // 接收目标子节点向量。
    output: &Ident,
) -> Result<TokenStream, Diagnostic> {
    // 保存有序语句令牌。
    let mut statements = Vec::new();
    // 按源码顺序处理每个节点索引，条件链可一次消费多个相邻分支。
    let mut index = 0;
    // 逐项生成直到消费全部节点。
    while index < children.len() {
        // 借用当前节点。
        let child = &children[index];
        // 忽略布局容器之间仅用于排版源码的空白。
        if matches!(child, Node::Text(text) if text.value.trim().is_empty()) {
            // 继续处理下一节点。
            // 前进到下一节点。
            index += 1;
            // 继续扫描。
            continue;
        }
        // 控制元素在当前向量作用域内展开。
        if let Node::Element(element) = child {
            // If 会连同相邻 ElseIf/Else 生成单次短路条件链。
            if element.name == "If" {
                // 生成链并取得下一个尚未消费的节点索引。
                let (chain, next_index) = generate_conditional_chain(children, index, output)?;
                // 保存完整条件链语句。
                statements.push(chain);
                // 跳过已经归入链内的分支和空白。
                index = next_index;
                // 继续处理链后节点。
                continue;
            }
            // 孤立 ElseIf 或 Else 没有前导 If，必须在编译期拒绝。
            if matches!(element.name.as_str(), "ElseIf" | "Else") {
                // 返回相邻配对诊断。
                return Err(Diagnostic::new(
                    // 指向孤立分支。
                    element.span,
                    // 陈述缺少前导条件链。
                    format!("孤立 <{}> 缺少相邻的前导 If", element.name),
                    // 给出紧邻修复方式。
                    "把该分支紧接在 <If> 或 <ElseIf> 后，且中间不要插入其他元素",
                ));
            }
            // For 生成控制流语句而非占位节点。
            if element.name == "For" {
                // 生成控制流语句。
                statements.push(generate_control(element, output)?);
                // 继续处理下一节点。
                // 前进到下一节点。
                index += 1;
                // 继续扫描。
                continue;
            }
            // KernelChildren 直接消费拥有型 ViewNode 列表，不生成额外占位 View。
            if element.name == "KernelChildren" {
                // 生成无克隆的顺序追加语句。
                statements.push(generate_kernel_children(element, output)?);
                // 前进到下一个源节点。
                index += 1;
                // 继续扫描。
                continue;
            }
        }
        // 生成普通节点 View。
        let view = generate_node_view(child)?;
        // 按顺序追加到目标向量。
        statements.push(quote! { #output.push(#view); });
        // 前进到下一节点。
        index += 1;
    }
    // 拼接全部有序语句。
    Ok(quote! { #(#statements)* })
}

// 生成一个非控制节点的 View 表达式，并向专用布局映射共享单子节点入口。
pub(super) fn generate_node_view(node: &Node) -> Result<TokenStream, Diagnostic> {
    // 按节点类型生成公开 View。
    match node {
        // 普通元素递归生成。
        Node::Element(element) => generate_view(element),
        // 非空直接文本生成 label。
        Node::Text(text) => {
            // 借用文本值用于字面量生成。
            let value = &text.value;
            // 返回文本 View。
            Ok(quote! { ::uix_app::prelude::label(#value) })
        }
        // 直接插值生成动态字符串 label。
        Node::Interpolation(expression) => {
            // 生成插值表达式。
            let value = generate_expression(&expression.expression, None)?;
            // 返回使用公开 ToString 的文本 View。
            Ok(quote! {
                // 把插值值转换为拥有所有权的文本。
                ::uix_app::prelude::label(::std::string::ToString::to_string(&(#value)))
            })
        }
        // 成员声明块是纯语法载体，声明解析阶段已经从模板中剥离。
        Node::WidgetMember(_) => Err(Diagnostic::new(
            // 使用零跨度定位内部装配错误。
            SourceSpan {
                start: 0,
                end: 0,
                line: 1,
                column: 1,
            },
            // 说明成员块不是可渲染节点。
            "成员声明块不是可渲染节点",
            // 给出正确归属提醒。
            "成员块只能出现在 <Widget> 模板顶部，由声明解析器剥离",
        )),
    }
}

// 生成 If 或 For 控制流语句。
fn generate_control(
    // 接收控制元素。
    element: &Element,
    // 接收目标子节点向量。
    output: &Ident,
) -> Result<TokenStream, Diagnostic> {
    // 按控制绑定结构生成。
    match (&element.name[..], element.control.as_ref()) {
        // For 按数据源生成重复子节点。
        (
            "For",
            Some(ControlBinding::For {
                binding,
                binding_span,
                index_binding,
                index_span,
                iterable,
                key,
            }),
        ) => {
            // 读取组件展开阶段为当前 For 分配的实例路径名称。
            let path = internal_control_ident(element, "__uix_for_path")?;
            // 读取可选父 For 实例路径名称。
            let parent_path = optional_internal_control_ident(element, "__uix_for_parent_path")?;
            // 生成带实际实例身份的循环。
            generate_for(
                // 传递循环项绑定。
                binding,
                // 传递循环项跨度。
                *binding_span,
                // 传递可选索引绑定。
                index_binding.as_deref(),
                // 传递可选索引跨度。
                *index_span,
                // 传递数据源表达式。
                iterable,
                // 传递可选稳定 key。
                key.as_ref(),
                // 传递循环子节点。
                &element.children,
                // 传递目标向量。
                output,
                // 传递完整控制跨度。
                element.span,
                // 传递控制元素继承的组件作用域标记。
                &element.widget_scopes,
                // 传递循环子树拥有型事件捕获的逐迭代克隆契约。
                &element.for_iteration_clones,
                // 传递循环子树按依赖顺序生成的逐迭代准备语句。
                &element.for_iteration_setup,
                // 传递当前循环路径局部变量。
                &path,
                // 传递可选父循环路径局部变量。
                parent_path.as_ref(),
            )
        }
        // 名称与控制绑定不一致表示内部结构损坏。
        _ => Err(Diagnostic::new(
            // 指向完整控制元素。
            element.span,
            // 说明控制结构不完整。
            format!("<{}> 缺少匹配的控制绑定", element.name),
            // 给出重新解析建议。
            "使用规范 If 或 For 语法重新声明控制元素",
        )),
    }
}

// 生成控制流内部子节点，并把控制元素继承的作用域传播到每个实际根。
pub(super) fn generate_scoped_child_statements(
    // 接收需要按源码顺序生成的子节点。
    children: &[Node],
    // 接收外层目标子节点向量。
    output: &Ident,
    // 接收控制元素继承的组件私有状态作用域标记。
    widget_scopes: &[WidgetScopeMarker],
) -> Result<TokenStream, Diagnostic> {
    // 没有组件私有状态作用域时保留既有直接追加路径。
    if widget_scopes.is_empty() {
        // 生成原有的有序子节点追加语句。
        return generate_child_statements(children, output);
    }
    // 创建控制流局部缓冲，避免修改兄弟节点的追加顺序。
    let scoped_output = Ident::new("__uix_scoped_children", Span::mixed_site());
    // 先生成控制分支内的全部子节点。
    let generated_children = generate_child_statements(children, &scoped_output)?;
    // 把控制元素自己的作用域标记应用到每个实际根节点。
    let scoped_view = apply_widget_scopes(quote! { __uix_scoped_view }, widget_scopes)?;
    // 返回缓冲、标记和追加的完整控制流语句。
    Ok(quote! {
        // 为当前控制分支收集实际生成的 View 根节点。
        let mut #scoped_output = ::std::vec::Vec::<::uix_app::prelude::ViewNode>::new();
        // 保持分支内部源码顺序生成全部子节点。
        #generated_children
        // 为每个实际根追加控制元素继承的组件作用域标记。
        for __uix_scoped_view in #scoped_output {
            // 把同一根追加回外层目标向量。
            #output.push(#scoped_view);
        }
    })
}

// 生成 For 循环与可选稳定 key。
#[allow(clippy::too_many_arguments)]
fn generate_for(
    // 接收循环项绑定名。
    binding: &str,
    // 接收循环项绑定跨度。
    binding_span: SourceSpan,
    // 接收可选索引绑定名。
    index_binding: Option<&str>,
    // 接收可选索引绑定跨度。
    index_span: Option<SourceSpan>,
    // 接收数据源表达式。
    iterable: &ExpressionNode,
    // 接收可选稳定 key 表达式。
    key: Option<&ExpressionNode>,
    // 接收循环子节点。
    children: &[Node],
    // 接收目标子节点向量。
    output: &Ident,
    // 接收完整 For 跨度。
    span: SourceSpan,
    // 接收控制元素继承的组件私有状态作用域标记。
    widget_scopes: &[WidgetScopeMarker],
    // 接收每次迭代都必须重新克隆的拥有型事件捕获名称。
    iteration_clones: &[String],
    // 接收每次迭代在构建子树前执行的组件准备语句。
    iteration_setup: &[String],
    // 接收当前循环实际实例路径名称。
    path: &Ident,
    // 接收可选父循环实际实例路径名称。
    parent_path: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // 生成 Rust 循环项标识符。
    let binding = rust_identifier(binding, binding_span)?;
    // 生成可选 Rust 索引标识符。
    let index_binding = index_binding
        // 转换存在的索引名称。
        .map(|name| rust_identifier(name, index_span.unwrap_or(binding_span)))
        // 把 Option<Result> 转置为 Result<Option>。
        .transpose()?;
    // 生成数据源表达式。
    let iterable = generate_expression(&iterable.expression, None)?;
    // 从展开阶段保存的卫生名称恢复逐迭代捕获标识符。
    let iteration_clones = iteration_clones
        // 遍历稳定声明顺序。
        .iter()
        // 生成仅供宏展开代码使用的 Rust 标识符。
        .map(|name| Ident::new(name, Span::call_site()))
        // 收集供 quote 重复展开。
        .collect::<Vec<_>>();
    // 恢复只由展开器写入 AST 的内部逐迭代准备语句。
    let iteration_setup = parse_for_iteration_setup(iteration_setup, span)?;
    // 每次生成拥有所有权的克隆项，避免借用逃逸到事件闭包。
    let iterator = quote! { ::std::iter::IntoIterator::into_iter((#iterable).clone()) };
    // 创建内部枚举下标以同时支持身份与可选作者索引绑定。
    let ordinal = Ident::new("__uix_for_ordinal", Span::mixed_site());
    // 存在作者索引绑定时把内部下标复制到公开绑定名称。
    let index_setup = index_binding
        // 借用可选绑定。
        .as_ref()
        // 生成当前循环体内的别名。
        .map(|index| quote! { let #index = #ordinal; });
    // 带 key 的 For 必须有一个稳定行根节点。
    let body = if let Some(key) = key {
        // 收集排除排版空白后的直接子节点。
        let renderable = children
            // 遍历子节点。
            .iter()
            // 排除空白文本。
            .filter(|node| is_renderable_node(node))
            // 收集借用。
            .collect::<Vec<_>>();
        // key 只能应用到唯一行根。
        if renderable.len() != 1 {
            // 返回稳定身份形状诊断。
            return Err(Diagnostic::new(
                // 指向完整 For。
                span,
                // 说明 key 需要唯一行根。
                "带 key 的 For 必须恰好生成一个直接子节点",
                // 给出规范结构。
                "用 Container 包裹多个行内节点，再把该 Container 作为 For 的唯一子节点",
            ));
        }
        // 生成唯一行根 View。
        let view = generate_node_view(renderable[0])?;
        // 把 For 控制元素继承的组件作用域同步附到当前行根。
        let view = apply_widget_scopes(view, widget_scopes)?;
        // 生成 key 表达式。
        let key = generate_expression(&key.expression, None)?;
        // 创建只求值一次的行 key 局部变量。
        let row_key = Ident::new("__uix_for_row_key", Span::mixed_site());
        // 按嵌套层级组合当前实际实例路径。
        let path_value = if let Some(parent_path) = parent_path {
            // 父路径与当前 key 共同组成嵌套身份。
            quote! { ::std::format!("{}|{}", #parent_path, #row_key) }
        } else {
            // 顶层 key 本身就是当前实例路径。
            quote! { #row_key.clone() }
        };
        // 返回带稳定身份的追加语句。
        quote! {
            // key 表达式只求值一次并统一格式化。
            let #row_key = ::std::format!("{}", #key);
            // 声明当前循环项供动态样式子树引用。
            let #path = #path_value;
            // 在当前 key 实例作用域中建立 props、state、computed 与事件适配器。
            #(#iteration_setup)*
            // 为仍由外层准备区创建的拥有型捕获取得当前行副本。
            #(let #iteration_clones = (#iteration_clones).clone();)*
            // 生成当前循环行 View。
            let __uix_for_view = #view;
            // 把 key 转成公开 ViewNode 接受的字符串。
            #output.push(__uix_for_view.key(#row_key));
        }
    } else {
        // 按嵌套层级组合当前实际位置路径。
        let path_value = if let Some(parent_path) = parent_path {
            // 父路径与当前 ordinal 共同组成嵌套身份。
            quote! { ::std::format!("{}|{}", #parent_path, #ordinal) }
        } else {
            // 顶层 ordinal 转换为拥有所有权的字符串。
            quote! { ::std::format!("{}", #ordinal) }
        };
        // 无 key 时按位置追加全部循环子节点。
        // 对无 key 的所有实际行根传播控制元素继承的组件作用域。
        let children = if widget_scopes.is_empty() {
            // 保留既有直接追加路径。
            generate_child_statements(children, output)?
        } else {
            // 复用控制流根标记传播逻辑。
            generate_scoped_child_statements(
                // 传递当前循环迭代中需要生成的行节点。
                children,
                // 传递循环外层的目标子节点向量。
                output,
                // 传递当前 For 控制元素继承的组件作用域标记。
                widget_scopes,
            )?
        };
        // 先声明位置路径，再生成当前项全部子节点。
        quote! {
            // 声明当前循环项供动态样式子树引用。
            let #path = #path_value;
            // 在当前位置实例作用域中建立 props、state、computed 与事件适配器。
            #(#iteration_setup)*
            // 为仍由外层准备区创建的拥有型捕获取得当前行副本。
            #(let #iteration_clones = (#iteration_clones).clone();)*
            // 保持当前项子节点源码顺序。
            #children
        }
    };
    // 全部 For 都枚举内部位置，以支持无 key 动态样式身份。
    Ok(quote! {
        // 克隆数据源并按位置枚举。
        for (#ordinal, #binding) in (#iterator).enumerate() {
            // 在作者声明时暴露同一 usize 索引绑定。
            #index_setup
            // 按源码顺序生成当前项节点。
            #body
        }
    })
}

// 把文本与插值组合成一个拥有所有权的内容表达式。
pub(super) fn generate_text_content(
    // 接收有序文本子节点。
    children: &[Node],
    // 接收所属元素跨度。
    span: SourceSpan,
) -> Result<TokenStream, Diagnostic> {
    // 禁止文本型组件嵌套元素以免静默丢失结构。
    if children
        .iter()
        .any(|child| matches!(child, Node::Element(_)))
    {
        // 返回内容形状诊断。
        return Err(Diagnostic::new(
            // 指向所属文本型元素。
            span,
            // 说明当前公开构造器只接收文本。
            "当前 Text/Button/Typography 核心映射只接受文本与插值子节点",
            // 给出布局修复建议。
            "把图标或其他元素移到相邻 Container/Row 中",
        ));
    }
    // 纯静态文本直接合并为一个字面量。
    if children
        // 遍历全部子节点。
        .iter()
        // 确认没有插值。
        .all(|child| matches!(child, Node::Text(_)))
    {
        // 合并源码顺序中的全部文本片段。
        let value = children
            // 遍历文本片段。
            .iter()
            // 提取文本值。
            .filter_map(|child| match child {
                // 返回文本借用。
                Node::Text(text) => Some(text.value.as_str()),
                // 其他节点不应出现。
                _ => None,
            })
            // 收集为拥有所有权的字符串。
            .collect::<String>();
        // 返回静态字符串字面量。
        return Ok(quote! { #value });
    }
    // 创建卫生的动态文本缓冲区。
    let output = Ident::new("__uix_text", Span::mixed_site());
    // 保存有序文本追加语句。
    let mut statements = Vec::new();
    // 按源码顺序处理文本与插值。
    for child in children {
        // 文本片段直接追加。
        match child {
            // 追加静态文本。
            Node::Text(text) => {
                // 借用文本值。
                let value = &text.value;
                // 生成字符串追加。
                statements.push(quote! { #output.push_str(#value); });
            }
            // 插值先生成 Rust 表达式再转为文本。
            Node::Interpolation(expression) => {
                // 生成插值表达式。
                let value = generate_expression(&expression.expression, None)?;
                // 生成稳定 ToString 追加。
                statements.push(quote! {
                    // 把插值结果转换并追加到动态文本。
                    #output.push_str(&::std::string::ToString::to_string(&(#value)));
                });
            }
            // 元素已在函数开头拒绝。
            Node::Element(_) => {}
            // 成员块已在声明解析阶段剥离；防御性跳过保持零输出。
            Node::WidgetMember(_) => {}
        }
    }
    // 返回构建动态文本的 Rust 块。
    Ok(quote! {{
        // 创建动态文本缓冲区。
        let mut #output = ::std::string::String::new();
        // 按源码顺序追加片段。
        #(#statements)*
        // 返回拥有所有权的内容。
        #output
    }})
}

// 判断节点是否会生成可见 View。
pub(super) fn is_renderable_node(node: &Node) -> bool {
    // 空白文本仅用于格式化源文件，不生成节点。
    !matches!(node, Node::Text(text) if text.value.trim().is_empty())
}
