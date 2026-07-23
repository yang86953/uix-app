use crate::draw::backend::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Cascader, CascaderOption, CascaderValue};

fn render_cascader(cascader: &Cascader, frame: Rect) {
    let _ = render_cascader_pixels(cascader, frame);
}

fn render_cascader_pixels(cascader: &Cascader, frame: Rect) -> (Vec<u32>, usize) {
    const WIDTH: i32 = 520;
    const HEIGHT: i32 = 280;
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
        WidgetRender::render(cascader, frame, &mut ctx, &tree);
    }
    (canvas.surface().pixels().to_vec(), WIDTH as usize)
}

fn click(cascader: &mut Cascader, x: f32, y: f32) -> EventResult {
    cascader.on_event(&SystemEvent::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    })
}

fn cascader() -> Cascader {
    Cascader::new(
        vec![
            CascaderOption::new("Unavailable", "unavailable").disabled(true),
            CascaderOption::new("China", "china").children(vec![
                CascaderOption::new("Unavailable city", "unavailable-city").disabled(true),
                CascaderOption::new("Beijing", "beijing"),
            ]),
            CascaderOption::new("Singapore", "singapore"),
        ],
        "Region",
    )
}

#[test]
fn cascader_keyboard_enters_levels_and_skips_disabled_options() {
    let mut cascader = cascader();
    assert_eq!(
        cascader.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(cascader.is_open());

    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["china"]);
    assert!(cascader.is_open());

    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["china", "beijing"]);
    assert!(!cascader.is_open());
    let semantic = cascader
        .semantic_event(ComponentId::new(3), &SystemEvent::FocusIn)
        .expect("leaf selection should emit the selected value path");
    assert_eq!(semantic.text_payload(), Some("china/beijing"));
    let accessibility = cascader.snapshot_fields().accessibility();
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("China / Beijing")
    );
    assert_eq!(accessibility.state.expanded, Some(false));
}

#[test]
fn cascader_arrow_navigation_wraps_across_enabled_root_options() {
    let mut cascader = cascader();
    cascader.open();
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["singapore"]);

    cascader.open();
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Up,
        mods: KeyMod::NONE,
    });
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["singapore"]);
}

#[test]
fn cascader_reconcile_rebuilds_open_navigation_from_new_options() {
    let mut cascader = cascader();
    cascader.open();
    cascader.sync_from(Cascader::new(
        vec![CascaderOption::new("Japan", "japan")],
        "New region",
    ));

    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["japan"]);
    assert!(matches!(
        cascader.snapshot_fields(),
        SnapshotFields::Cascader {
            selected_values,
            open: false,
            ..
        } if selected_values == ["japan"]
    ));
}

#[test]
fn cascader_identical_reconcile_preserves_open_child_level() {
    let mut widget = cascader();
    widget.open();
    widget.select_option(0, 1);
    assert_eq!(widget.selected().values, ["china"]);

    widget.sync_from(cascader());
    let _ = widget.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });

    assert_eq!(widget.selected().values, ["china", "beijing"]);
    assert!(!widget.is_open());
}

#[test]
fn loading_child_keeps_the_path_open_and_resumes_after_reconcile() {
    let mut widget = cascader()
        .loading_child("singapore", true)
        .loading_child("china", true);
    assert!(matches!(
        widget.snapshot_fields(),
        SnapshotFields::Cascader {
            loading_children,
            ..
        } if loading_children == ["china", "singapore"]
    ));
    assert!(!WidgetAnimation::update_animation(&mut widget, 0.1));

    widget.open();
    widget.select_option(0, 1);
    assert_eq!(widget.selected().values, ["china"]);
    assert!(widget.is_open());
    assert_eq!(
        EventHandler::hit_test_frame(&widget, Rect::new(0.0, 0.0, 120.0, 32.0)).w,
        200.0,
        "a loading branch must not expose stale child columns"
    );
    assert!(widget
        .semantic_event(ComponentId::new(8), &SystemEvent::FocusIn)
        .is_none());

    assert!(WidgetAnimation::update_animation(&mut widget, 1.0));
    let (spinner_before, _) = render_cascader_pixels(&widget, Rect::new(0.0, 0.0, 120.0, 32.0));
    assert!(WidgetAnimation::update_animation(&mut widget, 0.1));
    assert_ne!(
        WidgetAnimation::dirty_bounds(&widget, Rect::new(0.0, 0.0, 120.0, 32.0)),
        Rect::zero()
    );
    let (spinner_after, _) = render_cascader_pixels(&widget, Rect::new(0.0, 0.0, 120.0, 32.0));
    assert_ne!(
        spinner_before, spinner_after,
        "the visible loading child must paint its advancing spinner phase"
    );

    widget.sync_from(cascader());
    assert!(matches!(
        widget.snapshot_fields(),
        SnapshotFields::Cascader {
            loading_children,
            ..
        } if loading_children.is_empty()
    ));
    assert_eq!(
        widget.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        EventHandler::hit_test_frame(&widget, Rect::new(0.0, 0.0, 120.0, 32.0)).w,
        400.0
    );
    assert!(!WidgetAnimation::update_animation(&mut widget, 0.1));
}

