use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::navigation::anchor::*;
use crate::ui::widgets::Label;
use crate::ui::LayoutChild;

fn render_anchor(anchor: &Anchor, frame: Rect) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(480, 240));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
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
            480,
            240,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(anchor, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn anchor_click_emits_change_semantic_event() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Anchor::new(vec![
        AnchorItem::new("基础", "#basic"),
        AnchorItem::new("高级", "#advanced"),
        AnchorItem::new("API", "#api"),
    ])));
    tree.get_mut(id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 160.0, 108.0));

    let href = Rc::new(RefCell::new(String::new()));
    let href_for_handler = href.clone();
    tree.handler_table()
        .on(id, SemanticKind::Change, move |event| {
            if let Some(value) = event.text_payload() {
                *href_for_handler.borrow_mut() = value.to_string();
            }
        });

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(20.0, 80.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(&*href.borrow(), "#api");
}

#[test]
fn anchor_is_focusable_and_keyboard_navigation_emits_href() {
    let mut anchor = Anchor::new(vec![
        AnchorItem::new("Basic", "#basic"),
        AnchorItem::new("Advanced", "#advanced"),
    ]);
    let event = SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&anchor), 1);
    assert_eq!(anchor.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(anchor.on_event(&event), EventResult::Handled);
    assert_eq!(anchor.active_href(), "#advanced");
    assert_eq!(
        anchor
            .semantic_event(ComponentId::new(8), &event)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("#advanced".to_string())
    );
    assert_eq!(
        anchor.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}

#[test]
fn anchor_enter_reactivates_current_target() {
    let mut anchor = Anchor::new(vec![AnchorItem::new("Basic", "#basic")]);
    let event = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };

    assert_eq!(anchor.on_event(&event), EventResult::Handled);
    assert_eq!(
        anchor
            .semantic_event(ComponentId::new(9), &event)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("#basic".to_string())
    );
}

#[test]
fn anchor_snapshot_and_accessibility_expose_active_item() {
    let mut anchor = Anchor::new(vec![
        AnchorItem::new("Basic", "#basic"),
        AnchorItem::new("Advanced", "#advanced"),
    ]);
    anchor.update_active(100.0);
    let fields = anchor.snapshot_fields();

    assert!(matches!(
        fields,
        SnapshotFields::Anchor {
            active_index: 1,
            ..
        }
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Advanced"));
    assert_eq!(accessibility.state.value_now, Some(2.0));
}

#[test]
fn empty_anchor_is_not_focusable_and_negative_pointer_is_ignored() {
    let mut empty = Anchor::new(Vec::new());
    assert_eq!(WidgetComponent::tab_index(&empty), 0);
    assert_eq!(
        empty.on_event(&SystemEvent::PointerDown {
            pos: Point::new(0.0, -1.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}

#[test]
fn anchor_custom_container_view_materializes_as_a_real_child() {
    let tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Anchor::new(vec![
            AnchorItem::new("Basic", "#basic"),
            AnchorItem::new("Advanced", "#advanced"),
        ])
        .container(|| crate::ui::view::label("custom")),
    ));

    assert!(tree.find_by_type::<Label>().is_some());
}

#[test]
fn anchor_custom_container_reserves_a_real_content_viewport_and_keeps_navigation_active() {
    let mut anchor = Anchor::new(vec![
        AnchorItem::new("Basic", "#basic"),
        AnchorItem::new("Advanced", "#advanced"),
    ])
    .container(|| crate::ui::view::label("custom scroll viewport"));
    assert_eq!(
        WidgetLayout::measure(&anchor, Constraints::unconstrained()),
        Size::new(464.0, 240.0)
    );

    let child = ComponentId::new(41);
    assert_eq!(
        WidgetLayout::layout_children(
            &anchor,
            Rect::new(10.0, 20.0, 500.0, 240.0),
            &[LayoutChild::new(child, Size::new(320.0, 240.0))],
            &WidgetTree::new(),
        ),
        vec![(child, Rect::new(154.0, 20.0, 356.0, 240.0))]
    );

    assert_eq!(
        anchor.on_event(&SystemEvent::PointerDown {
            pos: Point::new(200.0, 54.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled,
        "the custom content viewport must not masquerade as an anchor row"
    );
    assert_eq!(anchor.active_href(), "#basic");
    assert_eq!(
        anchor.on_event(&SystemEvent::PointerDown {
            pos: Point::new(20.0, 54.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(anchor.active_href(), "#advanced");
}

#[test]
fn target_offset_changes_scroll_spy_threshold_and_is_exposed_in_snapshot() {
    let items = vec![
        AnchorItem::new("Basic", "#basic"),
        AnchorItem::new("Advanced", "#advanced"),
    ];
    let mut without_offset = Anchor::new(items.clone()).bounds(0.0);
    without_offset.set_positions(vec![0.0, 100.0]);
    without_offset.update_active(95.0);
    assert_eq!(without_offset.active_href(), "#basic");

    let mut with_offset = Anchor::new(items).bounds(0.0).target_offset(10.0);
    with_offset.set_positions(vec![0.0, 100.0]);
    with_offset.update_active(95.0);
    assert_eq!(with_offset.active_href(), "#advanced");
    assert!(matches!(
        with_offset.snapshot_fields(),
        SnapshotFields::Anchor {
            offset_top: 10.0,
            ..
        }
    ));
}

#[test]
fn anchor_bounds_changes_the_scroll_detection_boundary() {
    let make_anchor = |bounds| {
        Anchor::new(vec![
            AnchorItem::new("Basic", "#basic"),
            AnchorItem::new("Advanced", "#advanced"),
        ])
        .bounds(bounds)
    };
    let mut strict = make_anchor(0.0);
    strict.set_positions(vec![0.0, 100.0]);
    strict.update_active(95.0);
    let mut tolerant = make_anchor(10.0);
    tolerant.set_positions(vec![0.0, 100.0]);
    tolerant.update_active(95.0);

    assert_eq!(strict.active_href(), "#basic");
    assert_eq!(tolerant.active_href(), "#advanced");
}

#[test]
fn show_ink_controls_real_active_indicator_geometry() {
    let visible = Anchor::new(vec![AnchorItem::new("Basic", "#basic")]).show_ink(true);
    let hidden = Anchor::new(vec![AnchorItem::new("Basic", "#basic")]).show_ink(false);
    let frame = Rect::new(0.0, 0.0, 160.0, 72.0);
    let visible_render = render_anchor(&visible, frame);
    let hidden_render = render_anchor(&hidden, frame);

    assert!(
        visible_render.contains("w: 3.0, h: 36.0"),
        "active ink must be a real three-pixel indicator: {visible_render}"
    );
    assert!(
        !hidden_render.contains("w: 3.0, h: 36.0"),
        "show_ink(false) must remove indicator geometry: {hidden_render}"
    );
}
