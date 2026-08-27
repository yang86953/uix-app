// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入核心 View 生成器拥有的公共属性与子树映射。
use super::codegen::{apply_common_attributes, generate_node_view, is_renderable_node};
// 引入 Affix 专有属性所需的共享 AST、表达式与诊断契约。
use super::{
    AttributeValue, Diagnostic, Element, Node, generate_expression, numeric_value,
};

// 生成复用公开运行时组件的 Affix 固钉容器。
pub(crate) fn generate_affix(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Affix 只拥有一个明确的内容子树，避免宏层引入隐式布局。
    let child = single_affix_child(element)?;
    // 递归生成唯一内容 View。
    let child = generate_node_view(child)?;
    // offsetTop 缺省时沿用文档规定的零偏移。
    let offset_top = find_attribute(element, "offsetTop")
        // 已声明偏移时复用统一长度与数值表达式解析。
        .map(numeric_value)
        // 转置可选解析结果以保留来源诊断。
        .transpose()?
        // 未声明时生成确定的 f32 零值。
        .unwrap_or_else(|| quote! { 0.0_f32 });
    // scrollY 是 Affix 判断越过自然位置所需的状态来源。
    let scroll_attribute = find_attribute(element, "scrollY").ok_or_else(|| {
        // 返回缺失状态绑定诊断。
        Diagnostic::new(
            // 指向完整 Affix 元素。
            element.span,
            // 说明缺少运行时滚动事实来源。
            "<Affix> 缺少必需的 scrollY 状态绑定",
            // 给出最小合法 State<f32> 绑定写法。
            "使用 scrollY={scroll_y}",
        )
    })?;
    // 状态绑定只接受表达式，字符串不能表达 State 所有权。
    let AttributeValue::Expression(scroll_expression) = &scroll_attribute.value else {
        // 返回绑定形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 scrollY 属性。
            scroll_attribute.span,
            // 说明公开生成契约需要 State<f32>。
            "Affix scrollY 必须绑定 State<f32> 表达式",
            // 给出最小合法写法。
            "使用 scrollY={scroll_y}",
        ));
    };
    // 把受限状态表达式生成为 Rust 值。
    let scroll_state = generate_expression(&scroll_expression.expression, None)?;
    // 通过公开组件读取当前状态快照，根 View 依赖负责触发后续 reconcile。
    let widget = quote! {
        // 构造公开 Affix 并声明当前滚动位置。
        (::uix::prelude::Affix::new(#offset_top)).scroll_y((#scroll_state).get())
    };
    // 用公开 ViewNode 明确建立 Affix 对唯一内容 View 的组合所有权。
    let base = quote! {
        // 物化子 View 后交给公开 Affix 节点。
        ::uix::prelude::ViewNode::new(
            // 传入已经配置的公开运行时组件。
            #widget,
            // 保留唯一内容 View 的来源顺序。
            ::std::vec![::uix::prelude::View::build(#child)],
        )
    };
    // 专有属性消费后，把尺寸、样式与自动化属性交给统一契约。
    apply_common_attributes(base, &element.attributes, &["offsetTop", "scrollY"])
}

// 提取 Affix 唯一的可渲染直接子节点。
fn single_affix_child(element: &Element) -> Result<&Node, Diagnostic> {
    // 忽略只承担源码排版作用的空白文本。
    let mut children = element
        .children
        .iter()
        .filter(|child| is_renderable_node(child));
    // 没有内容时无法建立固钉占位与自然位置。
    let Some(child) = children.next() else {
        // 返回缺失内容诊断。
        return Err(Diagnostic::new(
            // 指向空 Affix。
            element.span,
            // 说明唯一子树要求。
            "<Affix> 必须包含一个可渲染直接子节点",
            // 给出明确内容示例。
            "在 Affix 内放置一个 Container、Row、Column、Grid 或其他 View",
        ));
    };
    // 第二个可渲染节点会制造不明确的固钉占位语义。
    if children.next().is_some() {
        // 返回多子节点形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Affix。
            element.span,
            // 说明宏不会隐式包裹多个节点。
            "<Affix> 只能包含一个可渲染直接子节点",
            // 引导调用方显式选择内容布局。
            "用 Container、Row、Column 或 Grid 包裹多个内容节点",
        ));
    }
    // 返回经过形状验证的唯一内容 View。
    Ok(child)
}

// 查找元素上的具名属性。