#[test]
fn loading_leaf_does_not_submit_or_close() {
    let mut widget = cascader().loading_child("singapore", true);
    widget.open();
    widget.select_option(0, 2);

    assert_eq!(widget.selected().values, ["singapore"]);
    assert!(widget.is_open());
    assert!(widget
        .semantic_event(ComponentId::new(9), &SystemEvent::FocusIn)
        .is_none());
}

#[test]
fn searchable_filters_leaf_paths_and_commits_the_full_value_path() {
    let mut widget = cascader().searchable(true);
    assert!(widget.as_text_input().is_some());
    assert!(matches!(
        widget.snapshot_fields(),
        SnapshotFields::Cascader {
            searchable: true,
            search_query,
            search_results,
            ..
        } if search_query.is_empty() && search_results.is_empty()
    ));

    widget.open();
    widget.select_option(0, 1);
    assert_eq!(
        EventHandler::hit_test_frame(&widget, Rect::new(0.0, 0.0, 120.0, 32.0)).w,
        400.0
    );
    let _ = widget.on_event(&SystemEvent::FocusIn);
    assert_eq!(
        widget.on_event(&SystemEvent::TextInput {
            text: "bei".to_owned(),
        }),
        EventResult::Handled
    );
    assert!(widget.is_open());
    assert!(matches!(
        widget.snapshot_fields(),
        SnapshotFields::Cascader {
            search_query,
            search_results,
            ..
        } if search_query == "bei"
            && search_results == [CascaderValue {
                labels: vec!["China".to_owned(), "Beijing".to_owned()],
                values: vec!["china".to_owned(), "beijing".to_owned()],
            }]
    ));
    assert_eq!(
        EventHandler::hit_test_frame(&widget, Rect::new(0.0, 0.0, 120.0, 32.0)).w,
        200.0,
        "search results use one path-projection column"
    );
    assert_eq!(
        WidgetRender::dirty_rect(&widget, Rect::new(0.0, 0.0, 120.0, 32.0)).w,
        400.0,
        "search projection damage must clear the previously visible child column"
    );
    assert!(widget
        .semantic_event(ComponentId::new(10), &SystemEvent::FocusIn)
        .is_none());

    render_cascader(&widget, Rect::new(0.0, 0.0, 120.0, 32.0));
    assert_eq!(click(&mut widget, 20.0, 50.0), EventResult::Handled);
    assert_eq!(widget.selected().values, ["china", "beijing"]);
    assert!(!widget.is_open());
    assert_eq!(
        widget
            .semantic_event(ComponentId::new(10), &SystemEvent::FocusIn)
            .expect("a search result commit must emit the full value path")
            .text_payload(),
        Some("china/beijing")
    );
}

#[test]
fn searchable_result_wheel_reaches_paths_below_the_viewport() {
    let options = (0..10)
        .map(|index| {
            CascaderOption::new(format!("Region {index}"), format!("region-{index}")).children(
                vec![CascaderOption::new(
                    format!("City {index}"),
                    format!("city-{index}"),
                )],
            )
        })
        .collect();
    let mut widget = Cascader::new(options, "Region").searchable(true);
    render_cascader(&widget, Rect::new(0.0, 0.0, 120.0, 32.0));
    let _ = widget.on_event(&SystemEvent::FocusIn);
    let _ = widget.on_event(&SystemEvent::TextInput {
        text: "city".to_owned(),
    });
    assert_eq!(
        widget.on_event(&SystemEvent::Wheel {
            pos: Point::new(20.0, 100.0),
            delta: Point::new(0.0, 100.0),
        }),
        EventResult::Handled
    );
    assert_eq!(click(&mut widget, 20.0, 220.0), EventResult::Handled);
    assert_eq!(widget.selected().values, ["region-9", "city-9"]);
}

