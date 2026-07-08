//! UIX 多页 GUI 演示 — `cargo run --bin uix-demo`

use std::time::Duration;

use uix::prelude::*;

use crate::common::page::{page_heading, INIT_H, INIT_W, PAGE_TITLES, SIDEBAR_GROUPS, SIDEBAR_W};
use crate::demos::{build_page, DemoCtx};

fn sidebar_item(active: State<usize>, index: usize, label_text: &str) -> ViewNode {
    let active_set = active.clone();
    let text = label_text.to_string();
    button(text)
        .on_click(move || active_set.set(index))
        .width(SIDEBAR_W - 16.0)
        .padding((8.0, 4.0, 8.0, 4.0))
}

fn sidebar_group_label(tk: &DesignTokens, text: &str) -> ViewNode {
    label(text)
        .font_size(11.0)
        .color(tk.color_text_quaternary)
        .padding((16.0, 12.0, 4.0, 8.0))
}

fn sidebar(active: State<usize>, tk: &DesignTokens) -> ViewNode {
    let mut items = vec![
        label("UIX Demo")
            .font_size(16.0)
            .padding((12.0, 16.0, 8.0, 8.0))
            .color(tk.color_text),
        embed(Divider::new()),
    ];
    for (group_name, indices) in SIDEBAR_GROUPS {
        items.push(sidebar_group_label(tk, group_name));
        for &i in *indices {
            let (_, title) = PAGE_TITLES[i];
            items.push(sidebar_item(active.clone(), i, title.trim()));
        }
    }
    items.push(label("").flex_grow(1.0));
    items.push(
        label("cargo run --bin uix-demo")
            .font_size(10.0)
            .color(tk.color_text_quaternary)
            .padding((4.0, 8.0, 4.0, 8.0)),
    );
    items.push(
        label("UIX v0.1.0")
            .font_size(11.0)
            .color(tk.color_text_quaternary)
            .padding((4.0, 8.0, 12.0, 8.0)),
    );
    column(items)
        .width(SIDEBAR_W)
        .bg(tk.color_bg_elevated)
        .flex_grow(1.0)
}

fn header_bar(tk: &DesignTokens, timer_ticks: &State<u32>) -> ViewNode {
    let ticks = timer_ticks.clone();
    row([
        label("").flex_grow(1.0).height(44.0),
        dynamic_label(move || format!("timer: {}", ticks.get()))
            .font_size(11.0)
            .color(tk.color_text_tertiary)
            .width(80.0)
            .height(44.0),
        embed(ThemeToggle::new()),
    ])
    .height(44.0)
    .bg(tk.color_bg_container)
}

fn page_body(
    active: State<usize>,
    tk: &DesignTokens,
    timer_ticks: &State<u32>,
    anim_time: &State<f32>,
) -> ViewNode {
    column([
        header_bar(tk, timer_ticks),
        page_content(active, tk, timer_ticks, anim_time),
    ])
    .flex_grow(1.0)
    .bg(tk.color_bg_container)
}

fn page_content(
    active: State<usize>,
    tk: &DesignTokens,
    timer_ticks: &State<u32>,
    anim_time: &State<f32>,
) -> ViewNode {
    let idx = active.get();
    let (icon, title) = PAGE_TITLES[idx];
    let ctx = DemoCtx::new(tk, timer_ticks, anim_time, Some(&active));
    column([
        page_heading(tk, icon, title.trim()),
        build_page(idx, &ctx),
    ])
    .key(format!("page-{idx}"))
    .flex_grow(1.0)
    .padding((0.0, 16.0, 0.0, 0.0))
}

fn app_shell(
    active: State<usize>,
    timer_ticks: State<u32>,
    anim_time: State<f32>,
) -> ViewNode {
    let tk = DesignTokens::antd_light();
    column([
        row([
            sidebar(active.clone(), &tk),
            page_body(active, &tk, &timer_ticks, &anim_time),
        ])
        .flex_grow(1.0),
        embed(
            tree! { Footer::new(24.0).bg(tk.color_fill_tertiary) => [
                tree! { Container::new().pad(EdgeInsets::uniform(4.0)) => [
                    Label::new("UIX GUI 演示 — 侧边栏切换页面 · --cli 查看无窗口 API")
                        .color(tk.color_text_tertiary)
                        .font_size(11.0),
                ]},
            ]},
        ),
    ])
    .flex_grow(1.0)
    .bg(tk.color_bg_layout)
}

pub fn run() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);

    let active_root = active.clone();
    let timer_for_root = timer_ticks.clone();
    let anim_for_root = anim_time.clone();

    let timer_for_start = timer_ticks.clone();
    let anim_for_start = anim_time.clone();

    App::new()
        .title("UIX Demo")
        .size(INIT_W, INIT_H)
        .theme(Theme::antd_light())
        .on_start(move |handle| {
            let ticks = timer_for_start.clone();
            handle.run_interval(Duration::from_secs(1), move || {
                ticks.set(ticks.get().wrapping_add(1));
            });
            let anim = anim_for_start.clone();
            handle.run_interval(Duration::from_millis(16), move || {
                anim.set(anim.get() + 0.016);
            });
        })
        .root(move || {
            app_shell(
                active_root.clone(),
                timer_for_root.clone(),
                anim_for_root.clone(),
            )
        })
        .run();
}

#[cfg(test)]
#[path = "../tests/gui.rs"]
mod tests;
