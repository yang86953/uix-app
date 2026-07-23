use crate::draw::backend::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::AutoComplete;

fn render_autocomplete(
    autocomplete: &AutoComplete,
    frame: Rect,
    measure_text: &str,
) -> (Vec<u32>, usize, f32) {
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
            WIDTH,
            HEIGHT,
        );
        measured = ctx.measure_text(measure_text, 13.0).w;
        WidgetRender::render(autocomplete, frame, &mut ctx, &tree);
    }
    (canvas.surface().pixels().to_vec(), WIDTH as usize, measured)
}

fn click(autocomplete: &mut AutoComplete, x: f32, y: f32) -> EventResult {
    autocomplete.on_event(&SystemEvent::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    })
}

fn autocomplete() -> AutoComplete {
    AutoComplete::new()
        .placeholder("Name")
        .options(vec!["Ada", "Alan", "Grace"])
}

#[test]
fn autocomplete_accepts_platform_text_and_filters_case_insensitively() {
    let mut autocomplete = autocomplete();
    assert!(autocomplete
        .as_text_input()
        .expect("AutoComplete text input capability")
        .accepts_text_input());

    assert_eq!(
        autocomplete.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(
        autocomplete.on_event(&SystemEvent::TextInput {
            text: "AL".to_owned(),
        }),
        EventResult::Handled
    );
    assert_eq!(autocomplete.value(), "AL");
    assert_eq!(autocomplete.filtered_options(), &["Alan"]);
    assert!(autocomplete.is_open());
    let semantic = autocomplete
        .semantic_event(ComponentId::new(19), &SystemEvent::FocusIn)
        .expect("typed AutoComplete text should emit Change");
    assert_eq!(semantic.text_payload(), Some("AL"));
    assert!(matches!(
        autocomplete.snapshot_fields(),
        SnapshotFields::AutoComplete {
            value,
            open: true,
            ..
        } if value == "AL"
    ));
    let accessibility = autocomplete.snapshot_fields().accessibility();
    assert_eq!(accessibility.state.value_text.as_deref(), Some("AL"));
    assert_eq!(accessibility.state.expanded, Some(true));
}

#[test]
fn autocomplete_backspace_refilters_and_enter_commits_match() {
    let mut autocomplete = autocomplete();
    let _ = autocomplete.on_event(&SystemEvent::TextInput {
        text: "ad".to_owned(),
    });
    let _ = autocomplete.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });

    assert_eq!(autocomplete.value(), "a");
    assert_eq!(autocomplete.filtered_options(), &["Ada", "Alan", "Grace"]);
    assert_eq!(
        autocomplete.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(autocomplete.value(), "Ada");
    assert!(!autocomplete.is_open());
    assert!(autocomplete.is_present());
    let semantic = autocomplete
        .semantic_event(ComponentId::new(21), &SystemEvent::FocusIn)
        .expect("committed AutoComplete suggestion should emit Change");
    assert_eq!(semantic.text_payload(), Some("Ada"));
}

#[test]
fn autocomplete_focus_out_closes_without_consuming_unrelated_keys() {
    let mut autocomplete = autocomplete();
    autocomplete.open();
    assert_eq!(
        autocomplete.on_event(&SystemEvent::KeyDown {
            key: KeyCode::F1,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        autocomplete.on_event(&SystemEvent::FocusOut),
        EventResult::Handled
    );
    assert!(!autocomplete.is_open());
}

#[test]
fn autocomplete_constrained_trigger_clips_text_and_uses_actual_hit_frame() {
    let mut autocomplete = AutoComplete::new()
        .placeholder("A very long placeholder that must stay inside the input")
        .options(vec![
            "A very long suggestion that must stay inside the popup",
        ]);
    let frame = Rect::new(0.0, 0.0, 60.0, 10.0);
    let (pixels, stride, _) = render_autocomplete(&autocomplete, frame, "");
    for y in 0..360usize {
        for x in 0..360usize {
            if x >= 60 || y >= 10 {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "closed constrained AutoComplete leaked paint at ({x}, {y})"
                );
            }
        }
    }

    assert_eq!(click(&mut autocomplete, 5.0, 20.0), EventResult::NotHandled);
    assert_eq!(click(&mut autocomplete, 70.0, 5.0), EventResult::NotHandled);
    assert!(!autocomplete.is_open());

    assert_eq!(click(&mut autocomplete, 5.0, 5.0), EventResult::Handled);
    let _ = WidgetAnimation::update_animation(&mut autocomplete, 1.0);
    let (pixels, stride, _) = render_autocomplete(&autocomplete, frame, "");
    let bounds = EventHandler::hit_test_frame(&autocomplete, frame);
    assert_eq!(bounds.w, 200.0);
    assert_eq!(bounds.h, 38.0);
    for y in 0..360usize {
        for x in 0..360usize {
            if x >= bounds.w as usize || y >= bounds.h as usize {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "open constrained AutoComplete leaked paint at ({x}, {y}); bounds={bounds:?}"
                );
            }
        }
    }
}

