use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::traits::WidgetTextInput;
use crate::ui::widgets::display::tree::*;
use crate::ui::AccessibilityRole;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn pointer_down(pos: Point) -> SystemEvent {
    SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

fn pointer_up(pos: Point) -> SystemEvent {
    SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

fn render_tree(tree: &Tree, frame: Rect, surface_size: (i32, i32)) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let widget_tree = WidgetTree::new();
    let mut display_list = crate::draw::command::DisplayList::new();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            font,
            &fonts,
            &images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            surface_size.0,
            surface_size.1,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(tree, frame, ctx, &widget_tree);
        });
    }
    format!("{display_list:?}")
}

fn key_event(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    }
}

#[test]
fn tree_keyboard_navigation_expands_selects_checks_and_collapses() {
    let nodes = vec![
        TreeNode::new("Root", "root").children(vec![
            TreeNode::new("Disabled", "disabled").disabled(true),
            TreeNode::new("Child", "child").checkable(true),
        ]),
        TreeNode::new("Other", "other"),
    ];
    let mut tree = Tree::new(nodes);

    assert_eq!(WidgetComponent::tab_index(&tree), 1);
    assert_eq!(tree.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(
        tree.on_event(&key_event(KeyCode::Down)),
        EventResult::Handled
    );
    assert_eq!(tree.selected_key(), "root");

    tree.on_event(&key_event(KeyCode::Right));
    assert!(EventHandler::take_layout_request(&mut tree));
    assert!(!EventHandler::take_layout_request(&mut tree));
    tree.on_event(&key_event(KeyCode::Right));
    assert_eq!(tree.selected_key(), "child");

    tree.on_event(&key_event(KeyCode::Space));
    assert!(matches!(
        tree.snapshot_fields(),
        SnapshotFields::Tree {
            nodes,
            selected_key,
            expanded_keys,
            ..
        } if nodes[0].children[1].checked
            && selected_key == "child"
            && expanded_keys == ["root"]
    ));

    tree.on_event(&key_event(KeyCode::Left));
    assert_eq!(tree.selected_key(), "root");
    tree.on_event(&key_event(KeyCode::Left));
    assert!(matches!(
        tree.snapshot_fields(),
        SnapshotFields::Tree { expanded_keys, .. } if expanded_keys.is_empty()
    ));
}

#[test]
fn tree_keyboard_navigation_skips_disabled_rows_and_reveals_selection() {
    let nodes = vec![
        TreeNode::new("First", "first"),
        TreeNode::new("Disabled", "disabled").disabled(true),
        TreeNode::new("Third", "third"),
        TreeNode::new("Fourth", "fourth"),
    ];
    let mut tree = Tree::new(nodes);
    tree.last_frame.set(Some(Rect::new(0.0, 0.0, 200.0, 56.0)));

    tree.on_event(&key_event(KeyCode::Down));
    tree.on_event(&key_event(KeyCode::Down));
    assert_eq!(tree.selected_key(), "third");
    assert!(tree.body_scroll.scroll_offset() > 0.0);
    assert_eq!(
        EventHandler::viewport_scroll_offset(&tree),
        Some((0.0, tree.body_scroll.scroll_offset()))
    );
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&tree),
        Some((0.0, 28.0))
    );
}

#[test]
fn tree_selection_emits_structured_change_event() {
    let mut tree = Tree::new(vec![TreeNode::new("First", "first")]);
    let event = key_event(KeyCode::Down);
    tree.on_event(&event);

    let semantic = tree
        .semantic_event(ComponentId::new(7), &event)
        .expect("keyboard selection should emit change");
    assert_eq!(semantic.text_payload(), Some("first"));
}

