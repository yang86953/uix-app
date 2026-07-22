use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::animation::AnimationConfig;
use crate::ui::view::{label, ViewAdapter};
use crate::ui::widgets::{
    AreaChart, AxisSide, BarChart, BarData, BrushConfig, ChartPlaceholder, ChartSeries, ChartType,
    ComboChart, ComboSeries, FunnelAlign, FunnelChart, FunnelData, FunnelShape, Gauge, GaugeRange,
    GaugeType, Heatmap, HeatmapCell, InteractionConfig, LabelPosition, LegendPosition, LineChart,
    LineData, LineStyle, PieChart, PieData, PointStyle, RadarAxis, RadarChart, RadarData,
    RadarShape, RoseStyle, ScatterChart, ScatterData, TooltipConfig, TooltipDatum, TooltipTrigger,
    Treemap, TreemapNode, WaterfallChart, WaterfallData, WaterfallKind,
};
use crate::ui::{AccessibilityRole, ConfigProvider};

fn render_advanced_chart<T: WidgetRender>(chart: &T, frame: Rect) -> String {
    let width = (frame.x + frame.w).ceil().max(1.0) as i32;
    let height = (frame.y + frame.h).ceil().max(1.0) as i32;
    let mut canvas = SharedRasterizer::new(PixelSurface::new(width, height));
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
            width,
            height,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(chart, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn paint_op_count(display_list: &str, operation: &str) -> usize {
    display_list.matches(&format!("{operation} {{")).count()
}

fn render_chart_pixels<T: WidgetRender>(chart: &T, frame: Rect) -> Vec<u32> {
    let width = (frame.x + frame.w).ceil().max(1.0) as i32;
    let height = (frame.y + frame.h).ceil().max(1.0) as i32;
    let mut canvas = SharedRasterizer::new(PixelSurface::new(width, height));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
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
            width,
            height,
        );
        WidgetRender::render(chart, frame, &mut ctx, &tree);
    }
    canvas.surface().pixels().to_vec()
}

static ADVANCED_CHART_CLICK_LABEL: Mutex<String> = Mutex::new(String::new());
static ADVANCED_CHART_BRUSH_RANGE: Mutex<Option<(f32, f32)>> = Mutex::new(None);

fn record_advanced_chart_click(label: &str) {
    *ADVANCED_CHART_CLICK_LABEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = label.to_owned();
}

fn record_advanced_chart_brush(range: (f32, f32)) {
    *ADVANCED_CHART_BRUSH_RANGE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(range);
}

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

#[test]
fn pie_chart_feature_matrix_renders_rose_radii_arc_endpoint_and_positioned_legend() {
    let data = vec![
        PieData::new("small", 20.0, Color::blue()),
        PieData::new("large", 80.0, Color::green()),
    ];
    let frame = Rect::new(0.0, 0.0, 240.0, 180.0);
    let plain_chart = PieChart::new()
        .data(data.clone())
        .legend(LegendPosition::None);
    let rose_chart = PieChart::new()
        .data(data.clone())
        .rose(true)
        .rose_style(RoseStyle::Radius)
        .label_position(LabelPosition::Outside)
        .legend(LegendPosition::None);
    let rose = render_advanced_chart(&rose_chart, frame);
    let rose_geometry = rose_chart.sector_geometry_for_test();
    assert_eq!(rose_geometry.len(), 2);
    assert!((rose_geometry[0].0 - 180.0).abs() < 0.001);
    assert!((rose_geometry[1].0 - 180.0).abs() < 0.001);
    assert!((rose_geometry[0].1 - 0.25).abs() < 0.001);
    assert!((rose_geometry[1].1 - 1.0).abs() < 0.001);
    assert_ne!(
        render_chart_pixels(&plain_chart, frame),
        render_chart_pixels(&rose_chart, frame),
        "rose mode must change painted sector radii"
    );
    assert!(!rose.contains("NaN"), "{rose}");

    let half = PieChart::new()
        .data(data.clone())
        .start_angle(180.0)
        .total(360.0);
    assert_eq!(half.sweep_degrees_for_test(), 180.0);
    assert!(
        (half
            .slices_for_test()
            .iter()
            .map(|slice| slice.1)
            .sum::<f32>()
            - 1.0)
            .abs()
            < 0.001
    );
    assert_eq!(
        PieChart::new()
            .data(data.clone())
            .start_angle(45.0)
            .end_angle(225.0)
            .sweep_degrees_for_test(),
        180.0
    );
    assert_eq!(
        PieChart::new()
            .data(data.clone())
            .total(f32::NAN)
            .sweep_degrees_for_test(),
        360.0
    );

    for position in [
        LegendPosition::Top,
        LegendPosition::Bottom,
        LegendPosition::Left,
        LegendPosition::Right,
    ] {
        let display =
            render_advanced_chart(&PieChart::new().data(data.clone()).legend(position), frame);
        assert!(display.contains("small 20%"), "{position:?}: {display}");
        assert!(display.contains("large 80%"), "{position:?}: {display}");
        assert!(!display.contains("NaN"), "{position:?}: {display}");
    }
}

