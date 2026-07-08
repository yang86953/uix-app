//! 组件库页面 — page_other。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};
use crate::common::widgets::{BounceBall, Counter, PulseRing};

pub fn page_other(tk: &DesignTokens) -> ViewNode {
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
        .push(demo_row(36.0).child(ColorPicker::new(tk.color_primary)))
        .section("主题色 — 语义色")
        .push(
            demo_row(44.0)
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
            demo_row(28.0)
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
            demo_row(28.0)
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
            demo_row(28.0)
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
