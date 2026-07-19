use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::display::tree::TreeNode;
use crate::ui::widgets::input::tree_select::*;
use crate::ui::AccessibilityRole;

fn render_tree_select(tree_select: &TreeSelect, frame: Rect) -> (Vec<u32>, usize) {
    const WIDTH: i32 = 360;
    const HEIGHT: i32 = 360;
    let mut canvas = CpuCanvas2D::new(PixelSurface::new(WIDTH, HEIGHT));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
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
            WIDTH,
            HEIGHT,
        );
        WidgetRender::render(tree_select, frame, &mut ctx, &tree);
    }
    (canvas.surface().pixels().to_vec(), WIDTH as usize)
}

fn click(tree_select: &mut TreeSelect, x: f32, y: f32) -> EventResult {
    tree_select.on_event(&SystemEvent::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    })
}

fn large_tree_select() -> TreeSelect {
    let nodes: Vec<TreeNode> = (0..80)
        .map(|i| TreeNode::new(&format!("Node {i}"), &format!("n-{i}")))
        .collect();
    TreeSelect::new().nodes(nodes)
}

#[test]
fn tree_select_dropdown_scroll_range_limits_visible_rows() {
    let tree_select = large_tree_select();
    let row_count = tree_select.flatten_nodes().len();
    let viewport_h = tree_select.dropdown_viewport_height(row_count);
    let (start, end) = tree_select
        .dropdown_scroll
        .scroll_range(row_count, 28.0, viewport_h);
    assert_eq!(start, 0);
    assert!(
        end - start < 80,
        "virtual scroll should expose a small window"
    );
}

#[test]
fn tree_select_dropdown_wheel_records_composite_delta() {
    let mut tree_select = large_tree_select();
    tree_select.open();

    assert_eq!(
        EventHandler::on_event(
            &mut tree_select,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 50.0),
                delta: Point::new(0.0, 1.0),
            },
        ),
        EventResult::Handled
    );
    assert!(tree_select.dropdown_scroll.scroll_offset() > 0.0);
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&tree_select),
        Some((0.0, 40.0))
    );
}

#[test]
fn tree_select_dropdown_row_at_y_accounts_for_scroll_offset() {
    let mut tree_select = large_tree_select();
    tree_select.open();
    tree_select.dropdown_scroll.set_scroll_offset(28.0 * 5.0);

    assert_eq!(tree_select.dropdown_row_at_y(33.0), Some(5));
    assert_eq!(tree_select.dropdown_row_at_y(61.0), Some(6));
}

#[test]
fn tree_select_keyboard_skips_disabled_nodes_and_commits_with_enter() {
    let mut tree_select = TreeSelect::new().nodes(vec![
        TreeNode::new("Disabled", "disabled").disabled(true),
        TreeNode::new("Alpha", "alpha"),
        TreeNode::new("Beta", "beta"),
    ]);

    assert_eq!(
        tree_select.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let _ = tree_select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    let _ = tree_select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });

    assert_eq!(tree_select.value(), "Beta");
    assert_eq!(tree_select.value_key(), "beta");
    assert!(!tree_select.is_open());
    let semantic = tree_select
        .semantic_event(ComponentId::new(17), &SystemEvent::FocusIn)
        .expect("TreeSelect selection should emit its stable node key");
    assert_eq!(semantic.text_payload(), Some("beta"));
    let accessibility = tree_select.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Combobox);
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Beta"));
    assert_eq!(accessibility.state.expanded, Some(false));
}

