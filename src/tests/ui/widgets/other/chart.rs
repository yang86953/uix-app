use crate::tests::common::*;
use crate::ui::widgets::{BarChart, BarData};
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