#[test]
fn tree_pointer_actions_require_two_dimensional_matching_release() {
    let nodes = vec![TreeNode::new("Root", "root").children(vec![TreeNode::new(
        "Checkable child",
        "child",
    )
    .checkable(true)])];
    let mut tree = Tree::new(nodes);
    tree.last_frame.set(Some(Rect::new(0.0, 0.0, 100.0, 56.0)));

    assert_eq!(
        tree.on_event(&pointer_down(Point::new(110.0, 10.0))),
        EventResult::NotHandled
    );
    assert_eq!(
        tree.on_event(&pointer_down(Point::new(10.0, 10.0))),
        EventResult::Handled
    );
    assert!(matches!(
        tree.snapshot_fields(),
        SnapshotFields::Tree { expanded_keys, .. } if expanded_keys.is_empty()
    ));
    assert_eq!(
        tree.on_event(&pointer_up(Point::new(110.0, 10.0))),
        EventResult::Handled
    );
    assert!(matches!(
        tree.snapshot_fields(),
        SnapshotFields::Tree { expanded_keys, .. } if expanded_keys.is_empty()
    ));

    assert_eq!(
        tree.on_event(&pointer_down(Point::new(10.0, 10.0))),
        EventResult::Handled
    );
    assert_eq!(
        tree.on_event(&pointer_up(Point::new(10.0, 10.0))),
        EventResult::Handled
    );
    assert!(matches!(
        tree.snapshot_fields(),
        SnapshotFields::Tree { expanded_keys, .. } if expanded_keys == ["root"]
    ));

    let check = Point::new(25.0, 42.0);
    assert_eq!(tree.on_event(&pointer_down(check)), EventResult::Handled);
    assert!(matches!(
        tree.snapshot_fields(),
        SnapshotFields::Tree { nodes, .. } if !nodes[0].children[0].checked
    ));
    assert_eq!(tree.on_event(&pointer_up(check)), EventResult::Handled);
    assert!(matches!(
        tree.snapshot_fields(),
        SnapshotFields::Tree { nodes, .. } if nodes[0].children[0].checked
    ));
}

#[test]
fn tree_hover_only_handles_changes_and_leave_cancels_pressed_action() {
    let mut tree = Tree::new(vec![TreeNode::new("Root", "root")]);
    tree.last_frame.set(Some(Rect::new(0.0, 0.0, 100.0, 28.0)));
    let inside = SystemEvent::PointerMove {
        pos: Point::new(50.0, 14.0),
        mods: KeyMod::NONE,
    };

    assert_eq!(tree.on_event(&inside), EventResult::Handled);
    assert_eq!(tree.on_event(&inside), EventResult::NotHandled);
    assert_eq!(
        tree.on_event(&pointer_down(Point::new(50.0, 14.0))),
        EventResult::Handled
    );
    assert_eq!(
        tree.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(
        tree.on_event(&pointer_up(Point::new(50.0, 14.0))),
        EventResult::NotHandled
    );
    assert!(tree.selected_key().is_empty());
}

#[test]
fn constrained_tree_clips_elides_and_avoids_negative_geometry() {
    let nodes = vec![
        TreeNode::new("很长的根节点标题 mixed root", "root").children(vec![TreeNode::new(
            "很长的子节点标题 mixed child",
            "child",
        )
        .children(vec![TreeNode::new(
            "很长的孙节点标题 mixed grandchild",
            "grandchild",
        )])]),
    ];
    let mut tree = Tree::new(nodes);
    tree.on_event(&key_event(KeyCode::Down));
    tree.on_event(&key_event(KeyCode::Right));
    tree.on_event(&key_event(KeyCode::Right));
    tree.on_event(&key_event(KeyCode::Right));

    let display_list = render_tree(&tree, Rect::new(10.0, 8.0, 72.0, 45.0), (100, 70));
    assert!(display_list.contains('…'), "{display_list}");
    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 8.0, w: 72.0, h: 45.0 } }"),
        "{display_list}"
    );
    assert!(!display_list.contains("NaN"), "{display_list}");
    assert!(
        !display_list.contains("w: -") && !display_list.contains("h: -"),
        "{display_list}"
    );

    let invalid = render_tree(
        &Tree::new(vec![TreeNode::new("invalid", "invalid")]),
        Rect::new(0.0, 0.0, f32::NAN, -10.0),
        (20, 20),
    );
    assert_eq!(invalid, "DisplayList { ops: [] }");
}

