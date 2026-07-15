//! 首页 — 轻量入门与快捷导航。

use uix::prelude::*;

use crate::common::page::{
    PageBuilder, PAGE_APP, PAGE_COUNT, PAGE_GALLERY, PAGE_GENERAL, PAGE_HOME,
};
use crate::common::showcase::{demo_card, panel};
use crate::demos::context::DemoCtx;

fn nav_tile(
    active: &State<usize>,
    index: usize,
    icon: &str,
    title: &str,
    desc: &str,
    tk: &DesignTokens,
) -> ViewNode {
    column_fit([
        row([
            embed(Icon::new(icon).size(20.0)),
            label(title)
                .font_size(14.0)
                .color(ColorValue::Neutral(NeutralRole::Text)),
        ])
        .align(AlignItems::Center)
        .gap(10.0),
        space(8.0),
        label(desc)
            .font_size(12.0)
            .color(ColorValue::Neutral(NeutralRole::TextTertiary)),
    ])
    .width(200.0)
    .padding(EdgeInsets::uniform(16.0))
    .bg(ColorValue::Neutral(NeutralRole::BgContainer))
    .radius(tk.border_radius)
    .border(1.0, ColorValue::Neutral(NeutralRole::BorderSecondary))
    .on_click(active, move |n| n.set(index))
}

pub fn page_home(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;
    let active = ctx
        .active_page
        .cloned()
        .unwrap_or_else(|| State::new(PAGE_HOME));

    let nav_app = active.clone();
    let nav_general = active.clone();
    let nav_gallery = active.clone();

    let count = ctx.home_count();

    PageBuilder::new(tk)
        .gap()
        .push_view(panel(
            tk,
            column_fit([
                row([
                    embed(Icon::new("star").size(18.0)),
                    label("UIX 组件全景")
                        .font_size(16.0)
                        .color(ColorValue::Neutral(NeutralRole::Text)),
                    embed(Tag::new("Demo").color(TagColor::Info)),
                ])
                .align(AlignItems::Center)
                .gap(10.0),
                space(8.0),
                label("侧边栏浏览内置组件；CLI API：`cargo run --bin uix-demo -- --cli`")
                    .font_size(12.0)
                    .color(ColorValue::Neutral(NeutralRole::TextSecondary)),
            ]),
        ))
        .block(
            "快速体验",
            row([
                demo_card(
                    tk,
                    "响应式 State",
                    300.0,
                    140.0,
                    column_fit((
                        count
                            .map_text(|n| format!("计数: {n}"))
                            .font_size(28.0)
                            .color(ColorValue::Palette(PaletteColor::Primary))
                            .automation_id("home-count-value"),
                        space(12.0),
                        row((
                            button("+1")
                                .primary()
                                .on_click(&count, |c| c.update(|v| *v += 1))
                                .automation_id("home-count-increment"),
                            button("-1")
                                .on_click(&count, |c| {
                                    c.update(|v| {
                                        if *v > 0 {
                                            *v -= 1;
                                        }
                                    });
                                })
                                .automation_id("home-count-decrement"),
                            button("重置")
                                .ghost()
                                .on_click(&count, |c| c.set(0))
                                .automation_id("home-count-reset"),
                        ))
                        .gap(8.0),
                    )),
                ),
                demo_card(
                    tk,
                    "覆盖范围",
                    220.0,
                    140.0,
                    column_fit([
                        label(format!("{PAGE_COUNT}"))
                            .font_size(36.0)
                            .color(ColorValue::Palette(PaletteColor::Success)),
                        space(4.0),
                        label("个演示页")
                            .font_size(13.0)
                            .color(ColorValue::Neutral(NeutralRole::Text)),
                        space(10.0),
                        embed(
                            // 卡片内容宽 ≈ 220−32；三 Tag+gap 超出时须换行，否则 Space
                            // flex_shrink=0 会把子项画到固定宽父容器外。
                            Space::new()
                                .size(SpaceSize::Small)
                                .direction(FlexDirection::Row)
                                .wrap(true)
                                .child(Tag::new("组件").color(TagColor::Success))
                                .child(Tag::new("运行时").color(TagColor::Info))
                                .child(Tag::new("主题").color(TagColor::Warning)),
                        ),
                    ]),
                ),
            ])
            .gap(16.0),
        )
        .block(
            "快捷导航",
            row([
                nav_tile(
                    &nav_app,
                    PAGE_APP,
                    "cpu",
                    "应用能力",
                    "State · Timer · Theme · 多窗",
                    tk,
                ),
                nav_tile(
                    &nav_general,
                    PAGE_GENERAL,
                    "type",
                    "通用组件",
                    "Button · Tag · Icon",
                    tk,
                ),
                nav_tile(
                    &nav_gallery,
                    PAGE_GALLERY,
                    "list",
                    "覆盖清单",
                    "矩阵与 backlog",
                    tk,
                ),
            ])
            .gap(12.0),
        )
        .build()
}

#[cfg(test)]
mod tests {
    use crate::common::page::{page_index_by_label, PAGE_TITLES};

    #[test]
    fn page_labels_resolve() {
        for (_, title) in PAGE_TITLES {
            let trimmed = title.trim();
            assert!(
                page_index_by_label(trimmed).is_some(),
                "missing index for {trimmed}"
            );
        }
    }
}
