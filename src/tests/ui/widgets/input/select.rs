use crate::draw::compositor::ScenePaint;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::input::select::*;

fn render_select(
    select: &Select,
    frame: Rect,
    surface_size: (i32, i32),
    measure_text: &str,
) -> (Vec<u32>, f32) {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();

    let measured;
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
        measured = ctx.measure_text(measure_text, 13.0).w;
        WidgetRender::render(select, frame, &mut ctx, &tree);
    }
    (canvas.surface().pixels().to_vec(), measured)
}

fn pixel_region(pixels: &[u32], surface_width: usize, rect: Rect) -> Vec<u32> {
    let x0 = rect.x.max(0.0) as usize;
    let y0 = rect.y.max(0.0) as usize;
    let x1 = (rect.x + rect.w).max(0.0) as usize;
    let y1 = (rect.y + rect.h).max(0.0) as usize;
    let mut region = Vec::new();
    for y in y0..y1 {
        let row = y * surface_width;
        region.extend_from_slice(&pixels[row + x0..row + x1]);
    }
    region
}

fn large_select() -> Select {
    let opts: Vec<String> = (0..100).map(|i| format!("Option {i}")).collect();
    Select::new().options(opts)
}

#[test]
fn open_select_is_promoted_to_the_overlay_layer() {
    let select = Select::new().options(["Alpha", "Beta", "Gamma"]);
    let _ = render_select(&select, Rect::new(20.0, 40.0, 160.0, 32.0), (240, 200), "");
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(select));
    tree.get_mut(id)
        .expect("select root")
        .set_frame(Rect::new(20.0, 40.0, 160.0, 32.0));

    assert!(!ScenePaint::node_is_overlay(&tree, id));
    tree.get_mut(id)
        .expect("select root")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Select>()
        .expect("select component")
        .open();

    assert!(
        ScenePaint::node_is_overlay(&tree, id),
        "an open Select must paint after ordinary sibling content"
    );
    let overlay = tree
        .get(id)
        .expect("select root")
        .overlay_entry(id, Rect::new(20.0, 40.0, 160.0, 32.0))
        .expect("select popup overlay");
    assert_eq!(overlay.kind(), crate::ui::OverlayKind::Popover);
    assert_eq!(overlay.z_index_value(), 900);
    assert_eq!(
        overlay.bounds_rect(),
        Some(Rect::new(20.0, 72.0, 160.0, 84.0))
    );
}

#[test]
fn select_dropdown_scroll_range_limits_visible_rows() {
    let select = large_select();
    let row_count = select.dropdown_row_count();
    let viewport_h = select.dropdown_viewport_height(row_count);
    let (start, end) = select
        .dropdown_scroll
        .scroll_range(row_count, 28.0, viewport_h);
    assert_eq!(start, 0);
    assert!(
        end - start < 100,
        "virtual scroll should expose a small window"
    );
}

#[test]
fn select_dropdown_wheel_records_composite_delta() {
    let mut select = large_select();
    select.open();

    assert_eq!(
        EventHandler::on_event(
            &mut select,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 50.0),
                delta: Point::new(0.0, 1.0),
            },
        ),
        EventResult::Handled
    );
    assert!(select.dropdown_scroll.scroll_offset() > 0.0);
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&select),
        Some((0.0, 40.0))
    );
}

#[test]
fn select_dropdown_row_at_y_accounts_for_scroll_offset() {
    let mut select = large_select();
    select.open();
    select.dropdown_scroll.set_scroll_offset(28.0 * 5.0);

    assert_eq!(select.dropdown_row_at_y(33.0), Some(5));
    assert_eq!(select.dropdown_row_at_y(61.0), Some(6));
}