#[test]
fn tree_select_disabled_pointer_row_stays_open_and_unselected() {
    let mut tree_select = TreeSelect::new().nodes(vec![
        TreeNode::new("Disabled", "disabled").disabled(true),
        TreeNode::new("Enabled", "enabled"),
    ]);
    tree_select.open();

    assert_eq!(
        tree_select.on_event(&SystemEvent::PointerDown {
            pos: Point::new(8.0, 33.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(tree_select.is_open());
    assert!(tree_select.value_key().is_empty());
}

#[test]
fn tree_select_keyboard_reveals_highlight_in_long_dropdown() {
    let mut tree_select = large_tree_select();
    tree_select.open();
    for _ in 0..20 {
        let _ = tree_select.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        });
    }

    assert!(tree_select.dropdown_scroll.scroll_offset() > 0.0);
    assert!(EventHandler::scroll_delta_for_dirty(&tree_select).is_some());
}

#[test]
fn tree_select_focus_out_closes_and_snapshot_preserves_current_value() {
    let mut tree_select = TreeSelect::new().nodes(vec![TreeNode::new("Alpha", "alpha")]);
    tree_select.open();
    let _ = tree_select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    tree_select.open();
    let _ = tree_select.on_event(&SystemEvent::FocusOut);

    assert!(!tree_select.is_open());
    assert!(matches!(
        tree_select.snapshot_fields(),
        SnapshotFields::TreeSelect {
            value,
            value_key,
            open: false,
            ..
        } if value == "Alpha" && value_key == "alpha"
    ));
}

#[test]
fn tree_select_constrained_trigger_clips_text_and_uses_actual_hit_frame() {
    let mut tree_select = TreeSelect::new()
        .placeholder("A very long placeholder that must stay before the trigger arrow")
        .nodes(vec![TreeNode::new(
            "A very long hierarchical option that must stay inside the popup",
            "long",
        )]);
    let frame = Rect::new(0.0, 0.0, 60.0, 10.0);
    let (pixels, stride) = render_tree_select(&tree_select, frame);
    for y in 0..360usize {
        for x in 0..360usize {
            if x >= 60 || y >= 10 {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "closed constrained TreeSelect leaked paint at ({x}, {y})"
                );
            }
        }
    }

    assert_eq!(click(&mut tree_select, 5.0, 20.0), EventResult::NotHandled);
    assert_eq!(click(&mut tree_select, 70.0, 5.0), EventResult::NotHandled);
    assert!(!tree_select.is_open());

    assert_eq!(click(&mut tree_select, 5.0, 5.0), EventResult::Handled);
    let _ = WidgetAnimation::update_animation(&mut tree_select, 1.0);
    let (pixels, stride) = render_tree_select(&tree_select, frame);
    let bounds = EventHandler::hit_test_frame(&tree_select, frame);
    assert_eq!(bounds.w, 200.0);
    assert_eq!(bounds.h, 38.0);
    for y in 0..360usize {
        for x in 0..360usize {
            if x >= bounds.w as usize || y >= bounds.h as usize {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "open constrained TreeSelect leaked paint at ({x}, {y}); bounds={bounds:?}"
                );
            }
        }
    }
}

#[test]
fn tree_select_pointer_hit_requires_popup_x_and_visible_y() {
    let mut tree_select = large_tree_select();
    let _ = render_tree_select(&tree_select, Rect::new(0.0, 0.0, 200.0, 32.0));

    assert_eq!(click(&mut tree_select, 5.0, 5.0), EventResult::Handled);
    assert_eq!(
        click(&mut tree_select, 220.0, 45.0),
        EventResult::NotHandled
    );
    assert!(tree_select.value_key().is_empty());
    assert!(!tree_select.is_open());

    assert_eq!(click(&mut tree_select, 5.0, 5.0), EventResult::Handled);
    assert_eq!(
        click(&mut tree_select, 10.0, 313.0),
        EventResult::NotHandled
    );
    assert!(tree_select.value_key().is_empty());
    assert!(!tree_select.is_open());
    assert_eq!(tree_select.dropdown_row_at_y(313.0), None);
}

