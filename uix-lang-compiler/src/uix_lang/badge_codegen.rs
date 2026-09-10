// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性、单节点生成与可见节点判定入口。
use super::codegen::{apply_common_attributes, generate_node_view, is_renderable_node};
// 引入 Badge 属性、表达式、节点与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Node, boolean_value, generate_expression,
    string_value,
};

// 生成零或一个静态真实子 View 的 Badge 装饰器。
pub(crate) fn generate_badge(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 收集忽略排版空白后的逻辑直接子节点。
    let children = element
        // 借用源码有序子节点。
        .children
        // 遍历全部直接子节点。
        .iter()
        // 只保留会生成可见 View 的节点。
        .filter(|node| is_renderable_node(node))
        // 物化集合以执行静态基数门禁。
        .collect::<Vec<_>>();
    // 多子节点会破坏唯一装饰锚点，必须在编译期拒绝。
    if children.len() > 1 {
        // 返回确定的组合形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Badge 元素。
            element.span,
            // 说明零或一直接子 View 的静态契约。
            "<Badge> 最多只能包含一个逻辑直接子 View",
            // 给出明确的组合容器修复路径。
            "使用 Container 或 Row 包裹多个子节点",
        ));
    }

    // 从公开默认构造器开始配置。
    let mut widget = quote! { ::uix_app::prelude::Badge::new() };
    // 可选数字接受 i32 字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "count") {
        // 生成类型明确的 i32 值。
        let count = i32_value(attribute)?;
        // 交给运行时统一归一化负数。
        widget = quote! { (#widget).count(#count) };
    }
    // 可选圆点接受布尔简写、字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "dot") {
        // 复用统一布尔属性生成与诊断。
        let dot = boolean_value(attribute)?;
        // 使用同类型构建器支持动态开关。
        widget = quote! { (#widget).dot_when(#dot) };
    }
    // 可选文字接受字符串字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "text") {
        // 生成统一字符串值令牌。
        let value = string_value(attribute)?;
        // 只在构造期间借用，运行时 Badge 复制并拥有文字。
        widget = quote! { (#widget).text(&*(#value)) };
    }

    // 可选唯一子节点必须拥有稳定静态 View 身份。
    if let Some(child) = children.first() {
        // 按直接节点形状建立真实子 View。
        let child = match child {
            // 直接控制流会让子基数或身份随运行时变化。
            Node::Element(element)
                if element.control.is_some() || matches!(element.name.as_str(), "If" | "For") =>
            {
                // 返回动态直接子树诊断。
                return Err(Diagnostic::new(
                    // 指向完整 Badge 元素。
                    element.span,
                    // 说明直接控制流不满足静态单子契约。
                    "<Badge> 的直接子 View 不能是 If 或 For",
                    // 建议把控制流放入唯一静态容器内部。
                    "使用一个静态 Container 或 Row 包裹 If 或 For",
                ));
            }
            // 普通静态元素递归生成完整 ViewNode。
            Node::Element(_) => generate_node_view(child)?,
            // 裸文本没有可供装饰与协调的组件身份。
            Node::Text(_) => {
                // 返回裸文本子节点诊断。
                return Err(Diagnostic::new(
                    // 指向完整 Badge 元素。
                    element.span,
                    // 说明必须使用真实 View。
                    "<Badge> 不接受可见文本作为直接子节点",
                    // 给出显式 Label 或容器修复路径。
                    "把文字放入 Label、Container 或 Row",
                ));
            }
            // 插值同样没有稳定组件身份。
            Node::Interpolation(_) => {
                // 返回插值子节点诊断。
                return Err(Diagnostic::new(
                    // 指向完整 Badge 元素。
                    element.span,
                    // 说明插值不满足真实子树契约。
                    "<Badge> 不接受插值作为直接子节点",
                    // 给出显式静态 View 修复路径。
                    "把插值放入唯一静态 Label、Container 或 Row",
                ));
            }
            // 成员块是声明载体，不会出现在调用内容里。
            Node::WidgetMember(_) => {
                // 返回成员块子节点诊断。
                return Err(Diagnostic::new(
                    // 指向完整 Badge 元素。
                    element.span,
                    // 说明成员块不满足直接子树契约。
                    "<Badge> 不接受成员声明块作为直接子节点",
                    // 给出显式静态 View 修复路径。
                    "把 @props/@state/@computed/@actions 移入 <Widget> 模板声明区",
                ));
            }
        };
        // 把完整真实子 ViewNode 交给运行时 owner。
        widget = quote! { (#widget).child(#child) };
    }

    // 子树由 Badge 的生命周期端口提供；公开根经组件自己的同目录 UIX 声明构建。
    let view = quote! { ::uix_app::prelude::View::build(#widget) };
    // 消费 Badge 专有属性并应用公共尺寸、样式、身份与事件。
    apply_common_attributes(
        // 传入已经配置的 Badge View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["count", "dot", "text"],
    )
}

// 生成 i32 字面量或受限表达式。
fn i32_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成数字徽章计数。
    match &attribute.value {
        // 静态值必须是 i32 整数。
        AttributeValue::Literal(source) => {
            // 解析与公开 Badge::count 相同的整数类型。
            let value = source.parse::<i32>().map_err(|_| {
                // 返回明确整数类型诊断。
                Diagnostic::new(
                    // 指向非法 count 属性。
                    attribute.span,
                    // 说明公开运行时类型。
                    "Badge count 必须是 i32 整数",
                    // 给出合法静态与动态写法。
                    "使用 count=\"5\" 或 i32 表达式",
                )
            })?;
            // 生成类型明确的 i32 字面量。
            Ok(quote! { #value })
        }
        // 动态数值表达式在 UIX 与 Rust 组件边界投影为 i32。
        AttributeValue::Expression(expression) => {
            // 生成受限整数表达式。
            let value = generate_expression(&expression.expression, None)?;
            // UIX 私有数值状态统一为 f64，在组件公开边界显式投影为 i32。
            Ok(quote! { ((#value) as i32) })
        }
        // 结构化内联样式不可能用于计数。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向异常 count 属性。
            attribute.span,
            // 说明属性值形状不匹配。
            "Badge count 不能使用内联样式值",
            // 给出有效整数写法。
            "使用 i32 整数字面量或受限表达式",
        )),
    }
}

// 查找元素上的具名属性。
