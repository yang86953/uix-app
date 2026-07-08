//! 首页 — 轻量入门与快捷导航。

use uix::prelude::*;

use crate::common::page::{PageBuilder, PAGE_APP, PAGE_GALLERY, PAGE_GENERAL};
use crate::common::showcase::info_note;
use crate::demos::context::DemoCtx;

fn nav_button(active: &State<usize>, index: usize, label_text: &str) -> ViewNode {
    let nav = active.clone();
    button(label_text.to_string())
        .on_click(move || nav.set(index))
        .padding((6.0, 10.0, 6.0, 10.0))
}

pub fn page_home(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;
    let active = ctx
        .active_page
        .expect("home page requires active_page for navigation");

    let nav_app = active.clone();
    let nav_general = active.clone();
    let nav_gallery = active.clone();

    PageBuilder::new(tk)
        .gap()
        .section("入门")
        .push(info_note(
            tk,
            "UIX 官方 GUI Demo — 侧边栏浏览组件；CLI API 见 `cargo run --bin uix-demo -- --cli`。",
        ))
        .push({
            let count = State::new(0i32);
            let display = count.clone();
            let inc = count.clone();
            column([
                dynamic_label(move || format!("计数: {}", display.get()))
                    .font_size(20.0)
                    .color(tk.color_primary),
                row([button("+1")
                    .primary()
                    .on_click(move || inc.set(inc.get() + 1))])
                .gap(8.0),
            ])
            .gap(8.0)
        })
        .section("快捷导航")
        .push(
            row([
                nav_button(&nav_app, PAGE_APP, "应用能力"),
                nav_button(&nav_general, PAGE_GENERAL, "通用组件"),
                nav_button(&nav_gallery, PAGE_GALLERY, "覆盖清单"),
            ])
            .gap(8.0),
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