#[test]
fn single_value_binding_reads_and_writes_option_text() {
    let options = ["Alpha", "Beta", "Gamma"];
    let selected = State::new("Beta".to_owned());
    let mut select = Select::new().options(&options).value(&selected);
    assert_eq!(select.current_value().as_deref(), Some("Beta"));

    select.open();
    assert_eq!(
        select.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 40.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(selected.get(), "Alpha");
    assert_eq!(select.current_value().as_deref(), Some("Alpha"));

    selected.set("Gamma".to_owned());
    select.sync_from(Select::new().value(&selected).options(&options));
    assert_eq!(select.current_value().as_deref(), Some("Gamma"));

    selected.set(String::new());
    select.sync_from(Select::new().options(&options).value(&selected));
    assert_eq!(select.current_value(), None);
    select.open();
    let _ = select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), "Alpha");
}

#[test]
fn multiple_value_binding_toggles_a_hash_set() {
    let options = ["Alpha", "Beta", "Gamma"];
    let selected = State::new(HashSet::from(["Beta".to_owned()]));
    let mut select = Select::multiple().options(&options).value(&selected);
    assert_eq!(select.current_values(), HashSet::from(["Beta".to_owned()]));

    select.open();
    let _ = select.on_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 40.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(
        selected.get(),
        HashSet::from(["Alpha".to_owned(), "Beta".to_owned()])
    );

    let _ = select.on_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 68.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), HashSet::from(["Alpha".to_owned()]));
}

#[test]
fn searchable_factory_and_external_state_reconcile_are_structural() {
    let options = ["Alpha", "Beta"];
    let selected = State::new("Alpha".to_owned());
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Select::searchable().options(&options).value(&selected))
    }));
    let root = tree.root_id().expect("select root");
    tree.reset_invalidation();

    selected.set("Beta".to_owned());
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| {
        ViewNode::leaf(Select::searchable().options(&options).value(&selected))
    });
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let select = tree
        .get(root)
        .expect("select node")
        .component()
        .as_any()
        .downcast_ref::<Select>()
        .expect("Select component");
    assert_eq!(select.current_value().as_deref(), Some("Beta"));
    assert!(matches!(
        select.snapshot_fields(),
        SnapshotFields::Select { search: true, .. }
    ));
}

#[test]
fn searchable_filters_case_insensitively_and_selects_original_option() {
    let options = ["Alpha", "Beta", "Alpine"];
    let selected = State::new(String::new());
    let mut select = Select::searchable().options(&options).value(&selected);

    assert!(select
        .as_text_input()
        .expect("searchable Select text capability")
        .accepts_text_input());
    select.open();
    assert_eq!(
        select.on_event(&SystemEvent::TextInput {
            text: "ALP".to_owned(),
        }),
        EventResult::Handled
    );
    assert_eq!(select.visible_option_indices(), vec![0, 2]);
    assert_eq!(select.dropdown_row_count(), 2);
    assert!(matches!(
        select.snapshot_fields(),
        SnapshotFields::Select {
            search_query,
            ..
        } if search_query == "ALP"
    ));
    assert_eq!(
        select
            .snapshot_fields()
            .accessibility()
            .state
            .value_text
            .as_deref(),
        Some("ALP")
    );

    assert_eq!(
        select.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 70.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(selected.get(), "Alpine");
    assert_eq!(
        select
            .snapshot_fields()
            .accessibility()
            .state
            .value_text
            .as_deref(),
        Some("Alpine")
    );
}

#[test]
fn searchable_backspace_updates_results_and_keeps_an_empty_row() {
    let mut select = Select::searchable().options(["Alpha", "Beta"]);
    select.open();
    let _ = select.on_event(&SystemEvent::TextInput {
        text: "zz".to_owned(),
    });
    assert!(select.visible_option_indices().is_empty());
    assert_eq!(select.dropdown_row_count(), 1);

    let _ = select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    assert!(matches!(
        select.snapshot_fields(),
        SnapshotFields::Select {
            search_query,
            ..
        } if search_query == "z"
    ));
    assert_eq!(select.dropdown_row_count(), 1);
}

#[test]
fn searchable_reconcile_preserves_query_and_enter_selects_first_match() {
    let selected = State::new(String::new());
    let mut select = Select::searchable()
        .options(["Alpha", "Beta", "Gamma"])
        .value(&selected);
    select.open();
    let _ = select.on_event(&SystemEvent::TextInput {
        text: "et".to_owned(),
    });

    select.sync_from(
        Select::searchable()
            .options(["Alpha", "Beta", "Delta"])
            .value(&selected),
    );
    assert_eq!(select.visible_option_indices(), vec![1]);
    let _ = select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });

    assert_eq!(selected.get(), "Beta");
    assert!(!select.is_open());
    assert!(select.is_present());
    assert!(!WidgetAnimation::update_animation(&mut select, 1.0));
    assert!(!select.is_present());
    assert!(matches!(
        select.snapshot_fields(),
        SnapshotFields::Select {
            search_query,
            ..
        } if search_query.is_empty()
    ));
}

