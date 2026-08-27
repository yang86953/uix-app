// 引入浮点字面量与过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Literal, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性、递归子树生成与可见节点判定。
use super::codegen::{apply_common_attributes, generate_node_view, is_renderable_node};
// 引入 Splitter 属性解析所需的共享 AST、表达式与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Node, generate_expression, literal_string,
};

// 生成复用公开运行时组件的双面板 Splitter。
pub(crate) fn generate_splitter(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 宏层只负责验证文档规定的两个直接内容面板。
    let [first, second] = splitter_children(element)?;
    // 递归生成左侧或上侧面板。
    let first = generate_node_view(first)?;
    // 递归生成右侧或下侧面板。
    let second = generate_node_view(second)?;
    // 从公开运行时默认构造器开始配置。
    let mut widget = quote! { ::uix::prelude::Splitter::new() };
    // 可选方向必须在编译期确定组件轴向。
    if let Some(attribute) = find_attribute(element, "direction") {
        // 读取文档登记的方向关键字。
        let direction = literal_string(attribute, "Splitter direction")?;
        // 映射到公开运行时的轴向构建器。
        widget = match direction.as_str() {
            // horizontal 是文档默认横向分隔。
            "horizontal" => quote! { (#widget).vertical(false) },
            // vertical 选择纵向堆叠与水平分隔柄。
            "vertical" => quote! { (#widget).vertical(true) },
            // 未登记方向不得静默近似。
            _ => {
                // 返回指向非法属性的结构化诊断。
                return Err(Diagnostic::new(
                    // 精确标记方向属性。
                    attribute.span,
                    // 说明允许的枚举值。
                    "Splitter direction 只支持 horizontal 或 vertical",
                    // 给出两种合法写法。
                    "使用 direction=\"horizontal\" 或 direction=\"vertical\"",
                ));
            }
        };
    }
    // 可选初始比例映射到运行时拥有的归一化构建器。
    if let Some(attribute) = find_attribute(element, "defaultRatio") {
        // 字面量在生成期验证，表达式交给运行时夹紧。
        let ratio = ratio_value(attribute)?;
        // 声明双面板左侧或上侧的初始比例。
        widget = quote! { (#widget).default_ratio(#ratio) };
    }
    // 明确建立运行时 Splitter 对两个有序内容 View 的组合所有权。
    let base = quote! {
        // 使用公开节点构造器保留组件与子树边界。
        ::uix::prelude::ViewNode::new(
            // 传入已经配置的 Splitter 运行时组件。
            #widget,
            // 按来源顺序构建恰好两个面板。
            ::std::vec![
                // 构建左侧或上侧面板。
                ::uix::prelude::View::build(#first),
                // 构建右侧或下侧面板。
                ::uix::prelude::View::build(#second),
            ],
        )
    };
    // 消费专有属性并应用统一尺寸、样式与自动化属性。
    apply_common_attributes(base, &element.attributes, &["direction", "defaultRatio"])
}

// 提取 Splitter 恰好两个可渲染直接子节点。
fn splitter_children(element: &Element) -> Result<[&Node; 2], Diagnostic> {
    // 忽略只承担源码排版作用的空白文本。
    let children = element
        // 借用源顺序子节点。
        .children
        // 遍历全部直接子节点。
        .iter()
        // 只保留会生成 View 的节点。
        .filter(|child| is_renderable_node(child))
        // 收集借用以便报告实际数量。
        .collect::<Vec<_>>();
    // 文档契约要求且只允许两个面板。
    if children.len() != 2 {
        // 返回包含实际数量的结构诊断。
        return Err(Diagnostic::new(
            // 指向完整 Splitter。
            element.span,
            // 说明双面板形状与实际数量。
            format!(
                "<Splitter> 必须恰好包含两个可渲染直接子节点，当前为 {} 个",
                // 插入经过空白过滤后的数量。
                children.len()
            ),
            // 引导调用方显式组织每个面板的复杂内容。
            "在 Splitter 内放置两个 Container、Row、Column、Grid 或其他 View；每个面板的多个内容请先用布局容器包裹",
        ));
    }
    // 数量已经验证，可安全返回固定形状借用。
    Ok([children[0], children[1]])
}

// 生成 defaultRatio 的有限 f32 表达式。
fn ratio_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状选择生成期验证或运行时收敛。
    match &attribute.value {
        // 字符串字面量可以在宏展开时完成范围验证。
        AttributeValue::Literal(value) => {
            // 解析无单位浮点数。
            let ratio = value.parse::<f32>().map_err(|_| {
                // 返回字面量格式诊断。
                Diagnostic::new(
                    // 指向非法比例属性。
                    attribute.span,
                    // 说明比例不接受长度单位或其他文本。
                    "Splitter defaultRatio 必须是 0~1 的无单位数字",
                    // 给出合法字面量示例。
                    "使用 defaultRatio=\"0.3\" 或 defaultRatio={ratio}",
                )
            })?;
            // 非有限值或越界值在生成期直接拒绝。
            if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) {
                // 返回稳定的闭区间诊断。
                return Err(Diagnostic::new(
                    // 指向越界比例属性。
                    attribute.span,
                    // 说明文档规定的取值范围。
                    "Splitter defaultRatio 必须位于 0~1 闭区间",
                    // 给出合法范围示例。
                    "使用 defaultRatio=\"0.5\"，或让动态表达式返回 0~1 的有限 f32",
                ));
            }
            // 构造明确的 f32 字面量，避免调用方类型推断漂移。
            let ratio = Literal::f32_unsuffixed(ratio);
            // 返回可嵌入公开构建器的令牌。
            Ok(quote! { #ratio })
        }
        // 动态表达式由受限表达式生成器校验语法。
        AttributeValue::Expression(expression) => {
            // 运行时 default_ratio 负责有限值与闭区间收敛。
            generate_expression(&expression.expression, None)
        }
        // 结构化内联样式不能作为数值比例。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向错误值形状。
            attribute.span,
            // 说明属性类型要求。
            "Splitter defaultRatio 不能使用内联样式",
            // 给出合法数值写法。
            "使用 defaultRatio=\"0.5\" 或 defaultRatio={ratio}",
        )),
    }
}

// 查找元素上的具名属性。
