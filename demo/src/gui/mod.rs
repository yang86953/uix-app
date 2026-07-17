//! UIX 多页 GUI 演示 — `cargo run --bin uix-demo`

use std::time::Duration;

use uix::core::log::info_fn;
use uix::prelude::*;

use crate::common::page::{page_heading, INIT_H, INIT_W, PAGE_TITLES, SIDEBAR_GROUPS, SIDEBAR_W};
use crate::demos::context::{FrameworkControl, GraphicsRecoveryControl, ThemeControl};
use crate::demos::{build_page, DemoCtx};
use std::cell::Cell;
use std::rc::Rc;

mod window_title_bar;

use window_title_bar::demo_window;

const SYSTEM_THEME_FOLLOW_STATUS_ID: &str = "system-theme-follow-status";

fn sidebar_group_label(_tk: &DesignTokens, text: &str) -> ViewNode {
    label(text)
        .font_size(11.0)
        .color(ColorValue::Neutral(NeutralRole::TextQuaternary))
        .margin(EdgeInsets::new(16.0, 14.0, 12.0, 6.0))
}

fn sidebar_brand(_tk: &DesignTokens) -> ViewNode {
    row([
        embed(Icon::new("box").size(22.0)),
        column_fit([
            label("UIX Demo")
                .font_size(17.0)
                .color(ColorValue::Neutral(NeutralRole::Text)),
            label("Component Showcase")
                .font_size(11.0)
                .color(ColorValue::Neutral(NeutralRole::TextTertiary)),
        ]),
    ])
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
    embed(item)
        .margin(EdgeInsets::new(8.0, 0.0, 8.0, 0.0))
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
    let mut nav_items = Vec::new();
    for (group_name, indices) in SIDEBAR_GROUPS {
        nav_items.push(sidebar_group_label(tk, group_name));
        for &page_idx in *indices {
            let (icon, title) = PAGE_TITLES[page_idx];
            nav_items.push(sidebar_nav_item(&active, &shared, page_idx, icon, title));
        }
    }
    let navigation = scroll(column_fit(nav_items).overflow_content())
        .vertical()
        .flex_grow(1.0)
        .build()
        .automation_id("sidebar-scroll");
    column([
        sidebar_brand(tk),
        embed(Divider::new()),
        navigation,
        embed(Divider::new()),
        label("cargo run --bin uix-demo")
            .font_size(10.0)
            .color(ColorValue::Neutral(NeutralRole::TextQuaternary))
            .padding(EdgeInsets::new(12.0, 16.0, 2.0, 8.0)),
        label("UIX v0.1.0")
            .font_size(11.0)
            .color(ColorValue::Neutral(NeutralRole::TextQuaternary))
            .padding(EdgeInsets::new(2.0, 16.0, 16.0, 8.0)),
    ])
    .width(SIDEBAR_W)
    .flex_grow(0.0)
    .flex_shrink(0.0)
    .bg(ColorValue::Neutral(NeutralRole::BgElevated))
}

fn header_bar(
    active: &State<usize>,
    timer_ticks: &State<u32>,
    theme_control: &ThemeControl,
) -> ViewNode {
    let active_for_title = active.clone();
    let ticks = timer_ticks.clone();
    let theme_control_view = if theme_control.follows_system_theme() {
        label("跟随系统")
            .automation_id(SYSTEM_THEME_FOLLOW_STATUS_ID)
            .font_size(12.0)
            .color(ColorValue::Palette(PaletteColor::Primary))
            .padding_h(8.0)
    } else {
        embed(ThemeToggle::new().dark(theme_control.is_dark()))
            .on_click_fn({
                let theme_control = theme_control.clone();
                move || {
                    theme_control.toggle();
                }
            })
            .automation_id("theme-toggle")
    };
    column_fit([
        row([
            dynamic_label(move || {
                let idx = active_for_title.get();
                let (_, title) = PAGE_TITLES[idx];
                title.trim().to_string()
            })
            .font_size(16.0)
            .color(ColorValue::Neutral(NeutralRole::Text)),
            label("").flex_grow(1.0),
            embed(Tag::new("live").color(TagColor::Success)),
            dynamic_label(move || format!("{}s", ticks.get()))
                .font_size(12.0)
                .color(ColorValue::Neutral(NeutralRole::TextTertiary)),
            theme_control_view,
        ])
        .align(AlignItems::Center)
        .gap(12.0)
        .height(48.0)
        .padding(EdgeInsets::new(20.0, 0.0, 16.0, 0.0))
        .bg(ColorValue::Neutral(NeutralRole::BgContainer)),
        embed(Divider::new()),
    ])
}

fn page_body(
    active: State<usize>,
    tk: &DesignTokens,
    timer_ticks: &State<u32>,
    component_case: &State<usize>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
    graphics_recovery_control: Option<&GraphicsRecoveryControl>,
    framework_control: &FrameworkControl,
) -> ViewNode {
    column([
        header_bar(&active, timer_ticks, theme_control),
        page_content(
            active,
            tk,
            timer_ticks,
            component_case,
            home_count,
            runtime_count,
            theme_control,
            graphics_recovery_control,
            framework_control,
        ),
    ])
    .flex_grow(1.0)
    .bg(ColorValue::Neutral(NeutralRole::BgLayout))
}

