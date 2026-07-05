//! 适配层 — 将 View 树展开为 WidgetTree。
//!
//! 用户在 `App::run()` 内部通过本模块将用户层的 `View` 树递归展开为框架层的
//! `WidgetTree`，完全隐藏 `WidgetNode`、`BoxedWidget` 等内部概念。
//!
//! # 职责
//!
//! 1. `ViewAdapter::build(root)` — 入口，将 `View` 构建为 `WidgetTree`。
//!    自动设置 View 上下文，使 `State::new` 绑定到正确的 WidgetId。
//! 2. `expand(node)` — 递归展开：ViewNode → WidgetNode。
//! 3. `apply_style(widget, style)` — 将 Style 应用到具体组件类型。
//!
//! # State 自动脏标记
//!
//! - View 构建期：`begin_state_capture` 捕获 `State::new`（`capture_view` / `with_view_context`）
//! - layout 后：`bind_reactive_widget_states` 探测 `dynamic_label` 闭包依赖并绑定 Paint 失效
//! - 兜底：`bind_orphan_pending_states` 将未关联 State 绑到根节点

use crate::ui::traits::WidgetComponent;
use crate::ui::style::Style;
use crate::ui::foundation::state::{begin_state_capture, end_state_capture};
use crate::ui::view::{View, ViewNode};
use crate::ui::{WidgetNode, WidgetTree};
use crate::ui::widgets::{Button, Container, Label};

/// View 树适配器。将 ViewNode 递归展开为 WidgetTree。
pub struct ViewAdapter;

impl ViewAdapter {
    /// 在 View 构建期开启 State 捕获并返回 ViewNode（供 `App::root` 使用）。
    pub fn capture_view(view: impl View) -> ViewNode {
        begin_state_capture();
        let node = view.build();
        end_state_capture();
        node
    }

    /// 将 `View` 树一步构建为 `WidgetTree`。
    pub fn build(view: impl View) -> WidgetTree {
        Self::build_nodes(Self::capture_view(view))
    }

    /// 将已展开的 ViewNode 树构建为 WidgetTree。
    pub fn build_nodes(root: ViewNode) -> WidgetTree {
        let mut tree = WidgetTree::new();
        let wnode = Self::expand(root);
        tree.build(wnode);
        tree.bind_orphan_pending_states();
        tree.bind_pending_effects();
        tree
    }

    /// 递归展开 ViewNode → WidgetNode。
    fn expand(node: ViewNode) -> WidgetNode {
        let children: Vec<WidgetNode> = node.children.into_iter().map(Self::expand).collect();

        let widget = Self::apply_style(node.widget, &node.style);

        let mut wnode = if children.is_empty() {
            WidgetNode::leaf(widget)
        } else {
            WidgetNode::new(widget, children)
        };

        if let Some(key) = node.key {
            wnode = wnode.key(&key);
        }

        if node.z_index != 0 {
            wnode = wnode.z_index(node.z_index);
        }

        wnode
    }

    fn apply_style(
        mut widget: Box<dyn WidgetComponent>,
        style: &Style,
    ) -> Box<dyn WidgetComponent> {
        if style == &Style::default() {
            return widget;
        }

        let tid = widget.as_any().type_id();

        if tid == std::any::TypeId::of::<Container>() {
            if let Some(c) = widget.as_any_mut().downcast_mut::<Container>() {
                c.style = style.clone();
            }
        } else if tid == std::any::TypeId::of::<Label>() {
            if let Some(l) = widget.as_any_mut().downcast_mut::<Label>() {
                l.style = Some(style.clone());
            }
        } else if tid == std::any::TypeId::of::<Button>() {
            if let Some(b) = widget.as_any_mut().downcast_mut::<Button>() {
                b.style = style.clone();
            }
        }

        widget
    }
}

/// 在闭包作用域内设置当前 View 的 State 捕获上下文（View 子树构建时使用）。
pub fn with_view_context<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    begin_state_capture();
    let result = f();
    end_state_capture();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::view::ViewNode;
    use crate::ui::WidgetCore;
    use crate::ui::widgets::Container;
    use crate::draw::Color;

    #[test]
    fn test_build_single_node() {
        let node = ViewNode::leaf(Container::new());
        let tree = ViewAdapter::build_nodes(node);
        assert!(tree.root().is_some());
    }

    #[test]
    fn test_build_with_children() {
        let child1 = ViewNode::leaf(Container::new());
        let child2 = ViewNode::leaf(Container::new());
        let node = ViewNode::new(Container::new(), vec![child1, child2]);
        let tree = ViewAdapter::build_nodes(node);
        let root_id = tree.root_id().expect("应有根节点");
        let root = tree.get(root_id).expect("根节点应存在");
        let child_ids = root.children().to_vec();
        assert_eq!(child_ids.len(), 2);
    }

    #[test]
    fn test_expand_key_and_zindex() {
        let node = ViewNode::leaf(Container::new())
            .key("my-container")
            .z_index(10);
        let wnode = ViewAdapter::expand(node);
        assert_eq!(wnode.key, Some("my-container".into()));
        assert_eq!(wnode.z_index, 10);
    }

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

    #[test]
    fn test_state_auto_dirty() {
        use crate::ui::state::State;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        let state = State::new(42);
        let dirty_called = Arc::new(AtomicBool::new(false));
        let dirty_called_clone = dirty_called.clone();
        state.set_dirty_fn(move || dirty_called_clone.store(true, Ordering::SeqCst));
        assert!(!dirty_called.load(Ordering::SeqCst));
        state.set(100);
        assert!(dirty_called.load(Ordering::SeqCst));
    }
}