#[test]
fn checkable_tree_rows_use_shared_icons_instead_of_text_boxes() {
    let mut checked_node = TreeNode::new("Checked", "checked").checkable(true);
    checked_node.checked = true;
    let tree = Tree::new(vec![
        checked_node,
        TreeNode::new("Unchecked", "unchecked").checkable(true),
    ]);

    let display_list = render_tree(&tree, Rect::new(0.0, 0.0, 180.0, 56.0), (180, 56));
    assert!(
        display_list.contains("\\u{e16a}") && display_list.contains("\\u{e167}"),
        "checked and unchecked states must use Lucide icons: {display_list}"
    );
    assert!(
        !display_list.contains("[x]") && !display_list.contains("[ ]"),
        "text checkbox symbols must not return: {display_list}"
    );
}

#[test]
fn tree_accessibility_reports_selected_visible_title_and_position() {
    let mut tree = Tree::new(vec![
        TreeNode::new("Root", "root").children(vec![TreeNode::new("Readable child", "child")])
    ]);
    tree.on_event(&key_event(KeyCode::Down));
    tree.on_event(&key_event(KeyCode::Right));
    tree.on_event(&key_event(KeyCode::Right));

    let accessibility = tree.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Tree);
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("Readable child")
    );
    assert_eq!(accessibility.state.value_now, Some(2.0));
    assert_eq!(accessibility.state.value_min, Some(1.0));
    assert_eq!(accessibility.state.value_max, Some(2.0));
}

#[test]
fn tree_node_filter_controls_search_matching() {
    let mut tree = Tree::new(vec![
        TreeNode::new("Only custom", "custom-x")
            .filter(|node, keyword| node.key == format!("custom-{keyword}")),
        TreeNode::new("Hidden label", "beta"),
    ])
    .searchable(true);

    tree.set_search_query("x");
    assert_eq!(tree.flat.len(), 1);
    assert_eq!(tree.visible_keys_for_test(), ["custom-x"]);

    tree.set_search_query("hidden");
    assert_eq!(tree.flat.len(), 1);
    assert_eq!(tree.visible_keys_for_test(), ["beta"]);

    tree.set_search_query("beta");
    assert!(
        tree.visible_keys_for_test().is_empty(),
        "the default filter matches the visible title, not the business key"
    );
}

#[test]
fn searchable_tree_consumes_text_preserves_selection_and_reconciles_runtime_query() {
    let mut tree = Tree::new(vec![
        TreeNode::new("Root", "root").children(vec![
            TreeNode::new("Alpha", "alpha-id"),
            TreeNode::new("Beta", "beta-id"),
        ]),
        TreeNode::new("Other", "other"),
    ])
    .searchable(true);
    tree.set_selected_key("beta-id");
    tree.last_frame.set(Some(Rect::new(0.0, 0.0, 200.0, 88.0)));

    assert!(WidgetTextInput::accepts_text_input(&tree));
    assert_eq!(
        tree.on_event(&SystemEvent::TextInput {
            text: "BETA".to_string(),
        }),
        EventResult::Handled
    );
    assert_eq!(tree.search_query(), "BETA");
    assert_eq!(tree.visible_keys_for_test(), ["root", "beta-id"]);
    assert_eq!(tree.selected_key(), "beta-id");
    assert!(EventHandler::take_layout_request(&mut tree));
    let display = render_tree(&tree, Rect::new(0.0, 0.0, 200.0, 88.0), (200, 88));
    assert!(
        display.contains("BETA") && display.contains("Beta"),
        "{display}"
    );
    assert!(!display.contains("Alpha"), "{display}");
    assert_eq!(
        tree.on_event(&SystemEvent::Wheel {
            pos: Point::new(20.0, 16.0),
            delta: Point::new(0.0, 1.0),
        }),
        EventResult::NotHandled,
        "the search editor must not leak wheel events into the tree body"
    );

    tree.sync_from(
        Tree::new(vec![TreeNode::new("Root next", "root")
            .children(vec![TreeNode::new("Beta next", "beta-id")])])
        .searchable(true),
    );
    assert_eq!(tree.search_query(), "BETA");
    assert_eq!(tree.visible_keys_for_test(), ["root", "beta-id"]);
    assert_eq!(tree.selected_key(), "beta-id");

    tree.sync_from(Tree::new(vec![TreeNode::new("Plain", "plain")]).searchable(false));
    assert!(!WidgetTextInput::accepts_text_input(&tree));
    assert!(tree.search_query().is_empty());
    assert_eq!(tree.visible_keys_for_test(), ["plain"]);
    assert!(EventHandler::take_layout_request(&mut tree));
}

