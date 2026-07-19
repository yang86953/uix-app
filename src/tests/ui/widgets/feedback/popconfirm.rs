use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::traits::{EventHandler, WidgetRender};
use crate::ui::widgets::{Popconfirm, PopconfirmPlacement};
use crate::ui::{LayoutChild, SemanticKind};

fn render_popconfirm(popconfirm: &Popconfirm, frame: Rect, surface_size: (i32, i32)) -> String {
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
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(popconfirm, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

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

#[test]
fn popconfirm_requires_matching_release_for_trigger_and_confirm() {
    let mut popconfirm = Popconfirm::new().title("删除基线？");
    let id = ComponentId::new(7);
    let trigger = Point::new(20.0, 12.0);

    assert_eq!(
        popconfirm.on_event(&pointer_down(trigger)),
        EventResult::Handled
    );
    assert!(!popconfirm.is_visible());
    assert_eq!(
        popconfirm.on_event(&pointer_up(trigger)),
        EventResult::Handled
    );
    assert!(popconfirm.is_visible());

    let confirm = Point::new(20.0, -40.0);
    let cancel = Point::new(120.0, -40.0);
    assert_eq!(
        popconfirm.on_event(&pointer_down(confirm)),
        EventResult::Handled
    );
    assert!(popconfirm
        .semantic_event(id, &pointer_down(confirm))
        .is_none());
    assert_eq!(
        popconfirm.on_event(&pointer_up(cancel)),
        EventResult::Handled
    );
    assert!(popconfirm.is_visible());
    assert!(popconfirm.semantic_event(id, &pointer_up(cancel)).is_none());

    assert_eq!(
        popconfirm.on_event(&pointer_down(confirm)),
        EventResult::Handled
    );
    let release = pointer_up(confirm);
    assert_eq!(popconfirm.on_event(&release), EventResult::Handled);
    let semantic = popconfirm
        .semantic_event(id, &release)
        .expect("matching release submits confirmation");
    assert_eq!(semantic.kind, SemanticKind::Submit);
    assert_eq!(semantic.text_payload(), Some("confirm"));
    assert!(!popconfirm.is_visible());
}

#[test]
fn constrained_popconfirm_flips_clips_elides_and_lays_out_trigger_child() {
    let mut popconfirm = Popconfirm::new()
        .title("超长中英文 mixed confirmation title that must be elided")
        .confirm_text("确认并永久删除")
        .cancel_text("取消并保留全部内容")
        .placement(PopconfirmPlacement::Top);
    popconfirm.open();
    let frame = Rect::new(100.0, 4.0, 80.0, 28.0);
    let commands = render_popconfirm(&popconfirm, frame, (132, 120));

    assert!(commands.contains("PushClip { rect: Rect { x: 0.0, y: 0.0, w: 132.0, h: 120.0 } }"));
    assert!(commands.contains('…'));
    assert!(!commands.contains("mixed confirmation title that must be elided"));
    assert!(!commands.contains('⚠'));
    assert!(!commands.contains("w: -") && !commands.contains("h: -"));
    let overlay = WidgetRender::overlay_entry(&popconfirm, ComponentId::new(9), frame)
        .expect("open popconfirm overlay");
    let bounds = overlay.bounds_rect().expect("bounded overlay");
    assert!(Rect::new(0.0, 0.0, 132.0, 120.0).contains(Point::new(
        bounds.x + bounds.w - 0.01,
        bounds.y + bounds.h - 0.01,
    )));

    let child = ComponentId::new(11);
    assert_eq!(
        popconfirm.layout_children(
            frame,
            &[LayoutChild::new(child, Size::new(20.0, 10.0))],
            &WidgetTree::new(),
        ),
        vec![(child, frame)]
    );
}
