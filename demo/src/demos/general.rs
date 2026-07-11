//! 组件库页面 — page_general（通用 widgets 全覆盖）。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder};
use crate::common::showcase::{flow_row, sample_block};
use crate::demos::context::DemoCtx;

pub fn page_general(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    PageBuilder::new(tk)
        .gap()
        .block(
            "Typography — 标题层级",
            column_fit([
                embed(Typography::heading("Heading 1 — 一级标题", 1)),
                embed(Typography::heading("Heading 2 — 二级标题", 2)),
                embed(Typography::heading("Heading 3 — 三级标题", 3)),
                embed(Typography::paragraph(
                    "正文段落：UIX Rust 原生 UI — 响应式布局、主题、80+ 内置组件。",
                )),
            ])
            .gap(8.0),
        )
        .block(
            "Label — 文字颜色",
            embed(
                flow_row(48.0)
                    .child(sample_block(
                        tk,
                        "Primary",
                        Label::new("Primary").color(tk.color_primary).font_size(14.0),
                    ))
                    .child(sample_block(
                        tk,
                        "Secondary",
                        Label::new("Secondary")
                            .color(tk.color_text_secondary)
                            .font_size(14.0),
                    ))
                    .child(sample_block(
                        tk,
                        "Tertiary",
                        Label::new("Tertiary")
                            .color(tk.color_text_tertiary)
                            .font_size(14.0),
                    )),
            ),
        )
        .block(
            "Button — StyleSet 预设",
            embed(
                flow_row(64.0)
                    .child(sample_block(tk, "Primary", button("主要").primary().widget()))
                    .child(sample_block(tk, "Default", button("默认").widget()))
                    .child(sample_block(tk, "Ghost", button("幽灵").ghost().widget()))
                    .child(sample_block(tk, "Danger", button("危险").danger().widget())),
            ),
        )
        .block(
            "Button — 交互态",
            embed(
                flow_row(48.0)
                    .child(button("保存").primary().widget())
                    .child(button("取消").widget())
                    .child(button("删除").danger().widget())
                    .child(button("禁用").disabled(true).widget()),
            ),
        )
        .block(
            "Tag / Badge",
            embed(
                flow_row(40.0)
                    .child(Tag::new("Default"))
                    .child(Tag::new("Success").color(TagColor::Success))
                    .child(Tag::new("Warning").color(TagColor::Warning))
                    .child(Tag::new("Error").color(TagColor::Error))
                    .child(Tag::new("Info").color(TagColor::Info))
                    .child(Badge::new().count(5).color(tk.color_error))
                    .child(Badge::new().text("New").color(tk.color_primary)),
            ),
        )
        .block(
            "Icon — Lucide",
            embed(
                flow_row(56.0)
                    .child(sample_block(tk, "search", Icon::new("search").size(20.0)))
                    .child(sample_block(tk, "home", Icon::new("home").size(20.0)))
                    .child(sample_block(tk, "settings", Icon::new("settings").size(20.0)))
                    .child(sample_block(tk, "user", Icon::new("user").size(20.0)))
                    .child(sample_block(tk, "bell", Icon::new("bell").size(20.0)))
                    .child(sample_block(tk, "star", Icon::new("star").size(20.0)))
                    .child(sample_block(tk, "heart", Icon::new("heart").size(20.0)))
                    .child(sample_block(tk, "edit", Icon::new("edit").size(20.0))),
            ),
        )
        .block(
            "Divider — 水平 / 垂直",
            column_fit([
                embed(Divider::new().color(tk.color_border)),
                embed(
                    demo_row(36.0)
                        .child(Label::new("左").color(tk.color_text).font_size(14.0))
                        .child(Divider::new().vertical().color(tk.color_border))
                        .child(Label::new("中").color(tk.color_text).font_size(14.0))
                        .child(Divider::new().vertical().color(tk.color_border))
                        .child(Label::new("右").color(tk.color_text).font_size(14.0)),
                ),
            ])
            .gap(12.0),
        )
        .block(
            "Space — SpaceSize",
            column_fit([
                embed(
                    Space::new()
                        .size(SpaceSize::Small)
                        .direction(FlexDirection::Row)
                        .align(AlignItems::Center)
                        .child(
                            Label::new("Small (8)")
                                .color(tk.color_text_tertiary)
                                .font_size(11.0),
                        )
                        .child(button("甲").widget())
                        .child(button("乙").widget()),
                ),
                embed(
                    Space::new()
                        .size(SpaceSize::Middle)
                        .direction(FlexDirection::Row)
                        .align(AlignItems::Center)
                        .child(
                            Label::new("Middle (16)")
                                .color(tk.color_text_tertiary)
                                .font_size(11.0),
                        )
                        .child(button("甲").widget())
                        .child(button("乙").widget()),
                ),
                embed(
                    Space::new()
                        .size(SpaceSize::Large)
                        .direction(FlexDirection::Row)
                        .align(AlignItems::Center)
                        .child(
                            Label::new("Large (24)")
                                .color(tk.color_text_tertiary)
                                .font_size(11.0),
                        )
                        .child(button("甲").widget())
                        .child(button("乙").widget()),
                ),
            ])
            .gap(12.0),
        )
        .block(
            "FloatButton",
            embed(
                flow_row(56.0)
                    .child(sample_block(tk, "FloatButton", FloatButton::new("+")))
                    .child(sample_block(
                        tk,
                        "BackTop",
                        FloatButtonBackTop::new(),
                    )),
            ),
        )
        .build()
}