#[test]
fn draggable_tree_reports_before_inside_after_with_search_header_offset() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let drops_for_callback = Rc::clone(&drops);
    let mut tree = Tree::new(vec![
        TreeNode::new("Source", "source"),
        TreeNode::new("Target", "target"),
    ])
    .searchable(true)
    .draggable(true)
    .on_drop(move |source, target, position| {
        drops_for_callback
            .borrow_mut()
            .push((source.to_string(), target.to_string(), position));
    });
    tree.last_frame.set(Some(Rect::new(0.0, 0.0, 200.0, 88.0)));

    for (target_y, expected) in [
        (61.0, DropPosition::Before),
        (74.0, DropPosition::Inside),
        (86.0, DropPosition::After),
    ] {
        assert_eq!(
            tree.on_event(&pointer_down(Point::new(100.0, 46.0))),
            EventResult::Handled
        );
        assert_eq!(
            tree.on_event(&pointer_up(Point::new(100.0, target_y))),
            EventResult::Handled
        );
        assert_eq!(drops.borrow().last().map(|drop| drop.2), Some(expected));
    }
    assert!(drops
        .borrow()
        .iter()
        .all(|(source, target, _)| { source == "source" && target == "target" }));
}

#[test]
fn tree_drag_cancels_on_focus_loss_and_reconcile_replaces_the_callback() {
    let old_calls = Rc::new(Cell::new(0));
    let old_calls_for_callback = Rc::clone(&old_calls);
    let new_calls = Rc::new(Cell::new(0));
    let new_calls_for_callback = Rc::clone(&new_calls);
    let nodes = || {
        vec![
            TreeNode::new("Source", "source"),
            TreeNode::new("Target", "target"),
        ]
    };
    let mut tree = Tree::new(nodes())
        .draggable(true)
        .on_drop(move |_, _, _| old_calls_for_callback.set(old_calls_for_callback.get() + 1));
    tree.last_frame.set(Some(Rect::new(0.0, 0.0, 200.0, 56.0)));

    tree.on_event(&pointer_down(Point::new(100.0, 14.0)));
    assert_eq!(tree.on_event(&SystemEvent::FocusOut), EventResult::Handled);
    assert_eq!(
        tree.on_event(&pointer_up(Point::new(100.0, 42.0))),
        EventResult::NotHandled
    );
    assert_eq!(old_calls.get(), 0);

    tree.on_event(&pointer_down(Point::new(100.0, 14.0)));
    tree.sync_from(
        Tree::new(nodes())
            .draggable(true)
            .on_drop(move |_, _, _| new_calls_for_callback.set(new_calls_for_callback.get() + 1)),
    );
    assert_eq!(
        tree.on_event(&pointer_up(Point::new(100.0, 42.0))),
        EventResult::NotHandled,
        "reconcile must cancel an in-flight drag"
    );
    tree.on_event(&pointer_down(Point::new(100.0, 14.0)));
    tree.on_event(&pointer_up(Point::new(100.0, 42.0)));
    assert_eq!(old_calls.get(), 0);
    assert_eq!(new_calls.get(), 1);
}