#[test]
fn searchable_query_edits_unicode_at_the_measured_cursor() {
    let mut widget =
        Cascader::new(vec![CascaderOption::new("测试", "test")], "Region").searchable(true);
    let _ = widget.on_event(&SystemEvent::FocusIn);
    let _ = widget.on_event(&SystemEvent::TextInput {
        text: "测试".to_owned(),
    });
    render_cascader(&widget, Rect::new(0.0, 0.0, 200.0, 32.0));
    let cursor = widget
        .as_text_input()
        .expect("searchable Cascader text input capability")
        .text_input_cursor_rect();
    assert!(cursor.x > 12.0, "cursor must follow measured query text");

    let _ = widget.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    let _ = widget.on_event(&SystemEvent::TextInput {
        text: "A".to_owned(),
    });
    assert!(matches!(
        widget.snapshot_fields(),
        SnapshotFields::Cascader { search_query, .. } if search_query == "测A试"
    ));

    render_cascader(&widget, Rect::new(0.0, 0.0, 200.0, 32.0));
    assert_eq!(click(&mut widget, 12.0, 16.0), EventResult::Handled);
    assert!(
        widget.is_open(),
        "clicking the search input must not close it"
    );
    let _ = widget.on_event(&SystemEvent::TextInput {
        text: "B".to_owned(),
    });
    assert!(matches!(
        widget.snapshot_fields(),
        SnapshotFields::Cascader { search_query, .. } if search_query == "B测A试"
    ));
}

#[test]
fn searchable_reconcile_preserves_query_and_replaces_loading_paths() {
    let mut widget = cascader().searchable(true).loading_child("china", true);
    let _ = widget.on_event(&SystemEvent::TextInput {
        text: "china".to_owned(),
    });
    let _ = widget.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(widget.selected().values, ["china"]);
    assert!(widget.is_open());
    assert!(widget
        .semantic_event(ComponentId::new(11), &SystemEvent::FocusIn)
        .is_none());

    widget.sync_from(cascader().searchable(true));
    assert!(matches!(
        widget.snapshot_fields(),
        SnapshotFields::Cascader {
            search_query,
            search_results,
            ..
        } if search_query == "china" && search_results.len() == 2
    ));
    let _ = widget.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(widget.selected().values, ["china", "beijing"]);
    assert!(!widget.is_open());
}

#[test]
fn cascader_identical_view_reconcile_preserves_keyboard_child_level() {
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(cascader()));
    let root = tree.root_id().expect("cascader root");
    tree.set_focus(Some(root));

    for step in 0..3 {
        assert_eq!(
            tree.dispatch_event(&SystemEvent::KeyDown {
                key: KeyCode::Enter,
                mods: KeyMod::NONE,
            }),
            EventResult::Handled
        );
        assert_eq!(
            tree.dispatch_event(&SystemEvent::KeyUp {
                key: KeyCode::Enter,
                mods: KeyMod::NONE,
            }),
            EventResult::Handled
        );
        if step < 2 {
            ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(cascader()));
        }
    }

    let widget = tree
        .get(root)
        .expect("cascader node")
        .component()
        .as_any()
        .downcast_ref::<Cascader>()
        .expect("cascader component");
    assert_eq!(widget.selected().values, ["china", "beijing"]);
    assert!(!widget.is_open());
}

#[test]
fn cascader_left_returns_to_parent_level() {
    let mut cascader = cascader();
    cascader.open();
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["singapore"]);
}

#[test]
fn constrained_trigger_clips_text_and_uses_its_actual_hit_frame() {
    let mut cascader = Cascader::new(
        vec![CascaderOption::new(
            "A very long option label that must stay inside the popup column",
            "long",
        )],
        "A very long placeholder that must not cover the arrow",
    );
    let frame = Rect::new(0.0, 0.0, 60.0, 10.0);
    let (pixels, stride) = render_cascader_pixels(&cascader, frame);
    for y in 0..280usize {
        for x in 0..520usize {
            if x >= 60 || y >= 10 {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "closed constrained Cascader leaked paint at ({x}, {y})"
                );
            }
        }
    }

    assert_eq!(click(&mut cascader, 5.0, 20.0), EventResult::NotHandled);
    assert!(!cascader.is_open());

    assert_eq!(click(&mut cascader, 5.0, 5.0), EventResult::Handled);
    let _ = WidgetAnimation::update_animation(&mut cascader, 1.0);
    let (pixels, stride) = render_cascader_pixels(&cascader, frame);
    let bounds = EventHandler::hit_test_frame(&cascader, frame);
    for y in 0..280usize {
        for x in 0..520usize {
            if x >= bounds.w as usize || y >= bounds.h as usize {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "open Cascader leaked paint at ({x}, {y}); bounds={bounds:?}"
                );
            }
        }
    }
}

