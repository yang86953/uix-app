use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::display::tree::*;
use crate::ui::AccessibilityRole;

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
    let mut display_list = crate::draw::painting::DisplayList::new();
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