#[test]
fn lazy_tree_expands_once_patches_children_and_does_not_reload_after_reopen() {
    let expanded = Rc::new(RefCell::new(Vec::new()));
    let expanded_for_callback = Rc::clone(&expanded);
    let mut tree = Tree::new(vec![TreeNode::new("Lazy", "lazy").lazy(true)])
        .on_expand(move |key| expanded_for_callback.borrow_mut().push(key.to_string()));
    tree.set_selected_key("lazy");

    assert_eq!(
        tree.on_event(&key_event(KeyCode::Right)),
        EventResult::Handled
    );
    assert_eq!(expanded.borrow().as_slice(), ["lazy"]);
    assert!(matches!(
        tree.snapshot_fields(),
        SnapshotFields::Tree { expanded_keys, .. } if expanded_keys == ["lazy"]
    ));
    tree.on_event(&key_event(KeyCode::Right));
    assert_eq!(
        expanded.borrow().len(),
        1,
        "an already-open node must not reload"
    );

    assert!(tree.patch_children("lazy", vec![TreeNode::new("Child", "child")]));
    assert!(!tree.patch_children("missing", Vec::new()));
    assert_eq!(tree.visible_keys_for_test(), ["lazy", "child"]);
    assert!(EventHandler::take_layout_request(&mut tree));

    tree.on_event(&key_event(KeyCode::Left));
    tree.on_event(&key_event(KeyCode::Right));
    assert_eq!(
        expanded.borrow().len(),
        1,
        "a loaded lazy node must stay loaded"
    );
}

#[test]
fn lazy_expand_callback_is_replaced_by_reconcile_before_first_expansion() {
    let old_calls = Rc::new(Cell::new(0));
    let old_calls_for_callback = Rc::clone(&old_calls);
    let new_calls = Rc::new(Cell::new(0));
    let new_calls_for_callback = Rc::clone(&new_calls);
    let mut tree = Tree::new(vec![TreeNode::new("Lazy", "lazy").lazy(true)])
        .on_expand(move |_| old_calls_for_callback.set(old_calls_for_callback.get() + 1));
    tree.set_selected_key("lazy");
    tree.sync_from(
        Tree::new(vec![TreeNode::new("Lazy next", "lazy").lazy(true)])
            .on_expand(move |_| new_calls_for_callback.set(new_calls_for_callback.get() + 1)),
    );

    tree.on_event(&key_event(KeyCode::Right));
    assert_eq!(old_calls.get(), 0);
    assert_eq!(new_calls.get(), 1);
}

#[test]
fn tree_node_icon_renders_selects_and_updates_on_reconcile() {
    let mut tree = Tree::new(vec![TreeNode::new("Favorite", "favorite").icon("star")]);
    tree.last_frame.set(Some(Rect::new(0.0, 0.0, 180.0, 28.0)));
    let star = render_tree(&tree, Rect::new(0.0, 0.0, 180.0, 28.0), (180, 28));
    assert!(star.contains("\\u{e176}"), "{star}");

    assert_eq!(
        tree.on_event(&pointer_down(Point::new(10.0, 14.0))),
        EventResult::Handled
    );
    assert_eq!(
        tree.on_event(&pointer_up(Point::new(10.0, 14.0))),
        EventResult::Handled
    );
    assert_eq!(tree.selected_key(), "favorite");

    tree.sync_from(Tree::new(vec![
        TreeNode::new("Favorite next", "favorite").icon("heart")
    ]));
    let heart = render_tree(&tree, Rect::new(0.0, 0.0, 180.0, 28.0), (180, 28));
    assert!(heart.contains("\\u{e0f2}"), "{heart}");
    assert!(!heart.contains("\\u{e176}"), "{heart}");
    assert_eq!(tree.selected_key(), "favorite");
}
