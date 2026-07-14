//! 首页 — 轻量入门与快捷导航。
//! 使用 [`使用.md`](../../docs/使用.md) 风格。

use uix::prelude::*;

use crate::common::page::{PageBuilder, PAGE_APP, PAGE_COUNT, PAGE_GALLERY, PAGE_GENERAL};
use crate::common::showcase::{demo_card, panel};
use crate::demos::context::DemoCtx;

fn nav_tile(
    active: &State<usize>,
    index: usize,
    icon_name: &str,
    title: &str,
    desc: &str,
    tk: &DesignTokens,
) -> impl View {
    column((
        row((
            icon(icon_name),
            label(title).font_size(14.0).fg(tk.color_text),
        ))
        .gap(10.0),
        label(desc).font_size(12.0).fg(tk.color_text_tertiary),
    ))
    .width(200.0)
    .padding(16.0)
    .bg(tk.color_bg_container)
    .radius(tk.border_radius)
    .border(1.0, tk.color_border_secondary)
    .on_click(active, move |n| n.set(index))
}

pub fn page_home(ctx: &DemoCtx<'_>) -> impl View {
    let tk = ctx.tk;
    let active = ctx
        .active_page
        .expect("home page requires active_page for navigation");

    let count = ctx.home_count();

    PageBuilder::new(tk)
        .gap()
        .push_view(panel(
            tk,
            column((
                row((
                    icon("star"),
                    label("UIX 组件全景").font_size(16.0).fg(tk.color_text),
                    Tag::new("Demo").color(TagColor::Info),
                ))
                .gap(10.0),
                label("侧边栏浏览内置组件；CLI API：`cargo run --bin uix-demo -- --cli`")
                    .font_size(12.0)
                    .fg(tk.color_text_secondary),
            )),
        ))
        .block(
            "快速体验",
            row((
                demo_card(
                    tk,
                    "响应式 State",
                    300.0,
                    140.0,
                    column((
                        count
                            .map_text(|n| format!("计数: {n}"))
                            .font_size(28.0)
                            .fg(tk.color_primary)
                            .automation_id("home-count-value"),
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
                    column((
                        label(format!("{PAGE_COUNT}"))
                            .font_size(36.0)
                            .fg(tk.color_success),
                        label("个演示页").font_size(13.0).fg(tk.color_text),
                        row((
                            Tag::new("组件").color(TagColor::Success),
                            Tag::new("运行时").color(TagColor::Info),
                            Tag::new("主题").color(TagColor::Warning),
                        ))
                        .gap(8.0),
                    )),
                ),
            ))
            .gap(16.0),
        )
        .block(
            "快捷导航",
            row((
                nav_tile(
                    active,
                    PAGE_APP,
                    "cpu",
                    "应用能力",
                    "State · Timer · Theme · 多窗",
                    tk,
                ),
                nav_tile(
                    active,
                    PAGE_GENERAL,
                    "type",
                    "通用组件",
                    "Button · Tag · Icon",
                    tk,
                ),
                nav_tile(
                    active,
                    PAGE_GALLERY,
                    "list",
                    "覆盖清单",
                    "矩阵与 backlog",
                    tk,
                ),
            ))
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
