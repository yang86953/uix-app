// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入受限表达式、元素形状与诊断契约。
use super::{generate_expression, AttributeValue, Diagnostic, Element, Node};
// 引入公共 View 属性、子树生成与可见子节点判定。
use super::codegen::{apply_common_attributes, generate_node_view, is_renderable_node};

// 生成框架内部 Rust 基础 View 到 UIX 组合树的窄桥接。
pub(crate) fn generate_kernel_view(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 基础 View 已经拥有完整子树，桥接元素不能再接收 UIX 子节点。
    if element.children.iter().any(is_renderable_node) {
        // 返回明确的单值边界诊断。
        return Err(Diagnostic::new(
            // 指向完整桥接元素。
            element.span,
            // 说明不能混合两套子树所有权。
            "<KernelView> 不接受子节点",
            // 给出规范自闭合形状。
            "使用 <KernelView value={build_kernel_view()} />",
        ));
    }
    // 内核已经拥有事件语义，模板不能在桥接节点重复登记事件。
    reject_events(element, "KernelView")?;
    // 读取并生成唯一基础 View 表达式。
    let value = kernel_expression(element, "KernelView", "build_kernel_view()")?;
    // 只在统一 View 层应用样式、尺寸与身份，不复制组件专有属性。
    apply_common_attributes(quote! { #value }, &element.attributes, &["value"])
}

// 生成接收唯一 UIX 展示子树的 Rust 基础内核宿主。
pub(crate) fn generate_kernel_host(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 基础内核只包装一个明确展示根，避免隐式选择布局。
    let child = single_kernel_host_child(element)?;
    // 递归生成由 UIX 拥有的唯一展示子树。
    let child = generate_node_view(child)?;
    // 交互事件只由 Rust 内核拥有，UIX 外壳不能重复登记。
    reject_events(element, "KernelHost")?;
    // value 在此处是接收 ViewNode 的 Rust 基础内核构造函数。
    let host = kernel_expression(element, "KernelHost", "build_kernel_host")?;
    // 把 UIX 子树所有权传给 Rust 内核，再应用通用 View 样式。
    let base = quote! { (#host)(#child) };
    // 组件专有配置仍不允许穿透框架内部桥接。
    apply_common_attributes(base, &element.attributes, &["value"])
}

// 读取桥接元素唯一 value 表达式并生成 Rust 令牌。
fn kernel_expression(
    element: &Element,
    element_name: &str,
    example: &str,
) -> Result<TokenStream, Diagnostic> {
    // value 是桥接已有 Rust 基础内核的唯一专有输入。
    let value = element
        // 遍历已完成重复属性检查的列表。
        .attributes
        // 查找 value。
        .iter()
        .find(|attribute| attribute.name == "value")
        // 缺失时返回稳定诊断。
        .ok_or_else(|| {
            Diagnostic::new(
                // 指向完整空元素。
                element.span,
                // 说明必需属性。
                format!("<{element_name}> 缺少 value 表达式"),
                // 给出框架模板中的规范用法。
                format!("使用 <{element_name} value={{{example}}} />"),
            )
        })?;
    // 字面量不能拥有 Rust View 或函数项，必须保留调用方类型检查。
    let AttributeValue::Expression(value) = &value.value else {
        // 返回表达式形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 value 属性。
            value.span,
            // 说明所有权值必须来自 Rust 表达式。
            format!("{element_name} value 必须是 Rust 基础内核表达式"),
            // 给出相应表达式示例。
            format!("使用 value={{{example}}}"),
        ));
    };
    // 复用语言受限表达式生成，最终类型由 rustc 在调用点验证。
    generate_expression(&value.expression, None)
}

// 拒绝在已经拥有交互语义的基础内核桥接上重复声明事件。
fn reject_events(element: &Element, element_name: &str) -> Result<(), Diagnostic> {
    // 查找任意事件属性。
    let event = element
        // 借用源码属性列表。
        .attributes
        // 遍历全部属性。
        .iter()
        // 只匹配事件声明。
        .find(|attribute| attribute.name.starts_with('@'));
    // 没有事件时保持窄桥接合法。
    let Some(event) = event else {
        return Ok(());
    };
    // 返回指向冲突事件的诊断。
    Err(Diagnostic::new(
        // 使用事件属性自身跨度。
        event.span,
        // 说明基础内核已经拥有事件语义。
        format!("<{element_name}> 不接受事件 {}", event.name),
        // 引导把事件保留在 Rust 内核或外部普通 UIX 控件。
        "由 Rust 基础内核拥有交互，或把事件声明移到外部普通 UIX 控件",
    ))
}

// 提取 KernelHost 唯一的可渲染直接子节点。
fn single_kernel_host_child(element: &Element) -> Result<&Node, Diagnostic> {
    // 忽略只承担源码排版的空白文本。
    let mut children = element
        .children
        .iter()
        .filter(|child| is_renderable_node(child));
    // 缺失展示子树时无法调用宿主内核。
    let Some(child) = children.next() else {
        return Err(Diagnostic::new(
            // 指向完整宿主元素。
            element.span,
            // 说明唯一 UIX 展示根要求。
            "<KernelHost> 必须包含一个可渲染直接子节点",
            // 给出基础图标内容示例。
            "在 KernelHost 内放置一个 Icon、Container 或其他展示 View",
        ));
    };
    // 多个展示根必须由 UIX 显式选择布局。
    if children.next().is_some() {
        return Err(Diagnostic::new(
            // 指向完整宿主元素。
            element.span,
            // 说明不会隐式包裹多个子树。
            "<KernelHost> 只能包含一个可渲染直接子节点",
            // 引导显式选择布局。
            "用 Container、Row 或 Column 包裹多个展示节点",
        ));
    }
    // 返回由 UIX 完整拥有的唯一展示根。
    Ok(child)
}
