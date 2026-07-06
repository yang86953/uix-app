//! 閫傞厤灞?鈥?灏?View 鏍戝睍寮€涓?WidgetTree銆?//!
//! 鐢ㄦ埛鍦?`App::run()` 鍐呴儴閫氳繃鏈ā鍧楀皢鐢ㄦ埛灞傜殑 `View` 鏍戦€掑綊灞曞紑涓烘鏋跺眰鐨?//! `WidgetTree`锛屽畬鍏ㄩ殣钘?`WidgetNode`銆乣BoxedWidget` 绛夊唴閮ㄦ蹇点€?//!
//! # 鑱岃矗
//!
//! 1. `ViewAdapter::build(root)` 鈥?鍏ュ彛锛屽皢 `View` 鏋勫缓涓?`WidgetTree`銆?//!    鑷姩璁剧疆 View 涓婁笅鏂囷紝浣?`State::new` 缁戝畾鍒版纭殑 WidgetId銆?//! 2. `expand(node)` 鈥?閫掑綊灞曞紑锛歏iewNode 鈫?WidgetNode銆?//! 3. `apply_style(widget, style)` 鈥?灏?Style 搴旂敤鍒板叿浣撶粍浠剁被鍨嬨€?//!
//! # State 鑷姩鑴忔爣璁?//!
//! - View 鏋勫缓鏈燂細`begin_state_capture` 鎹曡幏 `State::new`锛坄capture_view` / `with_view_context`锛?//! - layout 鍚庯細`bind_reactive_widget_states` 鎺㈡祴 `dynamic_label` 闂寘渚濊禆骞剁粦瀹?Paint 澶辨晥
//! - 鍏滃簳锛歚bind_orphan_pending_states` 灏嗘湭鍏宠仈 State 缁戝埌鏍硅妭鐐?
use crate::ui::foundation::state::{begin_state_capture, end_state_capture};
use crate::ui::style::Style;
use crate::ui::traits::WidgetComponent;
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::{Button, Container, Label};
use crate::ui::{WidgetNode, WidgetTree};

/// View tree adapter.
pub struct ViewAdapter;

impl ViewAdapter {
    /// Builds a View while capturing State bindings.
    pub fn capture_view(view: impl View) -> ViewNode {
        begin_state_capture();
        let node = view.build();
        end_state_capture();
        node
    }

    /// Builds a View tree into a WidgetTree.
    pub fn build(view: impl View) -> WidgetTree {
        Self::build_nodes(Self::capture_view(view))
    }

    /// Builds an already expanded ViewNode tree into a WidgetTree.
    pub fn build_nodes(root: ViewNode) -> WidgetTree {
        let mut tree = WidgetTree::new();
        let wnode = Self::expand(root);
        tree.build(wnode);
        tree.bind_orphan_pending_states();
        tree.bind_pending_effects();
        tree
    }

    /// Expands a ViewNode recursively into a WidgetNode.
    pub(crate) fn expand(node: ViewNode) -> WidgetNode {
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

        if !node.handlers.is_empty() {
            wnode = wnode.with_handlers(node.handlers);
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

/// Runs a closure inside the current View state-capture context.
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
    use crate::draw::Color;
    use crate::ui::view::ViewNode;
    use crate::ui::widgets::Container;
    use crate::ui::WidgetCore;

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
        let root_id = tree.root_id().expect("root node should exist");
        let root = tree.get(root_id).expect("root node should be present");
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
        style.background = Some(crate::ui::style::ColorValue::Custom(Color::red()));
        let widget: Box<dyn WidgetComponent> = Box::new(Container::new());
        let styled = ViewAdapter::apply_style(widget, &style);
        if let Some(c) = styled.as_any().downcast_ref::<Container>() {
            assert_eq!(
                c.style.background,
                Some(crate::ui::style::ColorValue::Custom(Color::red()))
            );
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

    #[test]
    fn button_on_click_is_registered_as_semantic_handler() {
        use crate::core::Point;
        use crate::native::traits::input::{KeyMod, MouseButton};
        use crate::ui::view::button;
        use crate::ui::SystemEvent;
        use std::cell::Cell;
        use std::rc::Rc;

        let clicks = Rc::new(Cell::new(0));
        let clicks_for_handler = clicks.clone();
        let mut tree = ViewAdapter::build(button("OK").on_click(move || {
            clicks_for_handler.set(clicks_for_handler.get() + 1);
        }));

        let pos = Point::new(2.0, 2.0);
        let _ = tree.dispatch_event(&SystemEvent::PointerDown {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        });
        let _ = tree.dispatch_event(&SystemEvent::PointerUp {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        });

        assert_eq!(clicks.get(), 1);
    }

    #[test]
    fn input_on_change_is_registered_as_semantic_handler() {
        use crate::core::{Point, Rect};
        use crate::native::traits::input::{KeyMod, MouseButton};
        use crate::ui::view::input;
        use crate::ui::{SystemEvent, WidgetCore};
        use std::cell::RefCell;
        use std::rc::Rc;

        let value = Rc::new(RefCell::new(String::new()));
        let value_for_handler = value.clone();
        let mut tree = ViewAdapter::build(input().on_change(move |next| {
            *value_for_handler.borrow_mut() = next.to_string();
        }));

        let root = tree.root_id().expect("input root should exist");
        tree.get_mut(root)
            .expect("input root should be present")
            .set_frame(Rect::new(0.0, 0.0, 120.0, 32.0));

        let _ = tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(8.0, 8.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        });
        let _ = tree.dispatch_event(&SystemEvent::TextInput {
            text: "A".to_string(),
        });

        assert_eq!(&*value.borrow(), "A");
    }
}