#[test]
fn focus_open_state_and_popover_overlay_are_observable() {
    let mut cascader = cascader();
    let id = ComponentId::new(15);
    let frame = Rect::new(20.0, 40.0, 80.0, 12.0);
    assert!(WidgetRender::overlay_entry(&cascader, id, frame).is_none());
    assert_eq!(
        cascader.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(
        cascader.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let overlay = WidgetRender::overlay_entry(&cascader, id, frame)
        .expect("an open Cascader must paint above later siblings");
    assert_eq!(overlay.kind(), crate::ui::OverlayKind::Popover);
    assert_eq!(
        overlay.bounds_rect(),
        Some(EventHandler::hit_test_frame(&cascader, frame))
    );
    assert_eq!(
        cascader.snapshot_fields().accessibility().state.expanded,
        Some(true)
    );
}

#[test]
fn pointer_hit_requires_both_popup_coordinates() {
    let mut cascader = Cascader::new(vec![CascaderOption::new("Alpha", "alpha")], "Region");
    render_cascader(&cascader, Rect::new(0.0, 0.0, 120.0, 32.0));
    assert_eq!(click(&mut cascader, 4.0, 4.0), EventResult::Handled);
    assert_eq!(click(&mut cascader, 250.0, 50.0), EventResult::NotHandled);
    assert!(cascader.selected().values.is_empty());
    assert!(!cascader.is_open());
}

#[test]
fn pointer_keeps_ancestor_columns_visible_and_can_switch_branch() {
    let mut cascader = Cascader::new(
        vec![
            CascaderOption::new("Alpha", "alpha")
                .children(vec![CascaderOption::new("Alpha child", "alpha-child")]),
            CascaderOption::new("Beta", "beta"),
        ],
        "Region",
    );
    let frame = Rect::new(0.0, 0.0, 120.0, 32.0);
    render_cascader(&cascader, frame);
    assert_eq!(click(&mut cascader, 4.0, 4.0), EventResult::Handled);
    assert_eq!(click(&mut cascader, 20.0, 50.0), EventResult::Handled);
    assert!(cascader.is_open());
    assert!(EventHandler::hit_test_frame(&cascader, frame).w >= 400.0);

    assert_eq!(click(&mut cascader, 20.0, 82.0), EventResult::Handled);
    assert_eq!(cascader.selected().values, ["beta"]);
    assert!(!cascader.is_open());
}

#[test]
fn wheel_reaches_options_below_the_initial_popup_viewport() {
    let options = (0..10)
        .map(|index| CascaderOption::new(format!("Option {index}"), format!("option-{index}")))
        .collect();
    let mut cascader = Cascader::new(options, "Region");
    render_cascader(&cascader, Rect::new(0.0, 0.0, 120.0, 32.0));
    assert_eq!(click(&mut cascader, 4.0, 4.0), EventResult::Handled);
    assert_eq!(
        cascader.on_event(&SystemEvent::Wheel {
            pos: Point::new(20.0, 100.0),
            delta: Point::new(0.0, 100.0),
        }),
        EventResult::Handled
    );
    assert_eq!(click(&mut cascader, 20.0, 220.0), EventResult::Handled);
    assert_eq!(cascader.selected().values, ["option-9"]);
}

#[test]
fn pointer_hover_and_keyboard_end_update_the_active_option() {
    let options = (0..10)
        .map(|index| CascaderOption::new(format!("Option {index}"), format!("option-{index}")))
        .collect();
    let mut cascader = Cascader::new(options, "Region");
    render_cascader(&cascader, Rect::new(0.0, 0.0, 120.0, 32.0));
    assert_eq!(click(&mut cascader, 4.0, 4.0), EventResult::Handled);
    assert_eq!(
        cascader.on_event(&SystemEvent::PointerMove {
            pos: Point::new(20.0, 82.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        cascader.on_event(&SystemEvent::KeyDown {
            key: KeyCode::End,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["option-9"]);
}
