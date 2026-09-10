// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入共享公共属性、子树生成与可见节点判定。
use super::codegen::{apply_common_attributes, generate_node_view, is_renderable_node};
// 引入 WindowDragRegion 所需的语法树与诊断。
use super::{Diagnostic, Element, Node};

// 生成复用公开窗口拖拽包装器的 WindowDragRegion 标签。
pub(crate) fn generate_window_drag_region(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 拖拽包装器只拥有一个明确的展示子树，避免宏层引入隐式布局。
    let child = single_drag_region_child(element)?;
    // 递归生成唯一展示 View。
    let child = generate_node_view(child)?;
    // 拖拽区拥有指针手势，不能再叠加声明式点击处理器。
    if let Some(event) = element
        // 借用原始属性列表。
        .attributes
        // 遍历全部属性与事件。
        .iter()
        // 查找任意事件声明。
        .find(|attribute| attribute.name.starts_with('@'))
    {
        // 返回来源位置稳定的交互所有权诊断。
        return Err(Diagnostic::new(
            // 指向冲突的事件属性。
            event.span,
            // 说明拖拽区已经拥有指针语义。
            format!("<WindowDragRegion> 不接受事件 {}", event.name),
            // 引导把交互控件移出拖拽子树。
            "将按钮和其他交互控件放在 WindowDragRegion 外部的同级节点中",
        ));
    }
    // 只投影到 ui/window_chrome 已有的公开拖拽能力。
    let base = quote! {
        // 运行时继续拥有窗口动作与命中测试语义。
        ::uix_app::prelude::window_drag_region(#child)
    };
    // WindowDragRegion 没有专有属性，统一处理样式与未知属性诊断。
    apply_common_attributes(base, &element.attributes, &[])
}

// 提取 WindowDragRegion 唯一的可渲染直接子节点。
fn single_drag_region_child(element: &Element) -> Result<&Node, Diagnostic> {
    // 忽略只承担源码排版作用的空白文本。
    let mut children = element
        // 借用原始直接子节点。
        .children
        // 遍历子节点并保留来源顺序。
        .iter()
        // 只保留会生成 View 的节点。
        .filter(|child| is_renderable_node(child));
    // 空拖拽区无法建立可命中的标题栏范围。
    let Some(child) = children.next() else {
        // 返回缺失内容诊断。
        return Err(Diagnostic::new(
            // 指向完整 WindowDragRegion。
            element.span,
            // 说明唯一展示子树要求。
            "<WindowDragRegion> 必须包含一个可渲染直接子节点",
            // 给出显式展示容器示例。
            "在 WindowDragRegion 内放置一个 Container、Row、Column 或其他展示 View",
        ));
    };
    // 第二个可渲染节点会制造不明确的拖拽范围布局。
    if children.next().is_some() {
        // 返回多子节点形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 WindowDragRegion。
            element.span,
            // 说明宏不会隐式包裹多个节点。
            "<WindowDragRegion> 只能包含一个可渲染直接子节点",
            // 引导调用方显式选择展示布局。
            "用 Container、Row 或 Column 包裹多个展示节点",
        ));
    }
    // 返回经过形状验证的唯一展示 View。
    Ok(child)
}
