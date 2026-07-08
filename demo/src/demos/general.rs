//! 组件库页面 — page_general（通用 widgets 全覆盖）。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};
use crate::common::showcase::{labeled_row, widget_caption};
use crate::demos::context::DemoCtx;

pub fn page_general(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    PageBuilder::new(tk)
        .gap()
        .section("Typography — 标题层级")
        .push(tree! { Container::new().size(INNER_W, 160.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            Typography::heading("Heading 1 — 一级标题", 1).into_node(),
            Typography::heading("Heading 2 — 二级标题", 2).into_node(),
            Typography::heading("Heading 3 — 三级标题", 3).into_node(),
            Typography::paragraph("正文段落：UIX Rust 原生 UI — 响应式布局、主题、80+ 内置组件。").into_node(),
        ]})
        .section("Label — 文字颜色")
        .push(
            demo_row(28.0)
                .child(widget_caption(tk, "Primary"))
                .child(Label::new("Primary").color(tk.color_primary).font_size(14.0))
                .child(widget_caption(tk, "Secondary"))
                .child(Label::new("Secondary").color(tk.color_text_secondary).font_size(14.0))
                .child(widget_caption(tk, "Tertiary"))
                .child(Label::new("Tertiary").color(tk.color_text_tertiary).font_size(14.0)),
        )
        .section("Button — StyleSet 预设")
        .push(
            demo_row(40.0)
                .child(widget_caption(tk, "Primary"))
                .child(button("主要").primary().widget())
                .child(widget_caption(tk, "Default"))
                .child(button("默认").widget())
                .child(widget_caption(tk, "Ghost"))
                .child(button("幽灵").ghost().widget())
                .child(widget_caption(tk, "Danger"))
                .child(button("危险").danger().widget()),
        )
        .section("Button — 交互态 (hover/click 可见)")
        .push(
            demo_row(40.0)
                .child(button("保存").primary().widget())
                .child(button("取消").widget())
                .child(button("删除").danger().widget())
                .child(button("禁用").disabled(true).widget()),
        )
        .section("Tag — TagColor")
        .push(
            demo_row(30.0)
                .child(Tag::new("Default"))
                .child(Tag::new("Success").color(TagColor::Success))
                .child(Tag::new("Warning").color(TagColor::Warning))
                .child(Tag::new("Error").color(TagColor::Error))
                .child(Tag::new("Info").color(TagColor::Info)),
        )
        .section("Icon — Lucide")
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
        .section("Divider — 水平 / 垂直")
        .push(Divider::new().color(tk.color_border))
        .push(
            demo_row(36.0)
                .child(Label::new("左").color(tk.color_text).font_size(14.0))
                .child(Divider::new().vertical().color(tk.color_border))
                .child(Label::new("中").color(tk.color_text).font_size(14.0))
                .child(Divider::new().vertical().color(tk.color_border))
                .child(Label::new("右").color(tk.color_text).font_size(14.0)),
        )
        .section("Space — SpaceSize")
        .push({
            let mut sp = Space::new().size(SpaceSize::Small).width(INNER_W).height(0.0)
                .direction(FlexDirection::Column);
            for (sz, label_text, h) in [
                (SpaceSize::Small, "Small (8px)", 32.0),
                (SpaceSize::Middle, "Middle (16px)", 56.0),
                (SpaceSize::Large, "Large (24px)", 72.0),
            ] {
                sp = sp.child(
                    Space::new().size(sz).width(INNER_W).height(h)
                        .direction(FlexDirection::Row).align(AlignItems::Center)
                        .child(Label::new(label_text).color(tk.color_text_tertiary).font_size(11.0))
                        .child(button("甲").widget())
                        .child(button("乙").widget()),
                );
            }
            sp
        })
        .section("FloatButton / FloatButtonBackTop")
        .push(labeled_row(tk, 48.0, "FloatButton", FloatButton::new("+")))
        .push(labeled_row(
            tk,
            48.0,
            "FloatButtonBackTop",
            FloatButtonBackTop::new(),
        ))
        .build()
}