#[test]
fn tree_select_empty_state_has_a_popover_overlay() {
    let mut tree_select = TreeSelect::new().placeholder("Nothing selected");
    let frame = Rect::new(20.0, 40.0, 80.0, 12.0);
    let id = ComponentId::new(41);
    assert!(WidgetRender::overlay_entry(&tree_select, id, frame).is_none());

    tree_select.open();
    let overlay = WidgetRender::overlay_entry(&tree_select, id, frame)
        .expect("an empty open TreeSelect must still expose its no-data popup");
    assert_eq!(overlay.kind(), OverlayKind::Popover);
    assert_eq!(
        overlay.bounds_rect(),
        Some(EventHandler::hit_test_frame(&tree_select, frame))
    );
    assert_eq!(overlay.bounds_rect().expect("overlay bounds").w, 200.0);
    assert_eq!(overlay.bounds_rect().expect("overlay bounds").h, 40.0);
    assert_eq!(
        tree_select.snapshot_fields().accessibility().state.expanded,
        Some(true)
    );

    let _ = WidgetAnimation::update_animation(&mut tree_select, 1.0);
    let (pixels, stride) = render_tree_select(&tree_select, Rect::new(0.0, 0.0, 80.0, 12.0));
    assert!(
        (12..40).any(|y| (0..200).any(|x| pixels[y * stride + x] != 0)),
        "empty TreeSelect must render a visible localized no-data row"
    );
}

#[test]
fn tree_select_pointer_hover_only_handles_visible_changes() {
    let mut tree_select = TreeSelect::new().nodes(vec![
        TreeNode::new("Alpha", "alpha"),
        TreeNode::new("Beta", "beta"),
    ]);
    let _ = render_tree_select(&tree_select, Rect::new(0.0, 0.0, 200.0, 32.0));
    tree_select.open();
    let second_row = SystemEvent::PointerMove {
        pos: Point::new(20.0, 74.0),
        mods: KeyMod::NONE,
    };
    assert_eq!(tree_select.on_event(&second_row), EventResult::Handled);
    assert_eq!(tree_select.on_event(&second_row), EventResult::NotHandled);

    let outside = SystemEvent::PointerMove {
        pos: Point::new(240.0, 74.0),
        mods: KeyMod::NONE,
    };
    assert_eq!(tree_select.on_event(&outside), EventResult::Handled);
    assert_eq!(tree_select.on_event(&outside), EventResult::NotHandled);
    assert_eq!(
        tree_select.on_event(&SystemEvent::PointerLeave),
        EventResult::NotHandled
    );
}

#[test]
fn tree_select_home_end_and_identical_reconcile_preserve_highlight() {
    let nodes = vec![
        TreeNode::new("Disabled first", "disabled-first").disabled(true),
        TreeNode::new("Alpha", "alpha"),
        TreeNode::new("Beta", "beta"),
        TreeNode::new("Disabled last", "disabled-last").disabled(true),
    ];
    let mut tree_select = TreeSelect::new().nodes(nodes.clone());
    tree_select.open();
    assert_eq!(
        tree_select.on_event(&SystemEvent::KeyDown {
            key: KeyCode::End,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );

    tree_select.sync_from(TreeSelect::new().nodes(nodes));
    let _ = tree_select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(tree_select.value(), "Beta");
    assert_eq!(tree_select.value_key(), "beta");
}

#[test]
fn tree_select_close_preserves_scroll_during_exit_animation() {
    let mut tree_select = large_tree_select();
    tree_select.open();
    let _ = tree_select.on_event(&SystemEvent::Wheel {
        pos: Point::new(10.0, 50.0),
        delta: Point::new(0.0, 3.0),
    });
    let before_close = tree_select.dropdown_scroll.scroll_offset();
    assert!(before_close > 0.0);

    let _ = tree_select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Escape,
        mods: KeyMod::NONE,
    });
    assert!(tree_select.is_present());
    assert_eq!(tree_select.dropdown_scroll.scroll_offset(), before_close);

    let _ = WidgetAnimation::update_animation(&mut tree_select, 1.0);
    tree_select.open();
    assert_eq!(tree_select.dropdown_scroll.scroll_offset(), 0.0);
}

#[test]
fn tree_select_reopening_reveals_the_selected_node() {
    let mut tree_select = large_tree_select();
    tree_select.open();
    let _ = tree_select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::End,
        mods: KeyMod::NONE,
    });
    let _ = tree_select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(tree_select.value_key(), "n-79");

    let _ = WidgetAnimation::update_animation(&mut tree_select, 1.0);
    tree_select.open();
    assert!(
        tree_select.dropdown_scroll.scroll_offset() > 0.0,
        "reopening must reveal the selected row instead of jumping to the root"
    );
}
