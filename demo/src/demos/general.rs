//! 组件库页面 — page_general（通用 widgets 全覆盖）。
//! 对照 [`使用.md`](../../docs/使用.md#组件) 通用组件 + ConfigProvider。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder};
use crate::common::showcase::{flow_row, info_note, sample_block};
use crate::demos::context::DemoCtx;

pub fn page_general(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    PageBuilder::new(tk)
        .gap()
        .block(
            "ConfigProvider — 组件尺寸系统",
            column((
                info_note(
                    tk,
                    "ConfigProvider 为子树注入组件默认配置。Button/Input/Select/Table 等 10+ 组件支持 size(Small/Middle/Large)；未显式传 size 时继承 ConfigProvider。",
                ),
                row((
                    // Small 子树
                    ConfigProvider::new()
                        .component_size(Size::Small)
                        .child(|| {
                            column((
                                label("Size::Small")
                                    .fg(tk.color_text_secondary)
                                    .font_size(11.0),
                                button("小按钮").primary(),
                                Input::new("小型输入..."),
                            ))
                            .gap(8.0)
                            .padding(8.0)
                            .bg(tk.color_bg_container)
                            .radius(tk.border_radius)
                        }),
                    space(12.0),
                    // Middle 子树
                    ConfigProvider::new()
                        .component_size(Size::Middle)
                        .child(|| {
                            column((
                                label("Size::Middle（默认）")
                                    .fg(tk.color_text_secondary)
                                    .font_size(11.0),
                                button("中按钮").primary(),
                                Input::new("中型输入..."),
                            ))
                            .gap(8.0)
                            .padding(8.0)
                            .bg(tk.color_bg_container)
                            .radius(tk.border_radius)
                        }),
                    space(12.0),
                    // Large 子树
                    ConfigProvider::new()
                        .component_size(Size::Large)
                        .child(|| {
                            column((
                                label("Size::Large")
                                    .fg(tk.color_text_secondary)
                                    .font_size(11.0),
                                button("大按钮").primary(),
                                Input::new("大型输入..."),
                            ))
                            .gap(8.0)
                            .padding(8.0)
                            .bg(tk.color_bg_container)
                            .radius(tk.border_radius)
                        }),
                ))
                .gap(8.0),
            ))
            .gap(12.0),
        )
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
            flow_row(48.0)
                .child(sample_block(
                    tk,
                    "Primary",
                    label("Primary").fg(tk.color_primary).font_size(14.0),
                ))
                .child(sample_block(
                    tk,
                    "Secondary",
                    label("Secondary")
                        .fg(tk.color_text_secondary)
                        .font_size(14.0),
                ))
                .child(sample_block(
                    tk,
                    "Tertiary",
                    label("Tertiary")
                        .fg(tk.color_text_tertiary)
                        .font_size(14.0),
                )),
        )
        .block(
            "Button — StyleSet 预设",
            flow_row(64.0)
                .child(sample_block(
                    tk,
                    "Primary",
                    button("主要").primary(),
                ))
                .child(sample_block(tk, "Default", button("默认")))
                .child(sample_block(tk, "Ghost", button("幽灵").ghost()))
                .child(sample_block(tk, "Danger", button("危险").danger())),
        )
        .block(
            "Button — 交互态",
            flow_row(48.0)
                .child(button("保存").primary())
                .child(button("取消"))
                .child(button("删除").danger())
                .child(button("禁用").disabled(true)),
        )
        .block(
            "Tag / Badge",
            flow_row(40.0)
                .child(Tag::new("Default"))
                .child(Tag::new("Success").color(TagColor::Success))
                .child(Tag::new("Warning").color(TagColor::Warning))
                .child(Tag::new("Error").color(TagColor::Error))
                .child(Tag::new("Info").color(TagColor::Info))
                .child(Badge::new().count(5).color(tk.color_error))
                .child(Badge::new().text("New").color(tk.color_primary)),
        )
        .block(
            "Icon — Lucide",
            flow_row(56.0)
                .child(sample_block(tk, "search", icon("search").size(20.0)))
                .child(sample_block(tk, "home", icon("home").size(20.0)))
                .child(sample_block(
                    tk,
                    "settings",
                    icon("settings").size(20.0),
                ))
                .child(sample_block(tk, "user", icon("user").size(20.0)))
                .child(sample_block(tk, "bell", icon("bell").size(20.0)))
                .child(sample_block(tk, "star", icon("star").size(20.0)))
                .child(sample_block(tk, "heart", icon("heart").size(20.0)))
                .child(sample_block(tk, "edit", icon("edit").size(20.0))),
        )
        .block(
            "Divider — 水平 / 垂直",
            column((
                divider(),
                demo_row(36.0)
                    .child(
                        label("左")
                            .fg(tk.color_text)
                            .font_size(14.0),
                    )
                    .child(
                        divider().vertical(),
                    )
                    .child(
                        label("中")
                            .fg(tk.color_text)
                            .font_size(14.0),
                    )
                    .child(
                        divider().vertical(),
                    )
                    .child(
                        label("右")
                            .fg(tk.color_text)
                            .font_size(14.0),
                    ),
            ))
            .gap(12.0),
        )
        .block(
            "Space — SpaceSize",
            column((
                Space::new()
                    .size(SpaceSize::Small)
                    .direction(FlexDirection::Row)
                    .align(AlignItems::Center)
                    .child(
                        label("Small (8)")
                            .fg(tk.color_text_tertiary)
                            .font_size(11.0),
                    )
                    .child(button("甲"))
                    .child(button("乙")),
                Space::new()
                    .size(SpaceSize::Middle)
                    .direction(FlexDirection::Row)
                    .align(AlignItems::Center)
                    .child(
                        label("Middle (16)")
                            .fg(tk.color_text_tertiary)
                            .font_size(11.0),
                    )
                    .child(button("甲"))
                    .child(button("乙")),
                Space::new()
                    .size(SpaceSize::Large)
                    .direction(FlexDirection::Row)
                    .align(AlignItems::Center)
                    .child(
                        label("Large (24)")
                            .fg(tk.color_text_tertiary)
                            .font_size(11.0),
                    )
                    .child(button("甲"))
                    .child(button("乙")),
            ))
            .gap(12.0),
        )
        .block(
            "FloatButton",
            flow_row(56.0)
                .child(sample_block(tk, "FloatButton", FloatButton::new("+")))
                .child(sample_block(tk, "BackTop", FloatButtonBackTop::new())),
        )
        .build()
}
