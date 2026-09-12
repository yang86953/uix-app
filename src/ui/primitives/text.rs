// 引入标签构造入口使用的声明节点。
use crate::ui::view::ViewNode;

/// 文本内容：静态字符串或动态闭包（公开用法见仓库 `docs/使用/组件.md`）。
pub trait IntoLabelContent {
    /// 消费静态或动态文本内容并构建标签视图节点。
    fn into_label_node(self) -> ViewNode;
}

// 为拥有所有权的字符串提供标签转换。
impl IntoLabelContent for String {
    // 消费字符串并构建静态标签节点。
    fn into_label_node(self) -> ViewNode {
        // 将字符串交给公开标签组件。
        ViewNode::leaf(crate::ui::Label::new(self))
    }
}

// 为字符串切片提供标签转换。
impl IntoLabelContent for &str {
    // 复制切片内容并构建静态标签节点。
    fn into_label_node(self) -> ViewNode {
        // 将切片交给公开标签组件。
        ViewNode::leaf(crate::ui::Label::new(self))
    }
}

// 为借用字符串提供标签转换。
impl IntoLabelContent for &String {
    // 借用字符串内容并构建静态标签节点。
    fn into_label_node(self) -> ViewNode {
        // 避免转移调用方字符串所有权。
        ViewNode::leaf(crate::ui::Label::new(self.as_str()))
    }
}

// 为返回字符串的闭包提供动态标签转换。
impl<F> IntoLabelContent for F
where
    // 保持原有闭包生命周期与返回值契约。
    F: Fn() -> String + 'static,
{
    // 把闭包封装为运行时动态标签节点。
    fn into_label_node(self) -> ViewNode {
        // 复用组件运行时持有的动态标签实现。
        ViewNode::leaf(crate::ui::widget_runtime::dynamic_label::DynamicLabel::new(
            self,
        ))
    }
}

/// 文本标签 — 静态或动态统一入口。
///
pub fn label(content: impl IntoLabelContent) -> ViewNode {
    // 按内容类型构建静态或动态标签节点。
    content.into_label_node()
}

/// 响应式文本标签（`label(closure)` 的别名，保留兼容）。
pub fn dynamic_label<F: Fn() -> String + 'static>(f: F) -> ViewNode {
    // 复用统一标签构造入口。
    label(f)
}

/// 响应式单行标签；受限宽度按真实字体省略，语义文本保留完整内容。
pub fn elided_label<F: Fn() -> String + 'static>(f: F) -> ViewNode {
    ViewNode::leaf(crate::ui::widget_runtime::dynamic_label::DynamicLabel::new(f).elided())
}
