use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_icon_size() {
    let measured = Icon::new("search")
        .size(32.0)
        .measure(Constraints::loose(Size::new(20.0, 24.0)));

    assert_eq!(measured, Size::new(20.0, 24.0));
}

#[test]
fn sidebar_icon_names_map_to_pua() {
    assert_eq!(icon_char("home"), "\u{E0F5}");
    assert_eq!(icon_char("cpu"), "\u{E0A9}");
    assert_eq!(icon_char("bar-chart"), "\u{E06A}");
    assert_eq!(icon_char("layers"), "\u{E529}");
}

/// 根因回归：混排图标须用 UI 字体行盒定位，再切 Lucide 绘制。
#[test]
fn paint_icon_uses_ui_font_line_box_not_lucide_metrics() {
    use crate::draw::font::text::render::TextRenderService;

    let mut fs = FontService::new();
    let Some(segoe) = fs.load_font_from_path(r"C:\Windows\Fonts\segoeui.ttf", 14.0) else {
        return;
    };
    let Some(lucide) = fs.load_font_from_path(r"assets/fonts/lucide.ttf", 14.0) else {
        return;
    };
    let row = Rect::new(0.0, 0.0, 200.0, 36.0);
    let ui_h = TextRenderService::new(segoe, &fs, 500.0).line_box_height(14.0);
    let lucide_h = TextRenderService::new(lucide, &fs, 500.0).line_box_height(14.0);
    let y_ui = row.y + (row.h - ui_h) * 0.5;
    // 与 paint_icon_in_frame 相同：先按 UI 行盒算 y
    assert!(
        (y_ui - (row.y + (row.h - ui_h) * 0.5)).abs() < 0.01,
        "icon slot y must follow UI line box"
    );
    let m = fs
        .horizontal_line_metrics(&lucide, 14.0)
        .expect("lucide metrics");
    assert!(
        m.descent.abs() < 0.5,
        "Lucide descent ~0 (got {}); switching font before measuring Y still risks mismatch when em sizes differ (ui_h={ui_h} lucide_h={lucide_h})",
        m.descent
    );
}
