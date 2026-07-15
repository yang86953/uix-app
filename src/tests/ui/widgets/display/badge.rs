use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::{Badge, BadgeStatus};
use crate::ui::{AccessibilityRole, WidgetRender};

fn render(badge: &Badge, canvas: &mut SharedRasterizer, fonts: &FontService, font: FontHandle) {
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut ctx = PaintContext::new_for_test(
        canvas,
        font,
        fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        24,
    );
    WidgetRender::render(badge, Rect::new(0.0, 0.0, 100.0, 20.0), &mut ctx, &tree);
}

#[test]
fn status_and_dot_text_participate_in_measurement_and_accessibility() {
    let constraints = Constraints::loose(Size::new(300.0, 100.0));
    let status = Badge::new().status(BadgeStatus::Success).text("Ready");
    let dot = Badge::new().dot().text("新消息");

    assert_eq!(status.measure(constraints), Size::new(53.0, 20.0));
    assert_eq!(dot.measure(constraints), Size::new(60.0, 20.0));
    let accessibility = status.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Status);
    assert_eq!(accessibility.name.as_deref(), Some("Ready"));

    let mut font_service = FontService::new();
    let Some(font) = font_service.load_font_from_path(r"C:\Windows\Fonts\segoeui.ttf", 13.0) else {
        return;
    };
    let mut canvas = SharedRasterizer::new(PixelSurface::new(100, 24));
    render(&status, &mut canvas, &font_service, font);
    assert!(
        canvas
            .surface()
            .pixels()
            .chunks_exact(100)
            .any(|row| row[18..70].iter().any(|pixel| *pixel != 0)),
        "status text must paint to the right of the marker"
    );
}

#[test]
fn count_and_max_are_normalized_and_expose_the_actual_value() {
    let badge = Badge::new().count(120).max(99);
    let accessibility = badge.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Status);
    assert_eq!(accessibility.name.as_deref(), Some("99+"));
    assert_eq!(accessibility.state.value_now, Some(120.0));
    assert_eq!(accessibility.state.value_max, Some(99.0));

    let normalized = Badge::new().count(-5).max(0).show_zero(true);
    assert!(matches!(
        normalized.snapshot_fields(),
        SnapshotFields::Badge {
            count: 0,
            max: 1,
            ..
        }
    ));
}

#[test]
fn hidden_zero_badge_is_excluded_from_accessibility() {
    let badge = Badge::new();
    assert_eq!(
        badge.snapshot_fields().accessibility().role,
        AccessibilityRole::None
    );
}