#[test]
fn autocomplete_pointer_hit_requires_popup_x_and_visible_y() {
    let options = (0..30).map(|index| format!("Option {index}")).collect();
    let mut autocomplete = AutoComplete::new().options(options);
    let _ = render_autocomplete(&autocomplete, Rect::new(0.0, 0.0, 200.0, 32.0), "");

    autocomplete.open();
    assert_eq!(
        click(&mut autocomplete, 220.0, 45.0),
        EventResult::NotHandled
    );
    assert!(autocomplete.value().is_empty());
    assert!(!autocomplete.is_open());

    autocomplete.open();
    assert_eq!(
        click(&mut autocomplete, 10.0, 313.0),
        EventResult::NotHandled
    );
    assert!(autocomplete.value().is_empty());
    assert!(!autocomplete.is_open());
}

#[test]
fn autocomplete_empty_filter_has_a_no_data_popover() {
    let mut autocomplete = AutoComplete::new()
        .placeholder("Search")
        .options(vec!["Alpha"]);
    let _ = autocomplete.on_event(&SystemEvent::TextInput {
        text: "zzz".to_owned(),
    });
    assert!(autocomplete.filtered_options().is_empty());
    let frame = Rect::new(20.0, 40.0, 80.0, 12.0);
    let id = ComponentId::new(23);
    let overlay = WidgetRender::overlay_entry(&autocomplete, id, frame)
        .expect("an unmatched query must still expose a no-data popup");
    assert_eq!(overlay.kind(), OverlayKind::Popover);
    assert_eq!(
        overlay.bounds_rect(),
        Some(EventHandler::hit_test_frame(&autocomplete, frame))
    );
    assert_eq!(overlay.bounds_rect().expect("overlay bounds").w, 200.0);
    assert_eq!(overlay.bounds_rect().expect("overlay bounds").h, 40.0);

    let _ = WidgetAnimation::update_animation(&mut autocomplete, 1.0);
    let (pixels, stride, _) =
        render_autocomplete(&autocomplete, Rect::new(0.0, 0.0, 80.0, 12.0), "zzz");
    assert!(
        (12..40).any(|y| (0..200).any(|x| pixels[y * stride + x] != 0)),
        "unmatched AutoComplete query must render a localized no-data row"
    );
}

#[test]
fn autocomplete_long_dropdown_wheels_and_selects_visible_rows() {
    let options = (0..30).map(|index| format!("Option {index}")).collect();
    let mut autocomplete = AutoComplete::new().options(options);
    autocomplete.open();
    assert_eq!(
        autocomplete.on_event(&SystemEvent::Wheel {
            pos: Point::new(10.0, 50.0),
            delta: Point::new(0.0, 3.0),
        }),
        EventResult::Handled
    );
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&autocomplete),
        Some((0.0, 120.0))
    );
    assert_eq!(click(&mut autocomplete, 10.0, 46.0), EventResult::Handled);
    assert_eq!(autocomplete.value(), "Option 4");
}

#[test]
fn autocomplete_pointer_hover_only_handles_visible_changes() {
    let mut autocomplete = AutoComplete::new().options(vec!["Alpha", "Beta"]);
    let _ = render_autocomplete(&autocomplete, Rect::new(0.0, 0.0, 200.0, 32.0), "");
    autocomplete.open();
    let second_row = SystemEvent::PointerMove {
        pos: Point::new(20.0, 74.0),
        mods: KeyMod::NONE,
    };
    assert_eq!(autocomplete.on_event(&second_row), EventResult::Handled);
    assert_eq!(autocomplete.on_event(&second_row), EventResult::NotHandled);

    let outside = SystemEvent::PointerMove {
        pos: Point::new(240.0, 74.0),
        mods: KeyMod::NONE,
    };
    assert_eq!(autocomplete.on_event(&outside), EventResult::Handled);
    assert_eq!(autocomplete.on_event(&outside), EventResult::NotHandled);
}

#[test]
fn autocomplete_caret_uses_measured_text_and_edits_at_the_cursor() {
    let mut autocomplete = AutoComplete::new();
    let _ = autocomplete.on_event(&SystemEvent::FocusIn);
    let _ = autocomplete.on_event(&SystemEvent::TextInput {
        text: "测试".to_owned(),
    });
    let (_, _, measured) =
        render_autocomplete(&autocomplete, Rect::new(0.0, 0.0, 200.0, 32.0), "测试");
    let cursor = autocomplete
        .as_text_input()
        .expect("AutoComplete text input capability")
        .text_input_cursor_rect();
    assert!((cursor.x - (10.0 + measured)).abs() < 0.6, "{cursor:?}");

    assert_eq!(
        autocomplete.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let _ = autocomplete.on_event(&SystemEvent::TextInput {
        text: "A".to_owned(),
    });
    assert_eq!(autocomplete.value(), "测A试");

    let _ = render_autocomplete(&autocomplete, Rect::new(0.0, 0.0, 200.0, 32.0), "测A试");
    assert_eq!(click(&mut autocomplete, 10.0, 16.0), EventResult::Handled);
    let _ = autocomplete.on_event(&SystemEvent::TextInput {
        text: "B".to_owned(),
    });
    assert_eq!(autocomplete.value(), "B测A试");
}

#[test]
fn autocomplete_identical_reconcile_preserves_keyboard_candidate() {
    let options = vec!["Alpha", "Beta", "Gamma"];
    let mut autocomplete = AutoComplete::new().options(options.clone());
    autocomplete.open();
    for _ in 0..2 {
        let _ = autocomplete.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        });
    }

    autocomplete.sync_from(AutoComplete::new().options(options));
    let _ = autocomplete.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(autocomplete.value(), "Gamma");
}
