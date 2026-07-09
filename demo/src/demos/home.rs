//! 首页 — 轻量入门与快捷导航。

use uix::prelude::*;

use crate::common::page::{PageBuilder, PAGE_APP, PAGE_COUNT, PAGE_GALLERY, PAGE_GENERAL};
use crate::common::showcase::{demo_card, info_note};
use crate::demos::context::DemoCtx;

fn nav_tile(
    active: &State<usize>,
    index: usize,
    icon: &str,
    title: &str,
    desc: &str,
    tk: &DesignTokens,
) -> ViewNode {
    let nav = active.clone();
    row([
        embed(Icon::new(icon).size(22.0)),
        space(12.0),
        column([
            label(title)
                .font_size(14.0)
                .color(tk.color_text),
            space(4.0),
            label(desc)
                .font_size(11.0)
                .color(tk.color_text_tertiary),
        ])
        .flex_grow(0.0),
    ])
    .width(210.0)
    .padding(EdgeInsets::uniform(14.0))
    .bg(tk.color_bg_elevated)
    .radius(tk.border_radius_lg)
    .border(1.0, tk.color_border_secondary)
    .on_semantic(SemanticKind::Click, move |_| nav.set(index))
}

pub fn page_home(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;
    let active = ctx
        .active_page
        .expect("home page requires active_page for navigation");

    let nav_app = active.clone();
    let nav_general = active.clone();
    let nav_gallery = active.clone();

    let count = State::new(0i32);
    let display = count.clone();
    let inc = count.clone();
    let dec = count.clone();

    PageBuilder::new(tk)
        .gap()
        .push(info_note(
            tk,
            "UIX 官方 GUI Demo — 侧边栏浏览组件；CLI API 见 `cargo run --bin uix-demo -- --cli`。",
        ))
        .section("快速体验")
        .push(
            row([
                demo_card(
                    tk,
                    "响应式 State",
                    280.0,
                    120.0,
                    column([
                        dynamic_label(move || format!("计数: {}", display.get()))
                            .font_size(28.0)
                            .color(tk.color_primary),
                        space(12.0),
                        row([
                            button("+1")
                                .primary()
                                .on_click(move || inc.set(inc.get() + 1)),
                            button("-1").on_click(move || {
                                let v = dec.get();
                                if v > 0 {
                                    dec.set(v - 1);
                                }
                            }),
                        ])
                        .gap(8.0),
                    ])
                    .flex_grow(0.0),
                ),
                demo_card(
                    tk,
                    "组件覆盖",
                    200.0,
                    120.0,
                    column([
                        label(format!("{PAGE_COUNT} 个演示页"))
                            .font_size(28.0)
                            .color(tk.color_success),
                        space(4.0),
                        label("8 类组件 + 参考")
                            .font_size(11.0)
                            .color(tk.color_text_tertiary),
                    ])
                    .flex_grow(0.0),
                ),
            ])
            .gap(12.0),
        )
        .section("快捷导航")
        .push(
            row([
                nav_tile(
                    &nav_app,
                    PAGE_APP,
                    "cpu",
                    "应用能力",
                    "State · Timer · View DSL",
                    tk,
                ),
                nav_tile(
                    &nav_general,
                    PAGE_GENERAL,
                    "type",
                    "通用组件",
                    "Button · Label · Tag",
                    tk,
                ),
                nav_tile(
                    &nav_gallery,
                    PAGE_GALLERY,
                    "list",
                    "覆盖清单",
                    "组件矩阵与 backlog",
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