#[test]
fn bar_chart_series_support_grouped_and_stacked_geometry() {
    let first = vec![
        BarData::new("A", 10.0, Color::blue()),
        BarData::new("B", 20.0, Color::blue()),
    ];
    let second = vec![
        BarData::new("A", 5.0, Color::green()),
        BarData::new("B", 8.0, Color::green()),
    ];
    let grouped = BarChart::new()
        .series(vec![
            ChartSeries::new("first", first.clone()),
            ChartSeries::new("second", second.clone()),
        ])
        .grouped(true);
    let grouped_items = grouped
        .geometry_items_for_test(Rect::new(0.0, 0.0, 240.0, 160.0))
        .expect("grouped chart geometry");
    assert_eq!(grouped_items.len(), 4);
    assert!(grouped_items[0].2.x < grouped_items[1].2.x);

    let stacked = BarChart::new()
        .series(vec![
            ChartSeries::new("first", first),
            ChartSeries::new("second", second),
        ])
        .stacked(true);
    let stacked_items = stacked
        .geometry_items_for_test(Rect::new(0.0, 0.0, 240.0, 160.0))
        .expect("stacked chart geometry");
    assert_eq!(stacked_items.len(), 4);
    assert!(stacked_items[0].2.y > stacked_items[1].2.y);
}

#[test]
fn bar_chart_feature_matrix_has_independent_gaps_and_horizontal_series_layout() {
    let first = vec![
        BarData::new("A", 10.0, Color::blue()),
        BarData::new("B", 20.0, Color::blue()),
    ];
    let second = vec![
        BarData::new("A", 5.0, Color::green()),
        BarData::new("B", 8.0, Color::green()),
    ];
    let series = || {
        vec![
            ChartSeries::new("first", first.clone()),
            ChartSeries::new("second", second.clone()),
        ]
    };
    let frame = Rect::new(10.0, 20.0, 280.0, 180.0);

    let compact = BarChart::new()
        .series(series())
        .grouped(true)
        .category_gap(0.1)
        .bar_gap(0.1)
        .geometry_items_for_test(frame)
        .expect("compact grouped bars");
    let separated_categories = BarChart::new()
        .series(series())
        .grouped(true)
        .category_gap(0.6)
        .bar_gap(0.1)
        .geometry_items_for_test(frame)
        .expect("wide category gap");
    let separated_bars = BarChart::new()
        .series(series())
        .grouped(true)
        .category_gap(0.1)
        .bar_gap(0.7)
        .geometry_items_for_test(frame)
        .expect("wide bar gap");
    assert!(compact[0].2.w > separated_categories[0].2.w);
    assert!(compact[0].2.w > separated_bars[0].2.w);
    assert!(compact[0].2.x + compact[0].2.w <= compact[1].2.x);

    let horizontal = BarChart::new()
        .series(series())
        .grouped(true)
        .horizontal(true)
        .category_gap(0.2)
        .bar_gap(0.2)
        .geometry_items_for_test(frame)
        .expect("horizontal grouped bars");
    assert!(horizontal[0].2.y < horizontal[1].2.y);
    assert!(horizontal.iter().all(|(_, _, rect)| {
        rect.x.is_finite()
            && rect.y.is_finite()
            && rect.w.is_finite()
            && rect.h.is_finite()
            && rect.y >= frame.y
            && rect.y + rect.h <= frame.y + frame.h
    }));

    let horizontal_stacked = BarChart::new()
        .series(series())
        .stacked(true)
        .horizontal(true)
        .geometry_items_for_test(frame)
        .expect("horizontal stacked bars");
    assert!((horizontal_stacked[0].2.y - horizontal_stacked[1].2.y).abs() < 0.01);
    assert!(
        (horizontal_stacked[0].2.x + horizontal_stacked[0].2.w - horizontal_stacked[1].2.x).abs()
            < 0.01
    );

    let display = render_advanced_chart(
        &BarChart::new()
            .series(series())
            .grouped(true)
            .title("Sales")
            .subtitle("FY26")
            .legend(LegendPosition::Left),
        frame,
    );
    for evidence in ["Sales", "FY26", "first", "second", "FillRect"] {
        assert!(
            display.contains(evidence),
            "missing `{evidence}`: {display}"
        );
    }
    assert!(!display.contains("NaN"), "{display}");
}

#[test]
fn line_chart_series_exposes_one_point_set_per_series() {
    let chart = LineChart::new().series(vec![
        ChartSeries::new(
            "first",
            vec![LineData::new("A", 10.0), LineData::new("B", 20.0)],
        ),
        ChartSeries::new(
            "second",
            vec![LineData::new("A", 4.0), LineData::new("B", 8.0)],
        ),
    ]);
    let points = chart
        .series_points_for_test(Rect::new(0.0, 0.0, 240.0, 160.0))
        .expect("multi-series line geometry");
    assert_eq!(points.len(), 2);
    assert_eq!(points[0].len(), 2);
    assert_eq!(points[1].len(), 2);
}

#[test]
fn line_chart_feature_matrix_renders_named_series_catmull_rom_and_step_paths() {
    let data = vec![
        LineData::new("A", 10.0),
        LineData::new("B", 40.0),
        LineData::new("C", 5.0),
        LineData::new("D", 30.0),
    ];
    let frame = Rect::new(0.0, 0.0, 320.0, 200.0);
    let base = || {
        LineChart::new()
            .data(data.clone())
            .show_grid(false)
            .show_dots(false)
            .line_width(1.0)
    };
    let smooth = render_advanced_chart(&base().smooth(true), frame);
    let step = render_advanced_chart(&base().step(true), frame);
    let straight_pixels = render_chart_pixels(&base(), frame);
    let smooth_pixels = render_chart_pixels(&base().smooth(true), frame);
    let step_pixels = render_chart_pixels(&base().step(true), frame);
    assert_ne!(
        smooth_pixels, straight_pixels,
        "smooth must bend between data points"
    );
    assert_ne!(
        step_pixels, straight_pixels,
        "step must use orthogonal segments"
    );
    assert_ne!(
        smooth_pixels, step_pixels,
        "smooth and step paths must differ"
    );
    assert!(!smooth.contains("NaN"), "{smooth}");
    assert!(!step.contains("NaN"), "{step}");

    let series_display = render_advanced_chart(
        &LineChart::new()
            .series(vec![
                ChartSeries::new("CPU", data.clone()),
                ChartSeries::new("Memory", data),
            ])
            .legend(LegendPosition::Bottom),
        frame,
    );
    assert!(series_display.contains("CPU"), "{series_display}");
    assert!(series_display.contains("Memory"), "{series_display}");
    assert!(paint_op_count(&series_display, "FillCircle") >= 8);
}

