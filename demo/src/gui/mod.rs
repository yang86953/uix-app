//! UIX 多页 GUI 演示 — 使用 [`使用.md`](../../docs/使用.md) 风格。

use std::time::Duration;

use uix::core::log::info_fn;
use uix::prelude::*;

use crate::common::page::{page_heading, INIT_H, INIT_W, PAGE_TITLES, SIDEBAR_GROUPS, SIDEBAR_W};
use crate::demos::context::ThemeControl;
use crate::demos::{build_page, DemoCtx};
use std::cell::Cell;
use std::rc::Rc;

// ── 侧边栏 ──

fn sidebar_group_label(tk: &DesignTokens, text: &str) -> impl View {
    label(text)
        .font_size(11.0)
        .fg(tk.color_text_quaternary)
        .padding(EdgeInsets::new(16.0, 14.0, 12.0, 6.0))
}

fn sidebar_brand(tk: &DesignTokens) -> impl View {
    row((
        icon("box").size(22.0),
        column((
            label("UIX Demo").font_size(17.0).fg(tk.color_text),
            label("Component Showcase")
                .font_size(11.0)
                .fg(tk.color_text_tertiary),
        )),
    ))
    .align(AlignItems::Center)
    .gap(10.0)
    .padding(EdgeInsets::new(16.0, 20.0, 12.0, 16.0))
}

fn sidebar_nav_item(
    active: &State<usize>,
    shared: &SharedActive,
    page_idx: usize,
    icon_str: &str,
    title: &str,
) -> impl View {
    let page = active.clone();
    NavItem::new(title.trim(), page_idx, shared.clone())
        .icon(icon_str)
        .width(SIDEBAR_W - 16.0)
        .height(36.0)
        .on_semantic(SemanticKind::Change, move |event| {
            if let Some(value) = event.text_payload() {
                if let Ok(index) = value.parse::<usize>() {
                    page.set(index);
                }
            }
        })
        .automation_id(format!("sidebar-page-{page_idx}"))
}

fn sidebar(active: State<usize>, tk: &DesignTokens) -> impl View {
    let shared: SharedActive = Rc::new(Cell::new(active.get()));
    let mut items: Vec<ViewNode> = vec![];
    items.push(sidebar_brand(tk).into_node());
    items.push(divider().into_node());
    for (group_name, indices) in SIDEBAR_GROUPS {
        items.push(sidebar_group_label(tk, group_name).into_node());
        for &page_idx in *indices {
            let (icon_str, title) = PAGE_TITLES[page_idx];
            items.push(sidebar_nav_item(&active, &shared, page_idx, icon_str, title).into_node());
        }
    }
    items.push(label("").flex_grow(1.0).into_node());
    items.push(divider().into_node());
    items.push(
        label("cargo run --bin uix-demo")
            .font_size(10.0)
            .fg(tk.color_text_quaternary)
            .padding(EdgeInsets::new(12.0, 16.0, 2.0, 8.0))
            .into_node(),
    );
    items.push(
        label("UIX v0.1.0")
            .font_size(11.0)
            .fg(tk.color_text_quaternary)
            .padding(EdgeInsets::new(2.0, 16.0, 16.0, 8.0))
            .into_node(),
    );
    column(items).width(SIDEBAR_W).bg(tk.color_bg_elevated)
}

// ── 头部 ──

fn header_bar(
    active: &State<usize>,
    timer_ticks: &State<u32>,
    theme_control: &ThemeControl,
    tk: &DesignTokens,
) -> impl View {
    let active_for_title = active.clone();
    let ticks = timer_ticks.clone();
    column((
        row((
            label({
                let idx = active_for_title.get();
                let (_, title) = PAGE_TITLES[idx];
                title.trim().to_string()
            })
            .font_size(16.0)
            .fg(tk.color_text),
            label("").flex_grow(1.0),
            Tag::new("live").color(TagColor::Success),
            label(format!("{}s", ticks.get()))
                .font_size(12.0)
                .fg(tk.color_text_tertiary),
            ThemeToggle::new()
                .dark(theme_control.is_dark())
                .on_click_fn({
                    let theme_control = theme_control.clone();
                    move || theme_control.toggle()
                })
                .automation_id("theme-toggle"),
        ))
        .align(AlignItems::Center)
        .gap(12.0)
        .height(48.0)
        .padding(EdgeInsets::new(20.0, 0.0, 16.0, 0.0))
        .bg(tk.color_bg_container),
        divider(),
    ))
}

// ── 页面主体 ──

fn page_body(
    active: State<usize>,
    timer_ticks: &State<u32>,
    anim_time: &State<f32>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
    tk: &DesignTokens,
) -> impl View {
    column((header_bar(&active, timer_ticks, theme_control, tk), {
        let idx = active.get();
        let (icon_str, title) = PAGE_TITLES[idx];
        let ctx = DemoCtx::new(tk, timer_ticks, anim_time, Some(&active))
            .with_counters(home_count, runtime_count)
            .with_theme_control(theme_control);
        column((
            page_heading(tk, icon_str, title.trim()),
            build_page(idx, &ctx),
        ))
        .flex_grow(1.0)
        .padding(EdgeInsets::new(8.0, 24.0, 16.0, 24.0))
    }))
    .flex_grow(1.0)
    .bg(tk.color_bg_layout)
}

fn app_shell_with_counters(
    active: State<usize>,
    timer_ticks: State<u32>,
    anim_time: State<f32>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
) -> impl View {
    let tk = DesignTokens::antd_light();
    column((
        row((
            sidebar(active.clone(), &tk),
            page_body(
                active,
                &timer_ticks,
                &anim_time,
                home_count,
                runtime_count,
                theme_control,
                &tk,
            ),
        ))
        .flex_grow(1.0),
        row((
            icon("terminal").size(12.0),
            space(6.0),
            label("UIX GUI 演示 — 侧边栏切换页面 · --cli 查看无窗口 API")
                .font_size(11.0)
                .fg(tk.color_text_tertiary),
        ))
        .padding(EdgeInsets::new(6.0, 12.0, 6.0, 12.0))
        .bg(tk.color_fill_tertiary)
        .border(1.0, tk.color_border_secondary),
    ))
    .flex_grow(1.0)
    .bg(tk.color_bg_layout)
}

// ── 启动 ──

pub fn run(agent_control: bool) {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);
    let home_count = State::new(0i32);
    let runtime_count = State::new(0i32);
    let theme_control = ThemeControl::default();

    let app = App::new()
        .title("UIX Demo")
        .size(INIT_W, INIT_H)
        .theme(Theme::antd_light())
        .on_start(|handle| {
            theme_control.set_handle(handle.clone());
            let ticks = timer_ticks.clone();
            handle
                .run_interval(Duration::from_secs(1), move || {
                    ticks.update(|v| *v = v.wrapping_add(1));
                })
                .detach();
        });

    #[cfg(feature = "agent-control")]
    let app = if agent_control {
        app.enable_agent_control()
    } else {
        app
    };
    #[cfg(not(feature = "agent-control"))]
    let app = {
        debug_assert!(!agent_control);
        app
    };

    app.root(move || {
        app_shell_with_counters(
            active.clone(),
            timer_ticks.clone(),
            anim_time.clone(),
            &home_count,
            &runtime_count,
            &theme_control,
        )
    })
    .run();
}

#[cfg(all(test, feature = "test-harness"))]
#[path = "../tests/gui.rs"]
mod tests;
