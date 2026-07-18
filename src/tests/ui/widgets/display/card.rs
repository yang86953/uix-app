use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::painting::PaintPass;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::display::card::*;
use crate::ui::AccessibilityRole;

fn render_card_in(card: &Card, frame: Rect, surface_size: (i32, i32)) -> String {
    render_card_in_pass(card, frame, surface_size, PaintPass::Content)
}

fn render_card_in_pass(
    card: &Card,
    frame: Rect,
    surface_size: (i32, i32),
    paint_pass: PaintPass,
) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
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
        ctx.set_paint_pass(paint_pass);
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(card, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn recorded_text_font_sizes(display_list: &str) -> Vec<f32> {
    let marker = "font_size: ";
    display_list
        .match_indices(marker)
        .map(|(start, _)| {
            let start = start + marker.len();
            let end = display_list[start..]
                .find([' ', '}', ']'])
                .map_or(display_list.len(), |offset| start + offset);
            display_list[start..end]
                .trim_end_matches(',')
                .parse()
                .expect("recorded Card font size")
        })
        .collect()
}

struct FixedChild(Size);

impl WidgetComponent for FixedChild {
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
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT)
    }

    crate::wc_upcast!(FixedChild; WidgetLayout);
}

impl WidgetLayout for FixedChild {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.0)
    }
}

struct CountingChild {
    size: Size,
    measure_calls: Rc<Cell<usize>>,
}

impl WidgetComponent for CountingChild {
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
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT)
    }

    crate::wc_upcast!(CountingChild; WidgetLayout);
}

impl WidgetLayout for CountingChild {
    fn measure(&self, constraints: Constraints) -> Size {
        self.measure_calls.set(self.measure_calls.get() + 1);
        constraints.clamp(self.size)
    }
}

#[test]
fn layout_children_measure_children_with_inner_constraints() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Card::new()
            .size(80.0, 80.0)
            .padding(16.0)
            .child(FixedChild(Size::new(200.0, 120.0))),
    ));

    tree.layout();

    let child = tree.get(root).unwrap().children()[0];
    let frame = tree.get(child).unwrap().frame();
    assert_eq!(frame.w, 48.0);
    assert_eq!(frame.h, 48.0);
}

#[test]
fn layout_children_reserve_actions_and_zero_exhausted_body() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Card::new()
            .size(80.0, 80.0)
            .padding(16.0)
            .actions(vec!["Save"])
            .child(FixedChild(Size::new(200.0, 120.0))),
    ));

    tree.layout();

    let child = tree.get(root).unwrap().children()[0];
    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(16.0, 16.0, 48.0, 8.0)
    );

    let narrow_frame = Rect::new(0.0, 0.0, 80.0, 30.0);
    let narrow_root = tree.set_root(Box::new(
        Card::new()
            .title("Title")
            .actions(vec!["Save"])
            .padding(16.0),
    ));
    let narrow_child = tree.add_child(narrow_root, Box::new(FixedChild(Size::new(40.0, 20.0))));
    tree.get_mut(narrow_child)
        .unwrap()
        .set_frame(Rect::new(16.0, 56.0, 48.0, 20.0));

    let widget = tree.get(narrow_root).unwrap().as_layout().unwrap();
    let measured = widget.measure_children(narrow_frame, &[narrow_child], &tree);
    let placements = widget.layout_children(narrow_frame, &measured, &tree);
    assert_eq!(placements[0].1, Rect::new(16.0, 0.0, 0.0, 0.0));
}

#[test]
fn action_rect_stays_inside_short_card() {
    let card = Card::new().actions(vec!["Save", "Cancel"]);

    assert_eq!(
        card.action_rect(Rect::new(4.0, 6.0, 80.0, 24.0)),
        Some(Rect::new(4.0, 6.0, 80.0, 24.0))
    );
}

#[test]
fn card_children_are_clipped_to_the_body_rect() {
    let card = Card::new()
        .title("Profile")
        .actions(vec!["Save"])
        .padding(16.0);

    assert_eq!(
        WidgetRender::children_clip(&card, Rect::new(4.0, 6.0, 120.0, 120.0)),
        Some(Rect::new(20.0, 62.0, 88.0, 8.0))
    );

    let exhausted = Card::new()
        .title("Profile")
        .actions(vec!["Save"])
        .padding(80.0);
    assert_eq!(
        WidgetRender::children_clip(&exhausted, Rect::new(0.0, 0.0, 60.0, 40.0)),
        Some(Rect::new(60.0, 0.0, 0.0, 0.0))
    );
}

