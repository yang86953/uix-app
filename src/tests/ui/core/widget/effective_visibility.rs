use crate::tests::common::*;
use crate::ui::widgets::{Container, Popover};

struct AnimationProbe {
    updates: Arc<AtomicUsize>,
}

impl WidgetComponent for AnimationProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::ANIMATION)
    }

    fn as_animation(&self) -> Option<&dyn WidgetAnimation> {
        Some(self)
    }

    fn as_animation_mut(&mut self) -> Option<&mut dyn WidgetAnimation> {
        Some(self)
    }
}

impl WidgetAnimation for AnimationProbe {
    fn update_animation(&mut self, _dt: f64) -> bool {
        self.updates.fetch_add(1, Ordering::Relaxed);
        true
    }
}

fn open_popover_under_parent() -> (WidgetTree, ComponentId, ComponentId) {
    let mut popover = Popover::new("details");
    popover.open();

    let mut tree = WidgetTree::new();
    let parent = tree.set_root(Box::new(Container::new()));
    let child = tree.add_child(parent, Box::new(popover));
    tree.get_mut(parent)
        .expect("parent")
        .set_frame(Rect::new(0.0, 0.0, 320.0, 200.0));
    tree.get_mut(child)
        .expect("popover")
        .set_frame(Rect::new(40.0, 30.0, 80.0, 28.0));
    tree.get_mut(child).expect("popover").set_active(true);
    (tree, parent, child)
}

#[test]
fn hidden_ancestor_removes_descendant_overlay_registration() {
    let (mut tree, parent, _) = open_popover_under_parent();
    tree.rebuild_widget_overlays();
    assert_eq!(tree.overlay_stack().len(), 1);

    tree.set_node_visibility(parent, false);
    tree.rebuild_widget_overlays();

    assert!(tree.overlay_stack().is_empty());
}

#[test]
fn hidden_ancestor_removes_descendant_overlay_semantic_bounds() {
    let (mut tree, parent, child) = open_popover_under_parent();
    assert!(tree.visible_rect_for(child).is_some());

    tree.set_node_visibility(parent, false);

    assert!(tree.visible_rect_for(child).is_none());
}

#[test]
fn hidden_ancestor_stops_descendant_animation_before_lifecycle_reconcile() {
    let updates = Arc::new(AtomicUsize::new(0));
    let mut tree = WidgetTree::new();
    let parent = tree.set_root(Box::new(Container::new()));
    let child = tree.add_child(
        parent,
        Box::new(AnimationProbe {
            updates: Arc::clone(&updates),
        }),
    );
    tree.get_mut(child).expect("animation").set_active(true);
    tree.set_node_visibility(parent, false);

    assert_eq!(
        tree.update_animation_nodes([child], 0.016),
        [(child, false)]
    );
    assert_eq!(updates.load(Ordering::Relaxed), 0);
}
