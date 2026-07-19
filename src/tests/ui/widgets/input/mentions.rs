use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::Mentions;

fn render_mentions(mentions: &Mentions, frame: Rect, measure_text: &str) -> (Vec<u32>, usize, f32) {
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
        WidgetRender::render(mentions, frame, &mut ctx, &tree);
    }
    (canvas.surface().pixels().to_vec(), WIDTH as usize, measured)
}

fn click(mentions: &mut Mentions, x: f32, y: f32) -> EventResult {
    mentions.on_event(&SystemEvent::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    })
}

fn mentions() -> Mentions {
    Mentions::new("Mention a person").options(vec!["Ada", "Alan", "Grace"])
}

#[test]
fn mentions_keeps_typed_query_in_value_and_filters_suggestions() {
    let mut mentions = mentions();
    assert!(mentions
        .as_text_input()
        .expect("Mentions text input capability")
        .accepts_text_input());

    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "Hello @AL".to_owned(),
    });
    assert_eq!(mentions.value(), "Hello @AL");
    assert!(mentions.is_suggesting());
    assert_eq!(mentions.filtered_options(), &["Alan"]);
    assert!(matches!(
        mentions.snapshot_fields(),
        SnapshotFields::Mentions {
            value,
            suggesting: true,
            ..
        } if value == "Hello @AL"
    ));
    let accessibility = mentions.snapshot_fields().accessibility();
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Hello @AL"));
    assert_eq!(accessibility.state.expanded, Some(true));
    let semantic = mentions
        .semantic_event(ComponentId::new(31), &SystemEvent::FocusIn)
        .expect("typed Mentions text should emit Change");
    assert_eq!(semantic.text_payload(), Some("Hello @AL"));
}

#[test]
fn mentions_backspace_refilters_and_enter_replaces_active_query() {
    let mut mentions = mentions();
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@Adx".to_owned(),
    });
    let _ = mentions.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    assert_eq!(mentions.value(), "@Ad");
    assert_eq!(mentions.filtered_options(), &["Ada"]);

    assert_eq!(
        mentions.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(mentions.value(), "@Ada ");
    assert!(!mentions.is_suggesting());
    let semantic = mentions
        .semantic_event(ComponentId::new(33), &SystemEvent::FocusIn)
        .expect("committed Mentions suggestion should emit Change");
    assert_eq!(semantic.text_payload(), Some("@Ada "));
}

#[test]
fn mentions_popup_click_commits_visible_suggestion() {
    let mut mentions = mentions();
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@a".to_owned(),
    });
    assert_eq!(
        mentions.on_event(&SystemEvent::PointerDown {
            pos: Point::new(8.0, 32.0 + SUGGESTION_ROW_HEIGHT_FOR_TEST * 2.0 + 1.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(mentions.value(), "@Grace ");
}

#[test]
fn mentions_focus_out_preserves_uncommitted_query_text() {
    let mut mentions = mentions();
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@Ada".to_owned(),
    });
    let _ = mentions.on_event(&SystemEvent::FocusOut);
    assert_eq!(mentions.value(), "@Ada");
    assert!(!mentions.is_suggesting());
    assert_eq!(
        mentions.on_event(&SystemEvent::KeyDown {
            key: KeyCode::F1,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}

#[test]
fn mentions_constrained_trigger_clips_text_and_uses_actual_hit_frame() {
    let mut mentions = Mentions::new("A very long placeholder that must stay inside the input")
        .options(vec![
            "A very long suggestion that must stay inside the popup",
        ]);
    let frame = Rect::new(0.0, 0.0, 60.0, 10.0);
    let (pixels, stride, _) = render_mentions(&mentions, frame, "");
    for y in 0..360usize {
        for x in 0..360usize {
            if x >= 60 || y >= 10 {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "closed constrained Mentions leaked paint at ({x}, {y})"
                );
            }
        }
    }

    assert_eq!(click(&mut mentions, 5.0, 20.0), EventResult::NotHandled);
    assert_eq!(click(&mut mentions, 70.0, 5.0), EventResult::NotHandled);
    assert!(!mentions.is_suggesting());

    assert_eq!(click(&mut mentions, 5.0, 5.0), EventResult::Handled);
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@".to_owned(),
    });
    let (pixels, stride, _) = render_mentions(&mentions, frame, "@");
    let bounds = EventHandler::hit_test_frame(&mentions, frame);
    assert_eq!(bounds.w, 200.0);
    assert_eq!(bounds.h, 38.0);
    for y in 0..360usize {
        for x in 0..360usize {
            if x >= bounds.w as usize || y >= bounds.h as usize {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "open constrained Mentions leaked paint at ({x}, {y}); bounds={bounds:?}"
                );
            }
        }
    }
}

#[test]
fn mentions_pointer_hit_requires_popup_x_and_visible_y() {
    let options = (0..30).map(|index| format!("Person {index}")).collect();
    let mut mentions = Mentions::new("Mention").options(options);
    let _ = render_mentions(&mentions, Rect::new(0.0, 0.0, 200.0, 32.0), "");
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@".to_owned(),
    });

    assert_eq!(click(&mut mentions, 220.0, 45.0), EventResult::NotHandled);
    assert_eq!(mentions.value(), "@");
    assert!(!mentions.is_suggesting());

    assert_eq!(click(&mut mentions, 10.0, 16.0), EventResult::Handled);
    assert!(mentions.is_suggesting());
    assert_eq!(click(&mut mentions, 10.0, 313.0), EventResult::NotHandled);
    assert_eq!(mentions.value(), "@");
    assert!(!mentions.is_suggesting());
}