#[test]
fn grouped_search_maps_duplicate_labels_to_their_original_indices() {
    let selected = State::new(String::new());
    let mut select = Select::searchable()
        .optgroups(vec![
            OptGroup::new("First").add("Same"),
            OptGroup::new("Second").add("Same"),
        ])
        .value(&selected);
    select.open();
    let _ = select.on_event(&SystemEvent::TextInput {
        text: "same".to_owned(),
    });

    assert_eq!(select.visible_option_indices(), vec![0, 1]);
    assert_eq!(select.dropdown_row_count(), 4);
    let _ = select.on_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 32.0 + 3.0 * 28.0 + 1.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(select.current_value().as_deref(), Some("Same"));
    assert_eq!(selected.get(), "Same");
    assert!(matches!(
        select.snapshot_fields(),
        SnapshotFields::Select { selected: 1, .. }
    ));
}

#[test]
fn option_groups_constructor_preserves_group_and_option_order() {
    let selected = State::new(String::new());
    let mut select = Select::new()
        .option_groups([
            SelectOptionGroup::new("Fruit", ["Apple", "Pear"]),
            SelectOptionGroup::new("Vegetable", ["Carrot"]),
        ])
        .value(&selected);

    assert_eq!(select.visible_option_indices(), vec![0, 1, 2]);
    assert_eq!(select.dropdown_row_count(), 5);
    assert!(matches!(
        select.snapshot_fields(),
        SnapshotFields::Select { optgroups, .. }
            if optgroups.iter().map(|group| group.label.as_str()).collect::<Vec<_>>()
                == ["Fruit", "Vegetable"]
    ));

    select.open();
    assert_eq!(
        select.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 32.0 + 4.0 * 28.0 + 1.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(selected.get(), "Carrot");
}

#[test]
fn loading_select_replaces_options_and_keeps_selection_inert() {
    let selected = State::new("Beta".to_owned());
    let mut select = Select::new()
        .options(["Alpha", "Beta"])
        .value(&selected)
        .loading(true);
    select.open();

    assert!(select.visible_option_indices().is_empty());
    assert_eq!(select.dropdown_row_count(), 1);
    assert!(matches!(
        select.snapshot_fields(),
        SnapshotFields::Select { loading: true, .. }
    ));
    assert_eq!(
        select.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 40.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(selected.get(), "Beta");

    assert!(WidgetAnimation::update_animation(&mut select, 1.0));
    let (spinner_before, _) =
        render_select(&select, Rect::new(0.0, 0.0, 120.0, 32.0), (180, 100), "");
    assert!(WidgetAnimation::update_animation(&mut select, 0.1));
    assert_ne!(
        WidgetAnimation::dirty_bounds(&select, Rect::new(0.0, 0.0, 120.0, 32.0)),
        Rect::zero(),
        "loading spinner must request a repaint after its phase advances"
    );
    let (spinner_after, _) =
        render_select(&select, Rect::new(0.0, 0.0, 120.0, 32.0), (180, 100), "");
    assert_ne!(
        pixel_region(&spinner_before, 180, Rect::new(50.0, 36.0, 20.0, 20.0),),
        pixel_region(&spinner_after, 180, Rect::new(50.0, 36.0, 20.0, 20.0),),
        "the loading row must paint the advancing spinner phase"
    );

    select.sync_from(
        Select::new()
            .options(["Alpha", "Beta"])
            .value(&selected)
            .loading(false),
    );
    assert_eq!(select.visible_option_indices(), vec![0, 1]);
    assert_eq!(select.current_value().as_deref(), Some("Beta"));
    assert!(!WidgetAnimation::update_animation(&mut select, 0.1));

    let mut closed_loading = Select::new().loading(true);
    assert!(!WidgetAnimation::update_animation(&mut closed_loading, 0.1));
}

#[test]
fn plain_select_does_not_request_platform_text_input() {
    assert!(!Select::new()
        .as_text_input()
        .expect("Select text capability")
        .accepts_text_input());
    assert!(!Select::searchable()
        .disabled(true)
        .as_text_input()
        .expect("searchable Select text capability")
        .accepts_text_input());
}

#[test]
fn searchable_arrow_navigation_does_not_commit_until_enter() {
    let selected = State::new(String::new());
    let mut select = Select::searchable()
        .options(["Alpha", "Alpine", "Beta"])
        .value(&selected);
    select.open();
    let _ = select.on_event(&SystemEvent::TextInput {
        text: "al".to_owned(),
    });

    let _ = select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), "", "navigation must not publish a value");

    let _ = select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), "Alpine");
}