#[test]
fn card_render_elides_long_title_and_action_labels_at_readable_sizes() {
    let card = Card::new()
        .title("A very long card title that must remain inside")
        .actions(vec!["Open detailed settings", "Cancel operation"])
        .elevation(0);
    let display_list = render_card_in(&card, Rect::new(0.0, 0.0, 120.0, 96.0), (120, 96));
    let font_sizes = recorded_text_font_sizes(&display_list);

    assert_eq!(font_sizes.len(), 3, "title and two actions should render");
    assert_eq!(font_sizes, vec![15.0, 13.0, 13.0]);
    assert_eq!(
        display_list.matches('…').count(),
        3,
        "long labels should elide instead of becoming unreadably small: {display_list}"
    );
    assert!(
        display_list.matches("PushClip").count() >= 4,
        "card, title, and action slots should be clipped: {display_list}"
    );
}

#[test]
fn card_render_omits_exhausted_title_and_never_records_negative_geometry() {
    let card = Card::new()
        .title("Hidden title")
        .actions(vec!["Action"])
        .padding(80.0)
        .elevation(2);
    let display_list = render_card_in(&card, Rect::new(0.0, 0.0, 30.0, 24.0), (30, 24));

    assert!(
        !display_list.contains("Hidden title"),
        "actions consume the whole short card, so the title must not paint: {display_list}"
    );
    assert!(
        display_list.contains('…'),
        "the short card should keep a readable elided action: {display_list}"
    );
    assert!(!display_list.contains("w: -"), "{display_list}");
    assert!(!display_list.contains("h: -"), "{display_list}");
}

#[test]
fn card_after_children_pass_does_not_cover_body_content() {
    let card = Card::new()
        .title("Profile")
        .actions(vec!["Save"])
        .child(FixedChild(Size::new(20.0, 20.0)));
    let display_list = render_card_in_pass(
        &card,
        Rect::new(0.0, 0.0, 120.0, 96.0),
        (120, 96),
        PaintPass::AfterChildren,
    );

    assert_eq!(display_list, "DisplayList { ops: [] }");
}

#[test]
fn card_actions_support_local_pointer_and_keyboard_submission() {
    let mut card = Card::new().title("Profile").actions(vec!["Save", "Cancel"]);
    card.set_frame_for_test(Rect::new(80.0, 40.0, 200.0, 120.0));

    assert_eq!(card.tab_index(), 1);
    assert_eq!(
        card.on_event(&SystemEvent::PointerDown {
            pos: Point::new(150.0, 100.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let event = card
        .semantic_event(ComponentId::new(4), &SystemEvent::FocusIn)
        .expect("pointer action should emit submit");
    assert_eq!(event.kind, SemanticKind::Submit);
    assert_eq!(event.text_payload(), Some("Cancel"));
    assert_eq!(card.focused_action(), Some(1));

    assert_eq!(
        card.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Home,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        card.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let event = card
        .semantic_event(ComponentId::new(4), &SystemEvent::FocusIn)
        .expect("keyboard action should emit submit");
    assert_eq!(event.text_payload(), Some("Save"));
}

#[test]
fn card_normalizes_geometry_and_exposes_focused_action_semantics() {
    let mut card = Card::new()
        .title("Profile")
        .actions(vec!["", "Save"])
        .size(f32::NAN, -1.0)
        .padding(f32::NAN)
        .flex_grow(-2.0);
    assert_eq!(card.action_labels(), &["Save"]);
    assert_eq!(
        card.measure(Constraints::loose(Size::new(500.0, 500.0))),
        Size::new(200.0, 120.0)
    );
    card.set_frame_for_test(Rect::new(0.0, 0.0, 200.0, 120.0));
    let _ = card.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });

    let accessibility = card.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Group);
    assert_eq!(accessibility.name.as_deref(), Some("Profile"));
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Save"));
    assert_eq!(accessibility.state.value_now, Some(1.0));
}

#[test]
fn card_arrange_uses_precomputed_measurements() {
    let calls = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Card::new().size(120.0, 80.0).padding(16.0)));
    let child = tree.add_child(
        root,
        Box::new(CountingChild {
            size: Size::new(200.0, 120.0),
            measure_calls: Rc::clone(&calls),
        }),
    );
    let frame = Rect::new(0.0, 0.0, 120.0, 80.0);
    let widget = tree.get(root).unwrap().as_layout().unwrap();
    let measured = widget.measure_children(frame, &[child], &tree);

    assert_eq!(calls.get(), 1);
    let placements = widget.layout_children(frame, &measured, &tree);

    assert_eq!(calls.get(), 1, "arrange must not measure children again");
    assert_eq!(placements[0].1, Rect::new(16.0, 16.0, 88.0, 48.0));
}