#[test]
fn mentions_empty_filter_has_a_no_data_popover() {
    let mut mentions = Mentions::new("Mention").options(vec!["Alpha"]);
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@zzz".to_owned(),
    });
    assert!(mentions.filtered_options().is_empty());
    let frame = Rect::new(20.0, 40.0, 80.0, 12.0);
    let overlay = WidgetRender::overlay_entry(&mentions, ComponentId::new(35), frame)
        .expect("an unmatched mention query must still expose a no-data popup");
    assert_eq!(overlay.kind(), OverlayKind::Popover);
    assert_eq!(
        overlay.bounds_rect(),
        Some(EventHandler::hit_test_frame(&mentions, frame))
    );
    assert_eq!(overlay.bounds_rect().expect("overlay bounds").w, 200.0);
    assert_eq!(overlay.bounds_rect().expect("overlay bounds").h, 40.0);

    let (pixels, stride, _) = render_mentions(&mentions, Rect::new(0.0, 0.0, 80.0, 12.0), "@zzz");
    assert!(
        (12..40).any(|y| (0..200).any(|x| pixels[y * stride + x] != 0)),
        "unmatched Mentions query must render a localized no-data row"
    );
}

#[test]
fn mentions_long_dropdown_wheels_and_selects_visible_rows() {
    let options = (0..30).map(|index| format!("Person {index}")).collect();
    let mut mentions = Mentions::new("Mention").options(options);
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@".to_owned(),
    });
    assert_eq!(
        mentions.on_event(&SystemEvent::Wheel {
            pos: Point::new(10.0, 50.0),
            delta: Point::new(0.0, 3.0),
        }),
        EventResult::Handled
    );
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&mentions),
        Some((0.0, 120.0))
    );
    assert_eq!(click(&mut mentions, 10.0, 46.0), EventResult::Handled);
    assert_eq!(mentions.value(), "@Person 4 ");
}

#[test]
fn mentions_pointer_hover_only_handles_visible_changes() {
    let mut mentions = Mentions::new("Mention").options(vec!["Alpha", "Beta"]);
    let _ = render_mentions(&mentions, Rect::new(0.0, 0.0, 200.0, 32.0), "");
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@".to_owned(),
    });
    let second_row = SystemEvent::PointerMove {
        pos: Point::new(20.0, 74.0),
        mods: KeyMod::NONE,
    };
    assert_eq!(mentions.on_event(&second_row), EventResult::Handled);
    assert_eq!(mentions.on_event(&second_row), EventResult::NotHandled);

    let outside = SystemEvent::PointerMove {
        pos: Point::new(240.0, 74.0),
        mods: KeyMod::NONE,
    };
    assert_eq!(mentions.on_event(&outside), EventResult::Handled);
    assert_eq!(mentions.on_event(&outside), EventResult::NotHandled);
}

#[test]
fn mentions_caret_uses_measured_text_and_edits_at_the_cursor() {
    let mut mentions = Mentions::new("Mention");
    let _ = mentions.on_event(&SystemEvent::FocusIn);
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "测试".to_owned(),
    });
    let (_, _, measured) = render_mentions(&mentions, Rect::new(0.0, 0.0, 200.0, 32.0), "测试");
    let cursor = mentions
        .as_text_input()
        .expect("Mentions text input capability")
        .text_input_cursor_rect();
    assert!((cursor.x - (10.0 + measured)).abs() < 0.6, "{cursor:?}");

    assert_eq!(
        mentions.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "A".to_owned(),
    });
    assert_eq!(mentions.value(), "测A试");

    let _ = render_mentions(&mentions, Rect::new(0.0, 0.0, 200.0, 32.0), "测A试");
    assert_eq!(click(&mut mentions, 10.0, 16.0), EventResult::Handled);
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "B".to_owned(),
    });
    assert_eq!(mentions.value(), "B测A试");
}

#[test]
fn mentions_commit_replaces_query_at_caret_without_losing_suffix() {
    let mut mentions = mentions();
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "Hi @Al there".to_owned(),
    });
    for _ in 0..6 {
        let _ = mentions.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Left,
            mods: KeyMod::NONE,
        });
    }
    assert!(mentions.is_suggesting());
    assert_eq!(mentions.filtered_options(), &["Alan"]);
    assert_eq!(
        mentions.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(mentions.value(), "Hi @Alan there");
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "X".to_owned(),
    });
    assert_eq!(mentions.value(), "Hi @Alan Xthere");
}

#[test]
fn mentions_identical_reconcile_preserves_keyboard_candidate() {
    let options = vec!["Alpha", "Beta", "Gamma"];
    let mut mentions = Mentions::new("Mention").options(options.clone());
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@".to_owned(),
    });
    for _ in 0..2 {
        let _ = mentions.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        });
    }

    mentions.sync_from(Mentions::new("Mention").options(options));
    let _ = mentions.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(mentions.value(), "@Gamma ");
}

const SUGGESTION_ROW_HEIGHT_FOR_TEST: f32 = 28.0;
