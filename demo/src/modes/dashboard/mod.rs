//! 组件库 GUI — `App` + `State` 分页 + View DSL 壳层。
//!
//! 运行：`cargo run --bin uix-demo -- --dashboard`（可选 `--gpu`）

use uix::core::log::info_fn;
use uix::prelude::*;

use crate::common::page::{page_heading, INIT_H, INIT_W, PAGE_TITLES, SIDEBAR_W};
use crate::demos::build_page;

fn sidebar_item(active: State<usize>, index: usize, label_text: &str) -> ViewNode {
    let active_set = active.clone();
    let text = label_text.to_string();
    button(text)
        .on_click(move || active_set.set(index))
        .width(SIDEBAR_W - 16.0)
        .padding((8.0, 4.0, 8.0, 4.0))
}

fn sidebar(active: State<usize>, tk: &DesignTokens) -> ViewNode {
    let mut items = vec![
        label("UIX 组件")
            .font_size(16.0)
            .padding((12.0, 16.0, 8.0, 8.0))
            .color(tk.color_text),
        embed(Divider::new()),
    ];
    for (i, (_, title)) in PAGE_TITLES.iter().enumerate() {
        items.push(sidebar_item(active.clone(), i, title.trim()));
    }
    items.push(label("").flex_grow(1.0));
    items.push(
        label("UIX v0.1.0")
            .font_size(11.0)
            .color(tk.color_text_quaternary)
            .padding((8.0, 8.0, 12.0, 8.0)),
    );
    column(items)
        .width(SIDEBAR_W)
        .bg(tk.color_bg_elevated)
        .flex_grow(1.0)
}

fn header_bar(tk: &DesignTokens) -> ViewNode {
    row([
        label("  UIX 组件库")
            .font_size(16.0)
            .color(tk.color_text)
            .width(400.0)
            .height(44.0),
        label("").flex_grow(1.0),
        label("UIX v0.1.0")
            .font_size(12.0)
            .color(tk.color_text_quaternary)
            .width(100.0)
            .height(44.0),
    ])
    .height(44.0)
    .bg(tk.color_bg_container)
}

fn content(active: State<usize>, tk: &DesignTokens) -> ViewNode {
    let idx = active.get();
    let (icon, title) = PAGE_TITLES[idx];
    column([
        header_bar(tk),
        column([
            page_heading(tk, icon, title.trim()),
            build_page(idx, tk),
        ])
        .flex_grow(1.0)
        .padding((0.0, 16.0, 0.0, 0.0)),
    ])
    .flex_grow(1.0)
    .bg(tk.color_bg_container)
}

fn dashboard_root(active: State<usize>) -> ViewNode {
    let tk = DesignTokens::antd_light();
    row([sidebar(active.clone(), &tk), content(active, &tk)])
        .flex_grow(1.0)
        .bg(tk.color_bg_layout)
}

pub fn run() {
    if std::env::args().any(|a| a == "--gpu") {
        info_fn("--gpu：App 启动时默认优先尝试 GPU 引擎，失败自动回退 CPU");
    }

    let active = State::new(0usize);
    let active_root = active.clone();

    App::new()
        .title("UIX — 组件库")
        .size(INIT_W, INIT_H)
        .theme(Theme::antd_light())
        .root(move || dashboard_root(active_root.clone()))
        .run();
}

#[cfg(test)]
#[path = "../../tests/demos/dashboard.rs"]
mod tests;
