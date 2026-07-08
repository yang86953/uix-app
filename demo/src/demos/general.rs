//! 组件库页面 — page_general。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};

pub fn page_general(tk: &DesignTokens) -> ViewNode {
    PageBuilder::new(tk)
        .gap()
        // ── Typography ──
        .section("排版 — 标题层级")
        .push(tree! { Container::new().size(INNER_W, 160.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            Typography::heading("Heading 1 — 一级标题", 1).into_node(),
            Typography::heading("Heading 2 — 二级标题", 2).into_node(),
            Typography::heading("Heading 3 — 三级标题", 3).into_node(),
            Typography::paragraph("正文段落：UIX 是一个 Rust 原生 UI 框架，支持响应式布局、主题系统和丰富组件。").into_node(),
        ]})
        .section("文字颜色")
        .push(
            demo_row(28.0)
                .child(Label::new("Primary").color(tk.color_primary).font_size(14.0))
                .child(Label::new("Secondary").color(tk.color_text_secondary).font_size(14.0))
                .child(Label::new("Tertiary").color(tk.color_text_tertiary).font_size(14.0))
                .child(Label::new("Quaternary").color(tk.color_text_quaternary).font_size(14.0)),
        )
        // ── Button ──
        .section("按钮 — StyleSet 预设")
        .push(
            demo_row(40.0)
                .child(button("主要").primary().widget())
                .child(button("默认").widget())
                .child(button("幽灵").ghost().widget())
                .child(button("危险").danger().widget()),
        )
        .section("按钮组")
        .push(
            demo_row(40.0)
                .child(button("保存").primary().widget())
                .child(button("取消").widget())
                .child(button("删除").danger().widget()),
        )
        // ── Tag ──
        .section("标签 Tag")
        .push(
            demo_row(30.0)
                .child(Tag::new("Default"))
                .child(Tag::new("成功").color(TagColor::Success))
                .child(Tag::new("警告").color(TagColor::Warning))
                .child(Tag::new("错误").color(TagColor::Error))
                .child(Tag::new("进行中").color(TagColor::Info)),
        )
        // ── Icon ──
        .section("图标 — Lucide 16px")
        .push(
            Space::new().size(SpaceSize::Small).width(INNER_W).height(100.0)
                .direction(FlexDirection::Row).wrap(true).align(AlignItems::Start)
                .child(Icon::new("search").size(16.0))
                .child(Label::new(" search").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("home").size(16.0))
                .child(Label::new(" home").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("settings").size(16.0))
                .child(Label::new(" settings").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("user").size(16.0))
                .child(Label::new(" user").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("bell").size(16.0))
                .child(Label::new(" bell").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("star").size(16.0))
                .child(Label::new(" star").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("heart").size(16.0))
                .child(Label::new(" heart").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("edit").size(16.0))
                .child(Label::new(" edit").color(tk.color_text_secondary).font_size(11.0)),
        )
        .build()

}
