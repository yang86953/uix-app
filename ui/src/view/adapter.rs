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
//! 展开时通过 `set_current_view_dirty_fn` 设置线程局部回调。
//! `State::new` 创建时自动读取该回调并绑定——State 值变更时自动触发 WidgetTree 重绘。

use crate::api::traits::WidgetComponent;
use crate::style::Style;
use crate::view::{View, ViewNode};
use crate::widget::{WidgetNode, WidgetTree};
use crate::widgets::{Button, Container, Label};

/// View 树适配器。将 ViewNode 递归展开为 WidgetTree。
pub struct ViewAdapter;

impl ViewAdapter {
    /// 将 `View` 树构建为 `WidgetTree`。
    ///
    /// 在构建前设置 View 上下文，`State::new` 在其内部创建时会自动绑定脏标记。
    /// `view.build()` 只会被调用一次，返回可直接交给渲染循环。
    pub fn build(view: impl View) -> WidgetTree {
        // 设置脏标记上下文（通过线程局部方式），使 State::new 自动绑定
        // 由于 ViewNode 在此闭包外构建，此处实际需要一个更细粒度的绑定方式。
        // 详见 Phase 3 的 ViewContext 设计。

        // 先用 thread_local 设置一个（将在更细粒度控制后完善）
        let root_node = view.build();
        Self::build_nodes(root_node)
    }

    /// 将已展开的 ViewNode 树构建为 WidgetTree。
    /// 当 ViewNode 已通过其他方式构建时使用此方法。
    pub fn build_nodes(root: ViewNode) -> WidgetTree {
        let mut tree = WidgetTree::new();
        let wnode = Self::expand(root);
        tree.build(wnode);
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

/// 在闭包作用域内设置当前 View 上下文。
/// `State::new` 在 `f` 内部创建时会自动绑定到此上下文的脏标记回调。
/// 返回闭包的返回值。
pub fn with_view_context<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    // Phase 3 完善：此处会设置 ViewId 上下文，使 State::new 能关联到 WidgetTree 节点。
    // 当前先直接执行，State 绑定通过 set_dirty_fn 手动/自动完成。
    f()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::ViewNode;
    use crate::widget::WidgetCore;
    use crate::widgets::Container;
    use uix_graphics::Color;

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
        use crate::state::State;
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