#[test]
fn basic_charts_share_responsive_interaction_brush_tooltip_and_reconcile_contracts() {
    ADVANCED_CHART_CLICK_LABEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    *ADVANCED_CHART_BRUSH_RANGE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    let interaction = InteractionConfig {
        zoom: true,
        pan: true,
        crosshair: true,
        on_click: Some(record_advanced_chart_click),
    };
    let brush = BrushConfig {
        enabled: true,
        on_select: Some(record_advanced_chart_brush),
    };
    let frame = Rect::new(0.0, 0.0, 200.0, 120.0);
    let make_chart = || {
        BarChart::new()
            .series(vec![ChartSeries::new(
                "sales",
                vec![
                    BarData::new("left", 10.0, Color::blue()),
                    BarData::new("right", 20.0, Color::green()),
                ],
            )])
            .interactive(interaction)
            .brush(brush)
            .tooltip(
                TooltipConfig::new()
                    .trigger(TooltipTrigger::Click)
                    .template("{series}/{label}={value}"),
            )
    };
    let mut chart = make_chart();
    let _ = render_advanced_chart(&chart, frame);
    assert_eq!(
        chart.tooltip_text_for_test(Point::new(20.0, 60.0), frame),
        Some("sales/left=10".to_owned())
    );
    assert_eq!(
        EventHandler::on_event(
            &mut chart,
            &SystemEvent::Wheel {
                pos: Point::new(50.0, 60.0),
                delta: Point::new(0.0, -1000.0),
            },
        ),
        EventResult::Handled
    );
    assert_eq!(
        EventHandler::on_event(
            &mut chart,
            &SystemEvent::PointerDown {
                pos: Point::new(40.0, 60.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );
    assert_eq!(
        chart.tooltip_position_for_test(),
        Some(Point::new(40.0, 60.0))
    );
    let pinned_tooltip = render_advanced_chart(&chart, frame);
    assert!(pinned_tooltip.contains("sales/left=10"), "{pinned_tooltip}");
    assert_eq!(
        EventHandler::on_event(
            &mut chart,
            &SystemEvent::PointerMove {
                pos: Point::new(140.0, 60.0),
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );
    assert_eq!(
        EventHandler::on_event(
            &mut chart,
            &SystemEvent::PointerUp {
                pos: Point::new(140.0, 60.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );
    assert_eq!(chart.interaction_state_for_test(), (2.0, 100.0));
    assert_eq!(
        ADVANCED_CHART_CLICK_LABEL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_str(),
        "left"
    );
    assert_eq!(
        *ADVANCED_CHART_BRUSH_RANGE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        Some((0.2, 0.7))
    );
    chart.sync_from(make_chart());
    assert_eq!(chart.interaction_state_for_test(), (2.0, 100.0));
    chart.sync_from(BarChart::new().data(vec![BarData::new("plain", 1.0, Color::blue())]));
    assert_eq!(chart.interaction_state_for_test(), (1.0, 0.0));

    let line = LineChart::new()
        .data(vec![LineData::new("A", 2.5)])
        .tooltip(
            TooltipConfig::new()
                .render(|datum| format!("line:{}", datum.value.unwrap_or_default())),
        );
    assert_eq!(
        line.tooltip_text_for_test(Point::new(100.0, 50.0), frame),
        Some("line:2.5".to_owned())
    );
    assert_eq!(line.interaction_state_for_test(), (1.0, 0.0));
    assert_eq!(line.tooltip_position_for_test(), None);

    let pie = PieChart::new()
        .data(vec![
            PieData::new("small", 20.0, Color::blue()),
            PieData::new("large", 80.0, Color::green()),
        ])
        .legend(LegendPosition::None)
        .tooltip(TooltipConfig::new().template("{label}:{percentage}"));
    assert_eq!(
        pie.tooltip_text_for_test(Point::new(120.0, 30.0), frame),
        Some("small:20.0%".to_owned())
    );
    assert_eq!(pie.interaction_state_for_test(), (1.0, 0.0));
    assert_eq!(pie.tooltip_position_for_test(), None);

    let constraints = Constraints::loose(Size::new(460.0, 260.0));
    assert_eq!(
        BarChart::new().responsive(true).measure(constraints),
        Size::new(460.0, 260.0)
    );
    assert_eq!(
        LineChart::new().responsive(true).measure(constraints),
        Size::new(460.0, 260.0)
    );
    assert_eq!(
        PieChart::new().responsive(true).measure(constraints),
        Size::new(460.0, 260.0)
    );
}

#[test]
fn advanced_chart_builders_accept_heatmap_radar_and_combo_data() {
    let _heatmap = Heatmap::new()
        .x_labels(vec!["Mon", "Tue"])
        .y_labels(vec!["AM", "PM"])
        .color_stops(vec![Color::blue(), Color::green(), Color::red()])
        .data(vec![HeatmapCell::new(0, 0, 1.0)]);
    let _radar = RadarChart::new()
        .shape(RadarShape::Circle)
        .axes(vec![RadarAxis::new("CPU", 0.0..=100.0)])
        .series(vec![ChartSeries::new(
            "current",
            vec![RadarData::new(60.0)],
        )]);
    let _combo = crate::ui::widgets::ComboChart::new().series(vec![
        ComboSeries::new("sales", vec![LineData::new("Jan", 10.0)])
            .chart_type(crate::ui::widgets::ChartType::Bar),
        ComboSeries::new("rate", vec![LineData::new("Jan", 2.0)]),
    ]);
}

#[test]
fn advanced_chart_entries_dispatch_to_all_nine_real_painters() {
    let frame = Rect::new(0.0, 0.0, 320.0, 220.0);
    let charts = [
        (
            "area",
            AreaChart::new()
                .data(vec![LineData::new("Jan", 10.0), LineData::new("Feb", 20.0)])
                .fill_opacity(0.35),
            "FillCircle",
        ),
        (
            "scatter",
            ScatterChart::new().data(vec![ScatterData::new("sample", 1.0, 2.0)]),
            "FillCircle",
        ),
        (
            "radar",
            RadarChart::new()
                .axes(vec![
                    RadarAxis::new("CPU", 0.0..=100.0),
                    RadarAxis::new("RAM", 0.0..=100.0),
                    RadarAxis::new("IO", 0.0..=100.0),
                ])
                .series(vec![ChartSeries::new(
                    "host",
                    vec![
                        RadarData::new(60.0),
                        RadarData::new(75.0),
                        RadarData::new(40.0),
                    ],
                )]),
            "CPU",
        ),
        (
            "heatmap",
            Heatmap::new()
                .x_labels(vec!["Mon", "Tue"])
                .y_labels(vec!["AM", "PM"])
                .show_values(true)
                .data(vec![
                    HeatmapCell::new(0, 0, 12.0),
                    HeatmapCell::new(1, 1, 45.0),
                ]),
            "Mon",
        ),
        (
            "funnel",
            FunnelChart::new()
                .data(vec![
                    FunnelData::new("Visit", 100.0),
                    FunnelData::new("Buy", 50.0),
                ])
                .show_conversion_rate(true),
            "Visit",
        ),
        (
            "waterfall",
            WaterfallChart::new().data(vec![
                WaterfallData::new("Start", 100.0, WaterfallKind::Total),
                WaterfallData::new("Cost", -25.0, WaterfallKind::Decrease),
            ]),
            "Start",
        ),
        (
            "combo",
            ComboChart::new().series(vec![
                ComboSeries::new("volume", vec![LineData::new("Jan", 100.0)])
                    .chart_type(ChartType::Bar)
                    .y_axis(AxisSide::Left),
                ComboSeries::new("rate", vec![LineData::new("Jan", 20.0)])
                    .chart_type(ChartType::Line)
                    .y_axis(AxisSide::Right)
                    .line_style(LineStyle::Dashed),
            ]),
            "Jan",
        ),
        (
            "treemap",
            Treemap::new().data(vec![
                TreemapNode::new("Root", 10.0).children(vec![TreemapNode::new("Leaf", 10.0)])
            ]),
            "Leaf",
        ),
        (
            "gauge",
            Gauge::new()
                .value(75.0)
                .range_colors(vec![GaugeRange::new(0.0, 100.0, Color::green())])
                .format(|value| format!("{value:.1}%")),
            "75.0%",
        ),
    ];

    for (kind, chart, evidence) in charts {
        assert_eq!(chart.kind_for_test(), kind);
        let display_list = render_advanced_chart(&chart, frame);
        assert!(
            display_list.contains(evidence),
            "{kind} must emit its painter-specific evidence `{evidence}`: {display_list}"
        );
        assert!(!display_list.contains("NaN"), "{kind}: {display_list}");
        assert!(!display_list.contains("暂无数据"), "{kind}: {display_list}");
    }
}

#[test]
fn advanced_area_scatter_radar_and_heatmap_configs_change_real_paint_output() {
    let frame = Rect::new(0.0, 0.0, 360.0, 240.0);
    let area_series = || {
        vec![
            ChartSeries::new(
                "base",
                vec![LineData::new("Jan", 10.0), LineData::new("Feb", 20.0)],
            ),
            ChartSeries::new(
                "extra",
                vec![LineData::new("Jan", 5.0), LineData::new("Feb", 8.0)],
            ),
        ]
    };
    let area = AreaChart::new()
        .series(area_series())
        .stacked(true)
        .fill_opacity(0.4)
        .legend(LegendPosition::Right);
    let area_display = render_advanced_chart(&area, frame);
    let unstacked_area = AreaChart::new()
        .series(area_series())
        .stacked(false)
        .fill_opacity(0.4)
        .legend(LegendPosition::Right);
    assert_ne!(
        render_chart_pixels(&area, frame),
        render_chart_pixels(&unstacked_area, frame),
        "stacked area geometry must differ from overlapping series"
    );
    assert!(area_display.contains("base"), "{area_display}");
    assert!(area_display.contains("extra"), "{area_display}");

    let diamond = ScatterChart::new()
        .data(vec![ScatterData::new("sample", 1.0, 2.0)])
        .x_axis("height")
        .y_axis("weight")
        .point_size(7.0)
        .point_style(PointStyle::Diamond);
    let diamond_display = render_advanced_chart(&diamond, frame);
    assert!(diamond_display.contains("height"), "{diamond_display}");
    assert!(diamond_display.contains("weight"), "{diamond_display}");
    let cross = ScatterChart::new()
        .data(vec![ScatterData::new("sample", 1.0, 2.0)])
        .point_style(PointStyle::Cross);
    let bubble_display = render_advanced_chart(
        &ScatterChart::new()
            .data(vec![crate::ui::widgets::BubbleData::new(
                "population",
                1.0,
                2.0,
                12.0,
            )])
            .bubble_scale(1.5),
        frame,
    );
    assert!(paint_op_count(&bubble_display, "FillCircle") >= 1);
    assert_ne!(
        render_chart_pixels(&diamond, frame),
        render_chart_pixels(&cross, frame)
    );

    let radar = RadarChart::new()
        .axes(vec![
            RadarAxis::new("CPU", 100.0..=0.0),
            RadarAxis::new("RAM", 0.0..=100.0),
            RadarAxis::new("IO", 0.0..=100.0),
        ])
        .series(vec![ChartSeries::new(
            "host",
            vec![
                RadarData::new(75.0),
                RadarData::new(f32::NAN),
                RadarData::new(40.0),
            ],
        )])
        .shape(RadarShape::Circle)
        .grid_levels(usize::MAX)
        .fill_opacity(0.25);
    let radar_display = render_advanced_chart(&radar, frame);
    assert_eq!(paint_op_count(&radar_display, "StrokeCircle"), 64);
    assert!(radar_display.contains("CPU"), "{radar_display}");
    assert!(!radar_display.contains("NaN"), "{radar_display}");

    let heatmap = Heatmap::new()
        .x_labels(vec!["Mon", "Tue"])
        .y_labels(vec!["AM", "PM"])
        .color_stops(vec![Color::blue(), Color::green(), Color::red()])
        .cell_gap(3.0)
        .show_values(true)
        .data(vec![
            HeatmapCell::new(0, 0, 12.0),
            HeatmapCell::new(1, 1, 45.0),
        ]);
    let heatmap_display = render_advanced_chart(&heatmap, frame);
    for evidence in ["Mon", "PM", "12", "45"] {
        assert!(
            heatmap_display.contains(evidence),
            "missing {evidence}: {heatmap_display}"
        );
    }
    let calendar_display = render_advanced_chart(
        &Heatmap::new()
            .calendar_mode(true)
            .year(2024)
            .cell_size(12.0)
            .cell_gap(2.0)
            .data(vec![HeatmapCell::new(31, 0, 1.0)]),
        frame,
    );
    assert!(calendar_display.contains("2月"), "{calendar_display}");
    assert!(!calendar_display.contains("NaN"), "{calendar_display}");
}

#[test]
fn advanced_funnel_waterfall_combo_treemap_and_gauge_configs_are_painted() {
    let frame = Rect::new(0.0, 0.0, 380.0, 250.0);
    let funnel_data = vec![
        FunnelData::new("Visit", 100.0),
        FunnelData::new("Buy", 50.0),
    ];
    let aligned_chart = FunnelChart::new()
        .data(funnel_data.clone())
        .align(FunnelAlign::Right)
        .shape(FunnelShape::Normal)
        .gap(8.0)
        .show_conversion_rate(true)
        .label_position(LabelPosition::Right);
    let symmetric_chart = FunnelChart::new()
        .data(funnel_data)
        .shape(FunnelShape::Symmetric)
        .gap(f32::MAX)
        .show_conversion_rate(true);
    let aligned = render_advanced_chart(&aligned_chart, frame);
    let symmetric = render_advanced_chart(&symmetric_chart, frame);
    assert!(aligned.contains("Visit 100"), "{aligned}");
    assert!(aligned.contains("Buy 50 (50%)"), "{aligned}");
    assert_ne!(
        render_chart_pixels(&aligned_chart, frame),
        render_chart_pixels(&symmetric_chart, frame),
        "funnel shape/alignment must affect geometry"
    );
    assert!(!symmetric.contains("NaN"), "{symmetric}");
    assert!(!symmetric.contains("inf"), "{symmetric}");

    let waterfall = WaterfallChart::new()
        .data(vec![
            WaterfallData::new("Start", 100.0, WaterfallKind::Total),
            WaterfallData::new("Gain", 30.0, WaterfallKind::Increase),
            WaterfallData::new("Cost", -25.0, WaterfallKind::Decrease),
        ])
        .horizontal(true)
        .x_axis("project")
        .y_axis("amount");
    let waterfall_display = render_advanced_chart(&waterfall, frame);
    for evidence in ["Start", "Gain", "Cost"] {
        assert!(waterfall_display.contains(evidence), "{waterfall_display}");
    }
    assert!(waterfall_display.contains("project"), "{waterfall_display}");
    assert!(waterfall_display.contains("amount"), "{waterfall_display}");
    let vertical_waterfall = WaterfallChart::new().data(vec![
        WaterfallData::new("Start", 100.0, WaterfallKind::Total),
        WaterfallData::new("Gain", 30.0, WaterfallKind::Increase),
        WaterfallData::new("Cost", -25.0, WaterfallKind::Decrease),
    ]);
    assert_ne!(
        render_chart_pixels(&waterfall, frame),
        render_chart_pixels(&vertical_waterfall, frame),
        "horizontal waterfall must change bar and connector geometry"
    );

    let combo = ComboChart::new()
        .series(vec![
            ComboSeries::new(
                "sales",
                vec![LineData::new("Jan", 100.0), LineData::new("Feb", 140.0)],
            )
            .chart_type(ChartType::Bar)
            .y_axis(AxisSide::Left),
            ComboSeries::new(
                "margin",
                vec![LineData::new("Jan", 10.0), LineData::new("Feb", 15.0)],
            )
            .chart_type(ChartType::Area)
            .y_axis(AxisSide::Right)
            .line_style(LineStyle::Dashed),
        ])
        .y_axis_left("sales-axis")
        .y_axis_right("margin-axis")
        .reference_line(12.0, "target", LineStyle::Dashed)
        .legend(LegendPosition::Top);
    let combo_display = render_advanced_chart(&combo, frame);
    for evidence in [
        "sales-axis",
        "margin-axis",
        "target",
        "sales",
        "margin",
        "Jan",
    ] {
        assert!(
            combo_display.contains(evidence),
            "missing {evidence}: {combo_display}"
        );
    }
    assert!(paint_op_count(&combo_display, "FillRect") >= 2);

    let split_combo = ComboChart::new()
        .bar_series(vec![ChartSeries::new(
            "volume",
            vec![
                BarData::new("Jan", 100.0, Color::blue()),
                BarData::new("Feb", 200.0, Color::blue()),
            ],
        )])
        .line_series(vec![ChartSeries::new(
            "rate",
            vec![LineData::new("Jan", 10.0), LineData::new("Feb", 20.0)],
        )])
        .y_axis_left("volume-axis")
        .y_axis_right("rate-axis")
        .legend(LegendPosition::Bottom);
    let split_display = render_advanced_chart(&split_combo, frame);
    for evidence in ["volume", "rate", "volume-axis", "rate-axis"] {
        assert!(split_display.contains(evidence), "{split_display}");
    }

    let treemap = Treemap::new()
        .data(vec![TreemapNode::new("Root", 100.0).children(vec![
            TreemapNode::new("Large", 75.0),
            TreemapNode::new("Small", 25.0),
        ])])
        .gap(6.0)
        .label_visible(true);
    let treemap_display = render_advanced_chart(&treemap, frame);
    for evidence in ["Root", "Large", "Small"] {
        assert!(treemap_display.contains(evidence), "{treemap_display}");
    }
    assert!(paint_op_count(&treemap_display, "FillRect") >= 4);

    let gauge = |gauge_type| {
        Gauge::new()
            .value(75.0)
            .min(0.0)
            .max(100.0)
            .gauge_type(gauge_type)
            .pointer_width(4.0)
            .pointer_color(Color::red())
            .range_colors(vec![
                GaugeRange::new(0.0, 60.0, Color::green()),
                GaugeRange::new(60.0, 100.0, Color::red()),
            ])
            .format(|value| format!("{value:.1}%"))
    };
    let dashboard_chart = gauge(GaugeType::Dashboard);
    let full_chart = gauge(GaugeType::Full);
    let ring_chart = gauge(GaugeType::Ring);
    let dashboard = render_advanced_chart(&dashboard_chart, frame);
    let full = render_advanced_chart(&full_chart, frame);
    let ring = render_advanced_chart(&ring_chart, frame);
    assert!(dashboard.contains("75.0%"), "{dashboard}");
    assert_ne!(
        render_chart_pixels(&dashboard_chart, frame),
        render_chart_pixels(&full_chart, frame),
        "dashboard must use a half-circle sweep"
    );
    assert_ne!(
        render_chart_pixels(&full_chart, frame),
        render_chart_pixels(&ring_chart, frame),
        "ring must cut out the center"
    );
    assert!(paint_op_count(&ring, "FillCircle") >= 2);
    for display in [&dashboard, &full, &ring] {
        assert!(!display.contains("NaN"), "{display}");
    }
}

#[test]
fn advanced_chart_tooltip_consumes_trigger_template_and_custom_renderer() {
    let frame = Rect::new(0.0, 0.0, 200.0, 120.0);
    let mut heatmap = Heatmap::new()
        .x_labels(vec!["Mon", "Tue"])
        .y_labels(vec!["AM"])
        .data(vec![HeatmapCell::new(0, 0, 12.0)])
        .tooltip(
            TooltipConfig::new()
                .trigger(TooltipTrigger::Click)
                .template("{y}/{label}={value}"),
        );
    let _ = render_advanced_chart(&heatmap, frame);
    assert_eq!(
        heatmap.tooltip_text_for_test(Point::new(20.0, 20.0), frame),
        Some("/AM Mon=12".to_owned())
    );
    assert_eq!(
        EventHandler::on_event(
            &mut heatmap,
            &SystemEvent::PointerMove {
                pos: Point::new(20.0, 20.0),
                mods: KeyMod::NONE,
            },
        ),
        EventResult::NotHandled,
        "click-triggered tooltip must not consume hover"
    );
    assert_eq!(
        EventHandler::on_event(
            &mut heatmap,
            &SystemEvent::PointerDown {
                pos: Point::new(20.0, 20.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );
    assert_eq!(
        heatmap.tooltip_position_for_test(),
        Some(Point::new(20.0, 20.0))
    );
    let pinned_tooltip = render_advanced_chart(&heatmap, frame);
    assert!(pinned_tooltip.contains("/AM Mon=12"), "{pinned_tooltip}");
    assert_eq!(
        EventHandler::on_event(
            &mut heatmap,
            &SystemEvent::PointerDown {
                pos: Point::new(30.0, 20.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );
    assert_eq!(heatmap.tooltip_position_for_test(), None);

    let custom = ScatterChart::new()
        .data(vec![ScatterData::new("A", 1.5, 2.5)])
        .tooltip(TooltipConfig::new().render(|datum: &TooltipDatum| {
            format!(
                "{}@{},{}",
                datum.label,
                datum.x.unwrap_or_default(),
                datum.y.unwrap_or_default()
            )
        }));
    assert_eq!(
        custom.tooltip_text_for_test(Point::new(50.0, 50.0), frame),
        Some("A@1.5,2.5".to_owned())
    );

    let treemap = Treemap::new()
        .data(vec![
            TreemapNode::new("A", 25.0),
            TreemapNode::new("B", 75.0),
        ])
        .tooltip(TooltipConfig::default());
    assert_eq!(
        treemap.tooltip_text_for_test(Point::new(20.0, 50.0), frame),
        Some("A: 25 (25.0%)".to_owned())
    );
}

#[test]
fn advanced_chart_common_title_responsive_and_all_legend_positions_reserve_layout() {
    let frame = Rect::new(10.0, 20.0, 400.0, 260.0);
    let constraints = Constraints::loose(Size::new(640.0, 360.0));
    let responsive = AreaChart::new()
        .data(vec![LineData::new("A", 1.0), LineData::new("B", 2.0)])
        .responsive(true);
    assert_eq!(responsive.measure(constraints), Size::new(640.0, 360.0));

    for position in [
        LegendPosition::Top,
        LegendPosition::Bottom,
        LegendPosition::Left,
        LegendPosition::Right,
    ] {
        let chart = AreaChart::new()
            .series(vec![
                ChartSeries::new(
                    "alpha",
                    vec![LineData::new("A", 1.0), LineData::new("B", 2.0)],
                ),
                ChartSeries::new(
                    "beta",
                    vec![LineData::new("A", 2.0), LineData::new("B", 3.0)],
                ),
            ])
            .title("Overview")
            .subtitle("FY26")
            .padding(12.0)
            .legend(position);
        let (plot, legend) = chart
            .legend_layout_for_test(frame)
            .expect("named series should reserve legend space");
        match position {
            LegendPosition::Top => assert!(legend.y + legend.h <= plot.y + 0.01),
            LegendPosition::Bottom => assert!(legend.y + 0.01 >= plot.y + plot.h),
            LegendPosition::Left => assert!(legend.x + legend.w <= plot.x + 0.01),
            LegendPosition::Right => assert!(legend.x + 0.01 >= plot.x + plot.w),
            LegendPosition::None => unreachable!(),
        }
        let display = render_advanced_chart(&chart, frame);
        for evidence in ["Overview", "FY26", "alpha", "beta"] {
            assert!(
                display.contains(evidence),
                "{position:?}: missing {evidence}: {display}"
            );
        }
        assert!(!display.contains("NaN"), "{position:?}: {display}");
    }

    let no_legend = AreaChart::new()
        .data(vec![LineData::new("A", 1.0)])
        .legend(LegendPosition::None);
    assert_eq!(no_legend.legend_layout_for_test(frame), None);
}

#[test]
fn advanced_chart_interaction_and_reconcile_preserve_settled_viewport_state() {
    ADVANCED_CHART_CLICK_LABEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    *ADVANCED_CHART_BRUSH_RANGE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    let interaction = InteractionConfig {
        zoom: true,
        pan: true,
        crosshair: true,
        on_click: Some(record_advanced_chart_click),
    };
    let brush = BrushConfig {
        enabled: true,
        on_select: Some(record_advanced_chart_brush),
    };
    let frame = Rect::new(0.0, 0.0, 200.0, 120.0);
    let mut chart = ScatterChart::new()
        .data(vec![
            ScatterData::new("left", 1.0, 2.0),
            ScatterData::new("right", 3.0, 4.0),
        ])
        .interactive(interaction)
        .brush(brush);
    let _ = render_advanced_chart(&chart, frame);

    assert_eq!(
        chart.on_event(&SystemEvent::Wheel {
            pos: Point::new(50.0, 60.0),
            delta: Point::new(0.0, -1000.0),
        }),
        EventResult::Handled
    );
    assert_eq!(
        chart.on_event(&SystemEvent::PointerDown {
            pos: Point::new(40.0, 60.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        chart.on_event(&SystemEvent::PointerMove {
            pos: Point::new(140.0, 60.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let selecting_display = render_advanced_chart(&chart, frame);
    assert!(
        selecting_display.contains("a: 48"),
        "brush selection must paint a translucent overlay: {selecting_display}"
    );
    assert_eq!(
        chart.on_event(&SystemEvent::PointerUp {
            pos: Point::new(140.0, 60.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(chart.interaction_state_for_test(), (2.0, 100.0));
    assert_eq!(
        ADVANCED_CHART_CLICK_LABEL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_str(),
        "left"
    );
    assert_eq!(
        *ADVANCED_CHART_BRUSH_RANGE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        Some((0.2, 0.7))
    );

    chart.sync_from(
        ScatterChart::new()
            .data(vec![ScatterData::new("next", 5.0, 6.0)])
            .interactive(interaction)
            .brush(brush),
    );
    assert_eq!(chart.interaction_state_for_test(), (2.0, 100.0));

    chart.sync_from(ScatterChart::new().data(vec![ScatterData::new("plain", 1.0, 1.0)]));
    assert_eq!(chart.interaction_state_for_test(), (1.0, 0.0));

    let crosshair_interaction = InteractionConfig {
        zoom: false,
        pan: false,
        crosshair: true,
        on_click: None,
    };
    let mut crosshair = ScatterChart::new()
        .data(vec![ScatterData::new("point", 1.0, 1.0)])
        .interactive(crosshair_interaction);
    let _ = render_advanced_chart(&crosshair, frame);
    assert_eq!(
        crosshair.on_event(&SystemEvent::PointerMove {
            pos: Point::new(80.0, 50.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let plain = ScatterChart::new().data(vec![ScatterData::new("point", 1.0, 1.0)]);
    assert_ne!(
        render_chart_pixels(&crosshair, frame),
        render_chart_pixels(&plain, frame),
        "crosshair must change the rendered pixels"
    );
}

#[test]
fn advanced_chart_animation_advances_dirties_and_survives_reconcile() {
    let frame = Rect::new(0.0, 0.0, 200.0, 120.0);
    let animation = AnimationConfig::fade_in(0.2);
    let mut chart = AreaChart::new()
        .data(vec![LineData::new("A", 1.0), LineData::new("B", 2.0)])
        .fill_opacity(0.3)
        .animation(animation);
    assert_eq!(chart.animation_progress_for_test(), Some(0.0));
    assert!(WidgetAnimation::update_animation(&mut chart, 0.1));
    let midpoint = chart
        .animation_progress_for_test()
        .expect("animation player");
    assert!(midpoint > 0.0 && midpoint < 1.0);
    assert_eq!(WidgetAnimation::dirty_bounds(&chart, frame), frame);

    chart.sync_from(
        AreaChart::new()
            .data(vec![LineData::new("A", 2.0), LineData::new("B", 3.0)])
            .fill_opacity(0.3)
            .animation(animation),
    );
    assert_eq!(chart.animation_progress_for_test(), Some(midpoint));
    assert!(!WidgetAnimation::update_animation(&mut chart, 0.1));
    assert_eq!(chart.animation_progress_for_test(), Some(1.0));
    assert_eq!(WidgetAnimation::dirty_bounds(&chart, frame), frame);
    assert_eq!(WidgetAnimation::dirty_bounds(&chart, frame), Rect::zero());
}

#[test]
fn advanced_chart_empty_state_uses_config_provider_and_non_empty_keeps_chart() {
    let empty_tree = ViewAdapter::build(
        ConfigProvider::new()
            .render_empty(|context| label(format!("{} empty", context.component_name())))
            .child(|| Heatmap::new().data(Vec::<HeatmapCell>::new())),
    );
    assert!(empty_tree.find_by_type::<ChartPlaceholder>().is_none());
    assert_eq!(
        empty_tree
            .find_all_by_type::<crate::ui::Label>()
            .first()
            .map(|(_, label)| label.text()),
        Some("ChartPlaceholder empty")
    );

    let populated_tree = ViewAdapter::build(
        ConfigProvider::new()
            .render_empty(|_| label("unused"))
            .child(|| Heatmap::new().data(vec![HeatmapCell::new(0, 0, 1.0)])),
    );
    assert!(populated_tree.find_by_type::<ChartPlaceholder>().is_some());
}

#[test]
fn advanced_chart_normalizes_non_finite_configuration_and_empty_data() {
    let frame = Rect::new(0.0, 0.0, 160.0, 100.0);
    let empty = FunnelChart::new()
        .data(Vec::<FunnelData>::new())
        .gap(f32::NAN)
        .padding(f32::INFINITY);
    let empty_display = render_advanced_chart(&empty, frame);
    assert!(empty_display.contains("暂无数据"), "{empty_display}");
    assert!(!empty_display.contains("NaN"), "{empty_display}");

    let extreme_heatmap = Heatmap::new().show_values(true).data(vec![HeatmapCell::new(
        usize::MAX,
        usize::MAX,
        f32::NAN,
    )]);
    let extreme_display = render_advanced_chart(&extreme_heatmap, frame);
    assert!(extreme_display.contains("暂无数据"), "{extreme_display}");
    assert!(!extreme_display.contains("NaN"), "{extreme_display}");

    let bubble = ScatterChart::new().data(vec![crate::ui::widgets::BubbleData::new(
        "invalid radius",
        1.0,
        2.0,
        f32::NAN,
    )]);
    let bubble_display = render_advanced_chart(&bubble, frame);
    assert!(!bubble_display.contains("NaN"), "{bubble_display}");

    let mut interactive = Gauge::new()
        .value(f32::NAN)
        .range_colors(vec![
            GaugeRange::new(f32::NAN, 50.0, Color::red()),
            GaugeRange::new(50.0, f32::INFINITY, Color::green()),
        ])
        .interactive(InteractionConfig {
            zoom: true,
            pan: false,
            crosshair: false,
            on_click: None,
        });
    let _ = render_advanced_chart(&interactive, frame);
    assert_eq!(
        interactive.on_event(&SystemEvent::Wheel {
            pos: Point::new(50.0, 50.0),
            delta: Point::new(0.0, f32::NAN),
        }),
        EventResult::Handled
    );
    assert_eq!(interactive.interaction_state_for_test().0, 1.0);
    let gauge_display = render_advanced_chart(&interactive, frame);
    assert!(!gauge_display.contains("NaN"), "{gauge_display}");
}
