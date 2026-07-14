//! 组件库页面 — page_general（通用 widgets 全覆盖）。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder};
use crate::demos::context::DemoCtx;

pub fn page_general(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    PageBuilder::new(tk)
        .gap()
        .block(
            "Typography — 标题层级",
            column((
                Typography::heading("Heading 1 — 一级标题", 1),
                Typography::heading("Heading 2 — 二级标题", 2),
                Typography::heading("Heading 3 — 三级标题", 3),
                Typography::paragraph(
                    "正文段落：UIX Rust 原生 UI — 响应式布局、主题、80+ 内置组件。",
                ),
            ))
            .gap(8.0),
        )
        .block(
            "Label — 文字颜色",
            row((
                column((
                    label("Primary").font_size(11.0).fg(tk.color_text_tertiary),
                    label("Primary").font_size(14.0).fg(tk.color_primary),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Secondary")
                        .font_size(11.0)
                        .fg(tk.color_text_tertiary),
                    label("Secondary")
                        .font_size(14.0)
                        .fg(tk.color_text_secondary),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Tertiary").font_size(11.0).fg(tk.color_text_tertiary),
                    label("Tertiary").font_size(14.0).fg(tk.color_text_tertiary),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
            ))
            .height(48.0)
            .wrap(true)
            .align(AlignItems::Start)
            .gap(24.0),
        )
        .block(
            "Button — StyleSet 预设",
            row((
                column((
                    label("Primary").font_size(11.0).fg(tk.color_text_tertiary),
                    button("主要").primary(),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Default").font_size(11.0).fg(tk.color_text_tertiary),
                    button("默认"),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Ghost").font_size(11.0).fg(tk.color_text_tertiary),
                    button("幽灵").ghost(),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Danger").font_size(11.0).fg(tk.color_text_tertiary),
                    button("危险").danger(),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
            ))
            .height(64.0)
            .wrap(true)
            .align(AlignItems::Start)
            .gap(24.0),
        )
        .block(
            "Button — 交互态",
            row((
                button("保存").primary(),
                button("取消"),
                button("删除").danger(),
                button("禁用").disabled(true),
            ))
            .height(48.0)
            .align(AlignItems::Center)
            .gap(8.0),
        )
        .block(
            "Tag / Badge",
            row((
                Tag::new("Default"),
                Tag::new("Success").color(TagColor::Success),
                Tag::new("Warning").color(TagColor::Warning),
                Tag::new("Error").color(TagColor::Error),
                Tag::new("Info").color(TagColor::Info),
                Badge::new().count(5).color(tk.color_error),
                Badge::new().text("New").color(tk.color_primary),
            ))
            .height(40.0)
            .wrap(true)
            .align(AlignItems::Center)
            .gap(8.0),
        )
        .block(
            "Icon — Lucide",
            row((
                column((
                    label("search").font_size(11.0).fg(tk.color_text_tertiary),
                    Icon::new("search").size(20.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("home").font_size(11.0).fg(tk.color_text_tertiary),
                    Icon::new("home").size(20.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("settings").font_size(11.0).fg(tk.color_text_tertiary),
                    Icon::new("settings").size(20.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("user").font_size(11.0).fg(tk.color_text_tertiary),
                    Icon::new("user").size(20.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("bell").font_size(11.0).fg(tk.color_text_tertiary),
                    Icon::new("bell").size(20.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("star").font_size(11.0).fg(tk.color_text_tertiary),
                    Icon::new("star").size(20.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("heart").font_size(11.0).fg(tk.color_text_tertiary),
                    Icon::new("heart").size(20.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("edit").font_size(11.0).fg(tk.color_text_tertiary),
                    Icon::new("edit").size(20.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
            ))
            .height(56.0)
            .wrap(true)
            .align(AlignItems::Start)
            .gap(16.0),
        )
        .block(
            "Divider — 水平 / 垂直",
            column((
                Divider::new().color(tk.color_border),
                row((
                    label("左").font_size(14.0).fg(tk.color_text),
                    Divider::new().vertical().color(tk.color_border),
                    label("中").font_size(14.0).fg(tk.color_text),
                    Divider::new().vertical().color(tk.color_border),
                    label("右").font_size(14.0).fg(tk.color_text),
                ))
                .height(36.0)
                .align(AlignItems::Center)
                .gap(8.0),
            ))
            .gap(12.0),
        )
        .block(
            "Space — Small / Middle / Large",
            column((
                row((
                    label("Small (8)")
                        .font_size(11.0)
                        .fg(tk.color_text_tertiary),
                    button("甲"),
                    button("乙"),
                ))
                .align(AlignItems::Center)
                .gap(8.0),
                row((
                    label("Middle (16)")
                        .font_size(11.0)
                        .fg(tk.color_text_tertiary),
                    button("甲"),
                    button("乙"),
                ))
                .align(AlignItems::Center)
                .gap(16.0),
                row((
                    label("Large (24)")
                        .font_size(11.0)
                        .fg(tk.color_text_tertiary),
                    button("甲"),
                    button("乙"),
                ))
                .align(AlignItems::Center)
                .gap(24.0),
            ))
            .gap(12.0),
        )
        .block(
            "FloatButton",
            row((
                column((
                    label("FloatButton")
                        .font_size(11.0)
                        .fg(tk.color_text_tertiary),
                    FloatButton::new("+"),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("BackTop").font_size(11.0).fg(tk.color_text_tertiary),
                    FloatButtonBackTop::new(),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
            ))
            .height(56.0)
            .wrap(true)
            .align(AlignItems::Start)
            .gap(24.0),
        )
        .build()
}