#[test]
fn empty_select_opening_reserves_a_localized_no_data_row() {
    let mut select = Select::new().placeholder("请选择");
    select.open();

    assert_eq!(select.dropdown_row_count(), 1);
    assert!(
        EventHandler::hit_test_frame(&select, Rect::new(0.0, 0.0, 120.0, 32.0))
            .contains(Point::new(10.0, 46.0))
    );
}

#[test]
fn dropdown_row_hit_rejects_points_below_the_visible_viewport() {
    let mut select = large_select();
    select.open();

    assert_eq!(select.dropdown_row_at_y(32.0 + 280.0 + 1.0), None);
}

#[test]
fn constrained_large_select_uses_the_rendered_control_height_for_pointer_hits() {
    let mut select = Select::new()
        .options(["Alpha", "Beta"])
        .default_selected(1)
        .size(ControlSize::Large);
    select.open();
    let _ = render_select(&select, Rect::new(0.0, 0.0, 120.0, 20.0), (180, 120), "");

    assert_eq!(
        select.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 25.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(select.current_value().as_deref(), Some("Alpha"));
}

#[test]
fn popup_flips_above_near_the_surface_bottom_and_remains_interactive() {
    let mut select = Select::new()
        .options(["Alpha", "Beta", "Gamma"])
        .default_selected(2);
    select.open();
    let frame = Rect::new(20.0, 280.0, 120.0, 32.0);
    let _ = render_select(&select, frame, (180, 320), "");

    let hit = EventHandler::hit_test_frame(&select, frame);
    assert!(
        hit.y < frame.y,
        "upward popup must expand hit testing: {hit:?}"
    );
    assert_eq!(
        select.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, -70.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(select.current_value().as_deref(), Some("Alpha"));
}

#[test]
fn search_cursor_uses_measured_text_and_the_rendered_control_height() {
    let mut select = Select::searchable()
        .options(["测试", "其他"])
        .size(ControlSize::Large);
    let _ = select.on_event(&SystemEvent::FocusIn);
    select.open();
    let _ = select.on_event(&SystemEvent::TextInput {
        text: "测".to_owned(),
    });
    let (_, measured) = render_select(&select, Rect::new(0.0, 0.0, 200.0, 40.0), (240, 160), "测");
    let cursor = select
        .as_text_input()
        .expect("searchable Select text capability")
        .text_input_cursor_rect();

    assert!((cursor.x - (10.0 + measured)).abs() < 0.6, "{cursor:?}");
    assert_eq!(cursor.y, 4.0);
    assert_eq!(cursor.h, 32.0);
}

#[test]
fn intrinsic_width_uses_unicode_text_metrics_instead_of_utf8_bytes() {
    let label = "超长中文选项用于宽度测量";
    let select = Select::new().options([label]);
    let measured = WidgetLayout::measure(&select, Constraints::unconstrained());
    let expected =
        crate::draw::font::text_backend::estimate_text_metrics(label, f32::INFINITY, 13.0)
            .max_line_width
            + 40.0;

    assert!(
        (measured.w - expected.max(120.0)).abs() < 0.01,
        "{measured:?}"
    );
}

#[test]
fn trigger_text_is_clipped_before_the_arrow_slot() {
    let long = Select::new().options(["MMMMMMMMMMMMMMMMMMMMMMMM"]);
    let empty = Select::new();
    let frame = Rect::new(0.0, 0.0, 120.0, 32.0);
    let (long_pixels, _) = render_select(&long, frame, (160, 64), "");
    let (empty_pixels, _) = render_select(&empty, frame, (160, 64), "");
    let arrow_slot = Rect::new(94.0, 2.0, 24.0, 28.0);

    assert_eq!(
        pixel_region(&long_pixels, 160, arrow_slot),
        pixel_region(&empty_pixels, 160, arrow_slot),
        "selected text must not paint underneath the arrow"
    );
}

#[test]
fn multiple_tags_never_paint_outside_the_control_frame() {
    let labels = [
        "Very long selected option Alpha",
        "Very long selected option Beta",
    ];
    let selected = State::new(HashSet::from(labels.map(str::to_owned)));
    let select = Select::multiple().options(labels).value(&selected);
    let (pixels, _) = render_select(&select, Rect::new(0.0, 0.0, 120.0, 32.0), (320, 64), "");

    assert!(
        pixel_region(&pixels, 320, Rect::new(121.0, 0.0, 190.0, 40.0))
            .iter()
            .all(|pixel| *pixel == 0),
        "tag text must be clipped to the Select control"
    );
}

#[test]
fn reopening_during_close_starts_with_a_fresh_search_query() {
    let mut select = Select::searchable().options(["Alpha", "Beta"]);
    select.open();
    let _ = select.on_event(&SystemEvent::TextInput {
        text: "alp".to_owned(),
    });
    select.close();
    select.open();

    assert_eq!(select.visible_option_indices(), vec![0, 1]);
    assert!(matches!(
        select.snapshot_fields(),
        SnapshotFields::Select { search_query, .. } if search_query.is_empty()
    ));
}

#[test]
fn multiple_tag_close_slot_removes_the_value_without_opening() {
    let label = "Very long selected option Alpha";
    let selected = State::new(HashSet::from([label.to_owned()]));
    let mut select = Select::multiple().options([label]).value(&selected);
    let _ = render_select(&select, Rect::new(0.0, 0.0, 120.0, 32.0), (180, 80), "");
    let close = select
        .first_multi_remove_rect()
        .expect("rendered selected tag close slot");

    assert_eq!(
        select.on_event(&SystemEvent::PointerDown {
            pos: Point::new(close.x + close.w * 0.5, close.y + close.h * 0.5),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(selected.get().is_empty());
    assert!(!select.is_open());
}

#[test]
fn multiple_keyboard_navigation_toggles_the_highlight_without_closing() {
    let selected = State::new(HashSet::<String>::new());
    let mut select = Select::multiple()
        .options(["Alpha", "Beta"])
        .value(&selected);
    select.open();
    let _ = select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    let _ = select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Space,
        mods: KeyMod::NONE,
    });

    assert_eq!(selected.get(), HashSet::from(["Beta".to_owned()]));
    assert!(select.is_open());
}

#[test]
fn rendered_pointer_hits_reject_the_shadow_margin_outside_the_control_width() {
    let mut select = Select::new().options(["Alpha", "Beta"]);
    select.open();
    let _ = render_select(&select, Rect::new(0.0, 0.0, 120.0, 32.0), (180, 120), "");

    assert_eq!(
        select.on_event(&SystemEvent::PointerDown {
            pos: Point::new(-4.0, 16.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert!(!select.is_open());
}
