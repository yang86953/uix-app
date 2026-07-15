use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::{BarChart, BarData, LineChart, LineData, PieChart, PieData};
use crate::ui::AccessibilityRole;

#[test]
fn bar_chart_maps_signed_and_non_finite_values_inside_the_plot() {
    let chart = BarChart::new().data(vec![
        BarData::new("gain", 10.0, Color::green()),
        BarData::new("loss", -5.0, Color::red()),
        BarData::new("missing", f32::NAN, Color::blue()),
    ]);
    let frame = Rect::new(10.0, 20.0, 240.0, 160.0);
    let (baseline, bars) = chart
        .geometry_for_test(frame)
        .expect("signed chart should have a finite plot");

    assert_eq!(bars.len(), 3);
    assert!(bars[0].y < baseline && (bars[0].y + bars[0].h - baseline).abs() < 0.01);
    assert!((bars[1].y - baseline).abs() < 0.01 && bars[1].h > 0.0);
    assert!((bars[2].y - baseline).abs() < 0.01 && bars[2].h == 0.0);
    for bar in bars {
        assert!(bar.x.is_finite() && bar.y.is_finite());
        assert!(bar.w.is_finite() && bar.h.is_finite());
        assert!(bar.x >= frame.x && bar.x + bar.w <= frame.x + frame.w + 0.01);
        assert!(bar.y >= frame.y && bar.y + bar.h <= frame.y + frame.h + 0.01);
    }
}

#[test]
fn bar_chart_normalizes_geometry_and_exposes_chart_semantics() {
    let chart = BarChart::new()
        .data(vec![
            BarData::new("Q1", 12.0, Color::blue()),
            BarData::new("unknown", f32::INFINITY, Color::red()),
        ])
        .width(f32::NAN)
        .height(-20.0)
        .max_value(f32::INFINITY)
        .bar_radius(f32::NAN);
    let measured = chart.measure(Constraints::loose(Size::new(120.0, 80.0)));
    assert_eq!(measured, Size::new(120.0, 80.0));

    let accessibility = chart.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Image);
    assert_eq!(accessibility.name.as_deref(), Some("Bar chart"));
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("Q1: 12; unknown: 0")
    );
    assert!(matches!(
        chart.snapshot_fields(),
        SnapshotFields::BarChart {
            fixed_width: 0.0,
            fixed_height: 0.0,
            max_value: 0.0,
            bar_radius: 0.0,
            ..
        }
    ));
}

#[test]
fn line_chart_keeps_a_single_point_visible_and_normalizes_non_finite_data() {
    let frame = Rect::new(20.0, 10.0, 220.0, 140.0);
    let single = LineChart::new()
        .data(vec![LineData::new("only", 5.0)])
        .auto_min(true);
    let (_, points) = single
        .geometry_for_test(frame)
        .expect("single point should produce plot geometry");
    assert_eq!(points.len(), 1);
    assert!((points[0].x - (frame.x + 36.0 + (frame.w - 36.0) * 0.5)).abs() < 0.01);
    assert!(points[0].y >= frame.y && points[0].y <= frame.y + frame.h);

    let invalid = LineChart::new().data(vec![
        LineData::new("start", f32::NEG_INFINITY),
        LineData::new("end", 4.0),
    ]);
    let (_, points) = invalid
        .geometry_for_test(frame)
        .expect("non-finite values should normalize instead of poisoning geometry");
    assert!(points
        .iter()
        .all(|point| point.x.is_finite() && point.y.is_finite()));
}

#[test]
fn line_chart_normalizes_public_geometry_and_exposes_chart_semantics() {
    let chart = LineChart::new()
        .data(vec![
            LineData::new("Mon", 5.0),
            LineData::new("unknown", f32::NAN),
        ])
        .width(f32::NAN)
        .height(-1.0)
        .max_value(f32::INFINITY)
        .line_width(f32::NAN)
        .dot_radius(-3.0);
    assert_eq!(
        chart.measure(Constraints::loose(Size::new(130.0, 90.0))),
        Size::new(130.0, 90.0)
    );

    let accessibility = chart.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Image);
    assert_eq!(accessibility.name.as_deref(), Some("Line chart"));
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("Mon: 5; unknown: 0")
    );
    assert!(matches!(
        chart.snapshot_fields(),
        SnapshotFields::LineChart {
            fixed_width: 0.0,
            fixed_height: 0.0,
            max_value: 0.0,
            line_width: 1.0,
            dot_radius: 0.0,
            ..
        }
    ));
}

#[test]
fn pie_chart_ignores_invalid_slices_and_normalizes_remaining_fractions() {
    let chart = PieChart::new().data(vec![
        PieData::new("used", 40.0, Color::blue()),
        PieData::new("negative", -10.0, Color::red()),
        PieData::new("unknown", f32::NAN, Color::green()),
        PieData::new("free", 60.0, Color::from_rgb(250, 200, 40)),
    ]);
    let slices = chart.slices_for_test();

    assert_eq!(slices.len(), 2);
    assert_eq!(slices[0].0, "used");
    assert_eq!(slices[1].0, "free");
    assert!((slices[0].1 - 0.4).abs() < 0.0001);
    assert!((slices[1].1 - 0.6).abs() < 0.0001);
    assert!((slices.iter().map(|slice| slice.1).sum::<f32>() - 1.0).abs() < 0.0001);
}

#[test]
fn pie_chart_normalizes_geometry_and_exposes_only_valid_slices() {
    let chart = PieChart::new()
        .data(vec![
            PieData::new("valid", 7.5, Color::blue()),
            PieData::new("zero", 0.0, Color::red()),
            PieData::new("unknown", f32::INFINITY, Color::green()),
        ])
        .size(f32::NAN)
        .donut(f32::NAN);
    assert_eq!(
        chart.measure(Constraints::loose(Size::new(100.0, 80.0))),
        Size::new(100.0, 80.0)
    );

    let accessibility = chart.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Image);
    assert_eq!(accessibility.name.as_deref(), Some("Pie chart"));
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("valid: 7.5")
    );
    assert!(matches!(
        chart.snapshot_fields(),
        SnapshotFields::PieChart {
            fixed_size: 0.0,
            hole_radius: 0.0,
            ..
        }
    ));
    assert!(matches!(
        PieChart::new().donut(2.0).snapshot_fields(),
        SnapshotFields::PieChart {
            hole_radius: 0.9,
            ..
        }
    ));
}

#[test]
fn pie_chart_renders_a_labeled_legend_in_reserved_space() {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(200, 120));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        font,
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        200,
        120,
    );
    let chart = PieChart::new().data(vec![
        PieData::new("used", 40.0, Color::blue()),
        PieData::new("free", 60.0, Color::green()),
    ]);
    WidgetRender::render(&chart, Rect::new(0.0, 0.0, 200.0, 120.0), &mut ctx, &tree);

    assert!(
        (4..16).any(|y| {
            canvas.surface().pixels()[y * 200 + 138..y * 200 + 150]
                .iter()
                .any(|pixel| *pixel != 0)
        }),
        "legend swatch or label should paint to the right of the pie area"
    );
}
