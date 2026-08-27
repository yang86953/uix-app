// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Rate 属性、表达式、布尔值与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_expression};

// 生成保持 State<u32> 评分所有权的 Rate 节点。
pub(crate) fn generate_rate(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Rate 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Rate 元素。
            element.span,
            // 说明评分组件不接受子节点。
            "<Rate> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Rate value={rating} />",
        ));
    }

    // 从公开默认构造器开始配置。
    let mut widget = quote! { ::uix::prelude::Rate::new() };
    // 星数必须在评分绑定前建立最大值契约。
    if let Some(attribute) = find_attribute(element, "count") {
        // 生成 usize 字面量或受限表达式。
        let count = usize_value(attribute)?;
        // 调用公开星数构建器。
        widget = quote! { (#widget).count(#count) };
    }
    // 半星语义必须在评分绑定前改变状态单位上限。
    if let Some(attribute) = find_attribute(element, "allowHalf") {
        // 复用统一布尔属性诊断与动态表达式生成。
        let enabled = boolean_value(attribute)?;
        // 静态 false 不生成无效配置调用。
        if !matches!(&attribute.value, AttributeValue::Literal(value) if value == "false") {
            // 静态 true 可直接调用公开半星构建器。
            if matches!(&attribute.value, AttributeValue::Literal(value) if value == "true") {
                // 启用半星状态单位。
                widget = quote! { (#widget).allow_half() };
            } else {
                // 动态布尔值用同类型分支选择是否启用半星。
                widget = quote! { if #enabled { (#widget).allow_half() } else { #widget } };
            }
        }
    }
    // 可选 value 必须保留 State<u32> 所有权句柄。
    if let Some(attribute) = find_attribute(element, "value") {
        // 字面量不能提供双向状态所有权。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回绑定形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 value 属性。
                attribute.span,
                // 说明公开运行时绑定类型。
                "Rate value 必须绑定 State<u32> 表达式",
                // 给出规范绑定写法。
                "使用 value={rating}",
            ));
        };
        // 生成受限状态表达式。
        let state = generate_expression(&expression.expression, None)?;
        // 借用状态句柄交给公开 Rate 双向绑定入口。
        widget = quote! { (#widget).value(&(#state)) };
    }

    // 物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 Rate 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的评分 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["value", "count", "allowHalf"],
    )
}

// 生成 usize 字面量或受限表达式。
fn usize_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成星数。
    match &attribute.value {
        // 静态值必须是 usize 整数。
        AttributeValue::Literal(source) => {
            // 解析无符号平台整数。
            let value = source.parse::<usize>().map_err(|_| {
                // 返回整数类型诊断。
                Diagnostic::new(
                    // 指向非法 count 属性。
                    attribute.span,
                    // 说明公开运行时类型。
                    "Rate count 必须是 usize 整数",
                    // 给出合法示例。
                    "使用 count=\"5\" 或 usize 表达式",
                )
            })?;
            // 生成类型明确的 usize 字面量。
            Ok(quote! { #value })
        }
        // 动态值保持 Rust usize 类型检查。
        AttributeValue::Expression(expression) => {
            // 生成受限整数表达式。
            generate_expression(&expression.expression, None)
        }
        // 结构化内联样式不可能用于星数。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向异常 count 属性。
            attribute.span,
            // 说明内部属性形状不匹配。
            "Rate count 不能使用内联样式值",
            // 给出有效整数写法。
            "使用 usize 整数字面量或受限表达式",
        )),
    }
}

// 查找元素上的具名属性。
