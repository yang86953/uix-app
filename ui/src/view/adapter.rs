//! 适配层 — 将 View 树展开为 WidgetTree。
//!
//! 用户在 `App::run()` 内部通过本模块将用户层的 `ViewNode` 树递归展开为框架层的
//! `WidgetTree`，完全隐藏 `WidgetNode`、`BoxedWidget` 等内部概念。
//!
//! # 职责
//!
//! 1. `ViewAdapter::build(root)` — 入口，将 ViewNode 树构建为 WidgetTree。
//! 2. `expand(node)` — 递归展开：ViewNode → WidgetNode。
//! 3. `apply_style(widget, style)` — 将 Style 应用到具体组件类型。

use crate::api::traits::WidgetComponent;
use crate::style::Style;
use crate::view::ViewNode;
use crate::widget::{WidgetNode, WidgetTree};
use crate::widgets::{Button, Container, Label};

/// View 树适配器。将 ViewNode 递归展开为 WidgetTree。
pub struct ViewAdapter;

impl ViewAdapter {
    /// 将 ViewNode 树构建为 WidgetTree。
    ///
    /// 此方法是外部唯一需要调用的入口。返回的 `WidgetTree` 可直接交给 App 渲染循环。
    pub fn build(root: ViewNode) -> WidgetTree {
        let mut tree = WidgetTree::new();
        let wnode = Self::expand(root);
        tree.build(wnode);
        tree
    }

    /// 递归展开 ViewNode → WidgetNode。
    ///
    /// 对于每个节点：递归处理子节点 → 应用样式 → 设置 key/z_index。
    fn expand(node: ViewNode) -> WidgetNode {
        // 先递归展开子节点
        let children: Vec<WidgetNode> = node
            .children
            .into_iter()
            .map(Self::expand)
            .collect();

        // 将 ViewNode 的 style 应用到 widget
        let widget = Self::apply_style(node.widget, &node.style);

        // 根据是否有子节点选择构造方式
        let mut wnode = if children.is_empty() {
            WidgetNode::leaf(widget)
        } else {
            WidgetNode::new(widget, children)
        };

        // 设置 key（用于 diff/状态保持）
        if let Some(key) = node.key {
            wnode = wnode.key(&key);
        }

        // 设置叠加顺序
        if node.z_index != 0 {
            wnode = wnode.z_index(node.z_index);
        }

        wnode
    }

    /// 将 Style 应用到具体组件类型。
    ///
    /// 通过 `Any` 下转型检查组件是否为已知支持 style 的类型，若是则注入 style 覆盖。
    /// 不支持的组件类型（如 Input、Space）直接跳过，不报错。
    fn apply_style(
        mut widget: Box<dyn WidgetComponent>,
        style: &Style,
    ) -> Box<dyn WidgetComponent> {
        // Style 全默认值时无需应用（优化常见路径）
        if style == &Style::default() {
            return widget;
        }

        // 先通过 TypeId 判断类型，避免多次可变借用导致的编译问题
        let tid = widget.as_any().type_id();

        if tid == std::any::TypeId::of::<Container>() {
            // Container 有 pub style: Style 字段
            if let Some(c) = widget.as_any_mut().downcast_mut::<Container>() {
                c.style = style.clone();
            }
        } else if tid == std::any::TypeId::of::<Label>() {
            // Label 有 style: Option<Style> 字段
            if let Some(l) = widget.as_any_mut().downcast_mut::<Label>() {
                l.style = Some(style.clone());
            }
        } else if tid == std::any::TypeId::of::<Button>() {
            // Button 有 style: Style 字段
            if let Some(b) = widget.as_any_mut().downcast_mut::<Button>() {
                b.style = style.clone();
            }
        }
        // Input / Space 等无 style 字段的组件暂不支持 style 注入，直接跳过

        widget
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::ViewNode;
    use crate::widget::WidgetComponent;
    use crate::widget::WidgetCore;
    use uix_graphics::Color;
    use crate::widgets::Container;

    /// 验证 build 能成功构造 WidgetTree。
    #[test]
    fn test_build_single_node() {
        let node = ViewNode::leaf(Container::new());
        let tree = ViewAdapter::build(node);
        // 树应有一个根节点
        assert!(tree.root().is_some());
    }

    /// 验证 build 能递归展开子节点。
    #[test]
    fn test_build_with_children() {
        let child1 = ViewNode::leaf(Container::new());
        let child2 = ViewNode::leaf(Container::new());
        let node = ViewNode::new(Container::new(), vec![child1, child2]);
        let tree = ViewAdapter::build(node);
        let root_id = tree.root_id().expect("应有根节点");
        let root = tree.get(root_id).expect("根节点应存在");
        let child_ids = root.children().to_vec();
        assert_eq!(child_ids.len(), 2);
    }

    /// 验证 expand 正确处理 key 和 z_index。
    #[test]
    fn test_expand_key_and_zindex() {
        let node = ViewNode::leaf(Container::new())
            .key("my-container")
            .z_index(10);
        let wnode = ViewAdapter::expand(node);
        assert_eq!(wnode.key, Some("my-container".into()));
        assert_eq!(wnode.z_index, 10);
    }

    /// 验证 apply_style 对 Container 生效。
    #[test]
    fn test_apply_style_container() {
        let mut style = Style::default();
        style.background = Some(Color::red());

        let widget: Box<dyn WidgetComponent> = Box::new(Container::new());
        let styled = ViewAdapter::apply_style(widget, &style);

        if let Some(c) = styled.as_any().downcast_ref::<Container>() {
            assert_eq!(c.style.background, Some(Color::red()));
        } else {
            panic!("expected Container");
        }
    }

    /// 验证默认 style 时 apply_style 不修改 widget。
    #[test]
    fn test_apply_style_default_noop() {
        let style = Style::default();
        let widget: Box<dyn WidgetComponent> = Box::new(Container::new());
        let styled = ViewAdapter::apply_style(widget, &style);
        if let Some(c) = styled.as_any().downcast_ref::<Container>() {
            assert_eq!(c.style.background, None);
        } else {
            panic!("expected Container");
        }
    }
}