fn page_shell(
    idx: usize,
    active: &State<usize>,
    tk: &DesignTokens,
    timer_ticks: &State<u32>,
    component_case: &State<usize>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
    graphics_recovery_control: Option<&GraphicsRecoveryControl>,
    framework_control: &FrameworkControl,
) -> ViewNode {
    let (icon, title) = PAGE_TITLES[idx];
    let mut ctx = DemoCtx::new(tk, timer_ticks, Some(active))
        .with_component_case(component_case)
        .with_counters(home_count, runtime_count)
        .with_theme_control(theme_control)
        .with_framework_control(framework_control);
    if let Some(control) = graphics_recovery_control {
        ctx = ctx.with_graphics_recovery_control(control);
    }
    column([
        page_heading(icon, title.trim()).key(format!("heading-{idx}")),
        build_page(idx, &ctx)
            .key(format!("body-{idx}"))
            .automation_id(format!("page-scroll-{idx}")),
    ])
    .key(format!("page-{idx}"))
    .flex_grow(1.0)
    .padding(EdgeInsets::new(8.0, 24.0, 16.0, 24.0))
}

fn page_content(
    active: State<usize>,
    tk: &DesignTokens,
    timer_ticks: &State<u32>,
    component_case: &State<usize>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
    graphics_recovery_control: Option<&GraphicsRecoveryControl>,
    framework_control: &FrameworkControl,
) -> ViewNode {
    let idx = active.get();
    page_shell(
        idx,
        &active,
        tk,
        timer_ticks,
        component_case,
        home_count,
        runtime_count,
        theme_control,
        graphics_recovery_control,
        framework_control,
    )
}

fn app_shell_with_controls(
    active: State<usize>,
    timer_ticks: State<u32>,
    component_case: &State<usize>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
    graphics_recovery_control: Option<&GraphicsRecoveryControl>,
    framework_control: &FrameworkControl,
) -> ViewNode {
    let tk = DesignTokens::antd_light();
    column([
        row([
            sidebar(active.clone(), &tk),
            page_body(
                active,
                &tk,
                &timer_ticks,
                component_case,
                home_count,
                runtime_count,
                theme_control,
                graphics_recovery_control,
                framework_control,
            ),
        ])
        .flex_grow(1.0),
        row([
            embed(Icon::new("terminal").size(12.0)),
            space(6.0),
            label("UIX GUI 演示 — 侧边栏切换页面 · --cli 查看无窗口 API")
                .font_size(11.0)
                .color(ColorValue::Neutral(NeutralRole::TextTertiary)),
        ])
        .padding(EdgeInsets::new(6.0, 12.0, 6.0, 12.0))
        .bg(ColorValue::Neutral(NeutralRole::FillTertiary))
        .border(1.0, ColorValue::Neutral(NeutralRole::BorderSecondary))
        .automation_id("app-status-bar"),
    ])
    .flex_grow(1.0)
    .bg(ColorValue::Neutral(NeutralRole::BgLayout))
}

#[cfg(all(test, feature = "test-harness"))]
fn app_shell_with_counters(
    active: State<usize>,
    timer_ticks: State<u32>,
    component_case: &State<usize>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
    graphics_recovery_control: Option<&GraphicsRecoveryControl>,
) -> ViewNode {
    app_shell_with_controls(
        active,
        timer_ticks,
        component_case,
        home_count,
        runtime_count,
        theme_control,
        graphics_recovery_control,
        &FrameworkControl::default(),
    )
}

#[cfg(all(test, feature = "test-harness"))]
fn app_shell(active: State<usize>, timer_ticks: State<u32>) -> ViewNode {
    let home_count = State::new(0i32);
    let runtime_count = State::new(0i32);
    let component_case = State::new(0usize);
    app_shell_with_counters(
        active,
        timer_ticks,
        &component_case,
        &home_count,
        &runtime_count,
        &ThemeControl::default(),
        None,
    )
}

pub fn run(agent_control: bool, follow_system_theme: bool, graphics_recovery_acceptance: bool) {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let component_case = State::new(0usize);
    let home_count = State::new(0i32);
    let runtime_count = State::new(0i32);
    let theme_control = ThemeControl::new(follow_system_theme);
    let graphics_recovery_control = GraphicsRecoveryControl::new(graphics_recovery_acceptance);
    let framework_control = FrameworkControl::default();

    let app = App::new()
        .title("UIX Demo")
        .size(INIT_W, INIT_H)
        .custom_title_bar(true)
        .theme(Theme::antd_light())
        .follow_system_theme(follow_system_theme)
        .on_start(
            with_cloned!(theme_control, graphics_recovery_control, timer_ticks, active; |handle| {
                theme_control.set_handle(handle.clone());
                graphics_recovery_control.set_handle(handle.clone());
                let ticks = timer_ticks.clone();
                handle
                    .run_interval(Duration::from_secs(1), move || {
                        ticks.update(|v| *v = v.wrapping_add(1));
                    })
                    .detach();
                // 动画样例改走 WidgetAnimation（Spin 等）；不再全局 16ms 探活，
                // 否则 RegisteredActive 永不 DeepIdle，且曾把 orphan State 绑成整树 reconcile。
                if std::env::var_os("UIX_PERF_PROBE").is_some() {
                    info_fn("PERF_SCENARIO=startup scheduled");
                    let page = active.clone();
                    // Delays are wall-clock from on_start; first paint can take seconds,
                    // so keep later scenarios well after that cost settles.
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
            }),
        );
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
        component_case,
        home_count,
        runtime_count,
        theme_control,
        graphics_recovery_control,
        framework_control;
        {
            demo_window(app_shell_with_controls(
                active,
                timer_ticks,
                &component_case,
                &home_count,
                &runtime_count,
                &theme_control,
                graphics_recovery_control.enabled().then_some(&graphics_recovery_control),
                &framework_control,
            ))
        }
    ))
    .run();
}

#[cfg(all(test, feature = "test-harness"))]
#[path = "../tests/gui.rs"]
mod tests;
