//! 组件展示页面（图表、其他）
//!
//! 使用 `prelude` 导入组件；自定义 widget 见同目录 `widgets` 模块。

use uix::prelude::*;
use uix::ui::core::widget::WidgetNode;

use super::widgets::{BounceBall, Counter, PulseRing};
use super::{row, PageBuilder, INNER_W};

// ═══════════════════════════════════════════════════════════════════════════
// Page 6: 图表 (Charts)
// ═══════════════════════════════════════════════════════════════════════════

pub fn page_charts(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("柱状图 — 月活跃用户")
        .push(
            BarChart::new()
                .width(INNER_W)
                .height(180.0)
                .show_value(true)
                .data(vec![
                    BarData::new("Jan", 420.0, tk.color_primary),
                    BarData::new("Feb", 380.0, tk.color_primary),
                    BarData::new("Mar", 530.0, tk.color_success),
                    BarData::new("Apr", 490.0, tk.color_primary),
                    BarData::new("May", 620.0, tk.color_success),
                    BarData::new("Jun", 580.0, tk.color_warning),
                    BarData::new("Jul", 710.0, tk.color_success),
                    BarData::new("Aug", 680.0, tk.color_primary),
                ]),
        )
        .section("折线图 — CPU 温度")
        .push(
            LineChart::new()
                .width(INNER_W)
                .height(160.0)
                .line_color(tk.color_error)
                .show_dots(true)
                .show_grid(true)
                .line_width(2.0)
                .data(vec![
                    LineData::new("00:00", 42.0),
                    LineData::new("01:00", 44.0),
                    LineData::new("02:00", 41.0),
                    LineData::new("03:00", 48.0),
                    LineData::new("04:00", 46.0),
                    LineData::new("05:00", 43.0),
                    LineData::new("06:00", 52.0),
                    LineData::new("07:00", 58.0),
                    LineData::new("08:00", 63.0),
                    LineData::new("09:00", 67.0),
                ]),
        )
        .section("饼图 — 浏览器市场份额")
        .push(
            tree! { Container::new().size(INNER_W, 220.0).dir(FlexDirection::Row) => [
                PieChart::new().size(180.0).data(vec![
                    PieData::new("Chrome", 65.0, tk.color_primary),
                    PieData::new("Firefox", 15.0, tk.color_success),
                    PieData::new("Safari", 10.0, tk.color_warning),
                    PieData::new("Edge", 8.0, tk.color_error),
                    PieData::new("Other", 2.0, tk.color_fill_tertiary),
                ]).into_node(),
                tree! { Container::new().size(20.0, 0.0) },
                PieChart::new().size(180.0).donut(0.45).data(vec![
                    PieData::new("Chrome", 65.0, tk.color_primary),
                    PieData::new("Firefox", 15.0, tk.color_success),
                    PieData::new("Safari", 10.0, tk.color_warning),
                    PieData::new("Edge", 8.0, tk.color_error),
                    PieData::new("Other", 2.0, tk.color_fill_tertiary),
                ]).into_node(),
            ]},
        )
        .build()
}

// ═══════════════════════════════════════════════════════════════════════════
// Page 7: 其他 (Other) — Custom widgets, Form, Colors, etc.
// ═══════════════════════════════════════════════════════════════════════════

pub fn page_other(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("自定义 Widget — Counter")
        .push(Counter { count: 0 })
        .section("自定义 Widget — PulseRing (动画)")
        .push(PulseRing { time: 0.0 })
        .section("自定义 Widget — BounceBall (动画)")
        .push(BounceBall { time: 0.0 })
        .section("表单 Form 示例")
        .push(
            tree! { Container::new().size(INNER_W, 100.0).dir(FlexDirection::Column)
                .pad(EdgeInsets::uniform(4.0)) => [
                Form::new().gap(4.0).into_node(),
            ]},
        )
        .section("颜色选择 ColorPicker")
        .push(row(36.0).child(ColorPicker::new(tk.color_primary)))
        .section("主题色 — 语义色")
        .push(
            row(44.0)
                .child(
                    Container::new()
                        .size(80.0, 32.0)
                        .bg(tk.color_primary_bg)
                        .rounded(tk.border_radius_sm),
                )
                .child(
                    Container::new()
                        .size(80.0, 32.0)
                        .bg(tk.color_success_bg)
                        .rounded(tk.border_radius_sm),
                )
                .child(
                    Container::new()
                        .size(80.0, 32.0)
                        .bg(tk.color_warning_bg)
                        .rounded(tk.border_radius_sm),
                )
                .child(
                    Container::new()
                        .size(80.0, 32.0)
                        .bg(tk.color_error_bg)
                        .rounded(tk.border_radius_sm),
                )
                .child(
                    Container::new()
                        .size(80.0, 32.0)
                        .bg(tk.color_info_bg)
                        .rounded(tk.border_radius_sm),
                ),
        )
        .push(
            row(28.0)
                .child(
                    Label::new("Primary")
                        .color(tk.color_primary)
                        .font_size(11.0),
                )
                .child(
                    Label::new("Success")
                        .color(tk.color_success)
                        .font_size(11.0),
                )
                .child(
                    Label::new("Warning")
                        .color(tk.color_warning)
                        .font_size(11.0),
                )
                .child(Label::new("Error").color(tk.color_error).font_size(11.0))
                .child(Label::new("Info").color(tk.color_info).font_size(11.0)),
        )
        .section("填充色层级")
        .push(
            row(28.0)
                .child(
                    Container::new()
                        .size(60.0, 20.0)
                        .bg(tk.color_fill)
                        .rounded(tk.border_radius_sm),
                )
                .child(
                    Container::new()
                        .size(60.0, 20.0)
                        .bg(tk.color_fill_secondary)
                        .rounded(tk.border_radius_sm),
                )
                .child(
                    Container::new()
                        .size(60.0, 20.0)
                        .bg(tk.color_fill_tertiary)
                        .rounded(tk.border_radius_sm),
                )
                .child(
                    Container::new()
                        .size(60.0, 20.0)
                        .bg(tk.color_fill_quaternary)
                        .rounded(tk.border_radius_sm),
                ),
        )
        .section("边框色")
        .push(
            row(28.0)
                .child(
                    Container::new()
                        .size(60.0, 20.0)
                        .bg(tk.color_border)
                        .rounded(tk.border_radius_sm),
                )
                .child(
                    Container::new()
                        .size(60.0, 20.0)
                        .bg(tk.color_border_secondary)
                        .rounded(tk.border_radius_sm),
                ),
        )
        .build()
}
