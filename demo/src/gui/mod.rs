//! UIX 多页 GUI 演示 — `cargo run --bin uix-demo`

use std::time::Duration;

use uix::core::log::info_fn;
use uix::prelude::*;

use crate::common::page::{page_heading, INIT_H, INIT_W, PAGE_TITLES, SIDEBAR_GROUPS, SIDEBAR_W};
use crate::demos::context::ThemeControl;
use crate::demos::{build_page, DemoCtx};
use std::cell::Cell;
use std::rc::Rc;

fn sidebar_group_label(tk: &DesignTokens, text: &str) -> ViewNode {
    label(text)
        .font_size(11.0)
        .fg(tk.color_text_quaternary)
        .margin(EdgeInsets::new(16.0, 14.0, 12.0, 6.0))
}

fn sidebar_brand(tk: &DesignTokens) -> ViewNode {
    row((
        Icon::new("box").size(22.0),
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
    icon: &str,
    title: &str,
) -> ViewNode {
    let page = active.clone();
    let item = NavItem::new(title.trim(), page_idx, shared.clone())
        .icon(icon)
        .width(SIDEBAR_W - 16.0)
        .height(36.0);
    item.margin(EdgeInsets::new(8.0, 0.0, 8.0, 0.0))
        .on_semantic(SemanticKind::Change, move |event| {
            if let Some(value) = event.text_payload() {
                if let Ok(index) = value.parse::<usize>() {
                    page.set(index);
                }
            }
        })
        .automation_id(format!("sidebar-page-{page_idx}"))
}

fn sidebar(active: State<usize>, tk: &DesignTokens) -> ViewNode {
    let shared: SharedActive = Rc::new(Cell::new(active.get()));
    let mut items: Vec<ViewNode> = vec![sidebar_brand(tk), Divider::new()];
    for (group_name, indices) in SIDEBAR_GROUPS {
        items.push(sidebar_group_label(tk, group_name));
        for &page_idx in *indices {
            let (icon, title) = PAGE_TITLES[page_idx];
            items.push(sidebar_nav_item(&active, &shared, page_idx, icon, title));
        }
    }
    items.push(label("").flex_grow(1.0));
    items.push(Divider::new());
    items.push(
        label("cargo run --bin uix-demo")
            .font_size(10.0)
            .fg(tk.color_text_quaternary)
            .padding(EdgeInsets::new(12.0, 16.0, 2.0, 8.0)),
    );
    items.push(
        label("UIX v0.1.0")
            .font_size(11.0)
            .fg(tk.color_text_quaternary)
            .padding(EdgeInsets::new(2.0, 16.0, 16.0, 8.0)),
    );
    column(items).width(SIDEBAR_W).bg(tk.color_bg_elevated)
}

fn header_bar(
    active: &State<usize>,
    timer_ticks: &State<u32>,
    theme_control: &ThemeControl,
    tk: &DesignTokens,
) -> ViewNode {
    let active_for_title = active.clone();
    let ticks = timer_ticks.clone();
    column((
        row((
            active_for_title
                .map_text(move |idx| {
                    let (_, title) = PAGE_TITLES[idx];
                    title.trim().to_string()
                })
                .font_size(16.0)
                .fg(tk.color_text),
            label("").flex_grow(1.0),
            Tag::new("live").color(TagColor::Success),
            ticks
                .map_text(|n| format!("{}s", n))
                .font_size(12.0)
                .fg(tk.color_text_tertiary),
            ThemeToggle::new()
                .dark(theme_control.is_dark())
                .on_click_fn({
                    let theme_control = theme_control.clone();
                    move || {
                        theme_control.toggle();
                    }
                })
                .automation_id("theme-toggle"),
        ))
        .align(AlignItems::Center)
        .gap(12.0)
        .height(48.0)
        .padding(EdgeInsets::new(20.0, 0.0, 16.0, 0.0))
        .bg(tk.color_bg_container),
        Divider::new(),
    ))
}

fn page_body(
    active: State<usize>,
    tk: &DesignTokens,
    timer_ticks: &State<u32>,
    anim_time: &State<f32>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
) -> ViewNode {
    column((
        header_bar(&active, timer_ticks, theme_control, tk),
        page_content(
            active,
            tk,
            timer_ticks,
            anim_time,
            home_count,
            runtime_count,
            theme_control,
        ),
    ))
    .flex_grow(1.0)
    .bg(tk.color_bg_layout)
}

fn page_shell(
    idx: usize,
    active: &State<usize>,
    tk: &DesignTokens,
    timer_ticks: &State<u32>,
    anim_time: &State<f32>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
) -> ViewNode {
    let (icon, title) = PAGE_TITLES[idx];
    let ctx = DemoCtx::new(tk, timer_ticks, anim_time, Some(active))
        .with_counters(home_count, runtime_count)
        .with_theme_control(theme_control);
    column((
        page_heading(tk, icon, title.trim()).key(format!("heading-{idx}")),
        build_page(idx, &ctx).key(format!("body-{idx}")),
    ))
    .key(format!("page-{idx}"))
    .flex_grow(1.0)
    .padding(EdgeInsets::new(8.0, 24.0, 16.0, 24.0))
}

fn page_content(
    active: State<usize>,
    tk: &DesignTokens,
    timer_ticks: &State<u32>,
    anim_time: &State<f32>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
) -> ViewNode {
    let idx = active.get();
    page_shell(
        idx,
        &active,
        tk,
        timer_ticks,
        anim_time,
        home_count,
        runtime_count,
        theme_control,
    )
}

fn app_shell_with_counters(
    active: State<usize>,
    timer_ticks: State<u32>,
    anim_time: State<f32>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
) -> ViewNode {
    let tk = DesignTokens::antd_light();
    column((
        row((
            sidebar(active.clone(), &tk),
            page_body(
                active,
                &tk,
                &timer_ticks,
                &anim_time,
                home_count,
                runtime_count,
                theme_control,
            ),
        ))
        .flex_grow(1.0),
        row((
            Icon::new("terminal").size(12.0),
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

#[cfg(test)]
fn app_shell(active: State<usize>, timer_ticks: State<u32>, anim_time: State<f32>) -> ViewNode {
    let home_count = State::new(0i32);
    let runtime_count = State::new(0i32);
    app_shell_with_counters(
        active,
        timer_ticks,
        anim_time,
        &home_count,
        &runtime_count,
        &ThemeControl::default(),
    )
}

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
        .on_start(with_cloned!(theme_control, timer_ticks, active; |handle| {
            theme_control.set_handle(handle.clone());
            let ticks = timer_ticks.clone();
            handle
                .run_interval(Duration::from_secs(1), move || {
                    ticks.update(|v| *v = v.wrapping_add(1));
                })
                .detach();
            if std::env::var_os("UIX_PERF_PROBE").is_some() {
                info_fn("PERF_SCENARIO=startup scheduled");
                let page = active.clone();
                handle
                    .run_after(Duration::from_millis(5000), {
                        let page = page.clone();
                        move || {
                            info_fn("PERF_SCENARIO=page_switch_general");
                            page.set(2);
                        }
                    })
                    .detach();
                handle
                    .run_after(Duration::from_millis(9000), {
                        let page = page.clone();
                        move || {
                            info_fn("PERF_SCENARIO=page_switch_input");
                            page.set(5);
                        }
                    })
                    .detach();
                handle
                    .run_after(Duration::from_millis(13000), {
                        let page = page.clone();
                        move || {
                            info_fn("PERF_SCENARIO=page_switch_home");
                            page.set(0);
                        }
                    })
                    .detach();
                handle
                    .run_after(Duration::from_millis(16000), || {
                        info_fn("PERF_SCENARIO=await_timer_tick");
                    })
                    .detach();
                handle
                    .run_after(Duration::from_millis(18500), || {
                        info_fn("PERF_SCENARIO=idle_window_expect_no_frame");
                    })
                    .detach();
                handle
                    .run_after(Duration::from_millis(20000), || {
                        info_fn("PERF_SCENARIO=done");
                        std::process::exit(0);
                    })
                    .detach();
            }
        }));
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

    app.root(with_cloned!(
        active,
        timer_ticks,
        anim_time,
        home_count,
        runtime_count,
        theme_control;
        {
            app_shell_with_counters(
                active,
                timer_ticks,
                anim_time,
                &home_count,
                &runtime_count,
                &theme_control,
            )
        }
    ))
    .run();
}

#[cfg(all(test, feature = "test-harness"))]
#[path = "../tests/gui.rs"]
mod tests;
