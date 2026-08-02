//! UIX 多页 GUI 演示 — `cargo run --bin uix-demo`

use std::time::{Duration, Instant};

use uix::prelude::*;

use crate::common::page::{
    page_heading, INIT_H, INIT_W, PAGE_CHARTS, PAGE_COMPONENT_QA, PAGE_FEEDBACK, PAGE_GENERAL,
    PAGE_HOME, PAGE_TITLES, SIDEBAR_GROUPS, SIDEBAR_W,
};
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
        label("UIX v0.0.1")
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

#[allow(
    clippy::too_many_arguments,
    reason = "demo page assembly keeps independent shared controls explicit"
)]
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

#[allow(
    clippy::too_many_arguments,
    reason = "demo page assembly keeps independent shared controls explicit"
)]
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

#[allow(
    clippy::too_many_arguments,
    reason = "demo page assembly keeps independent shared controls explicit"
)]
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

#[allow(
    clippy::too_many_arguments,
    reason = "demo page assembly keeps independent shared controls explicit"
)]
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

fn g5_overlay_layer(mode: &State<u8>) -> ViewNode {
    let mode = mode.get();
    row([
        embed(
            Modal::new("G5 Modal lifecycle")
                .visible(mode == 1)
                .overlay(true)
                .closable(false)
                .mask_closable(false)
                .footer_visible(false),
        )
        .key("g5-modal-node")
        .width(1.0)
        .height(1.0)
        .automation_id("g5-modal"),
        embed(
            Drawer::new("G5 Drawer lifecycle")
                .visible(mode == 2)
                .closable(false)
                .mask_closable(false)
                .placement(DrawerPlacement::Right),
        )
        .key("g5-drawer-node")
        .width(1.0)
        .height(1.0)
        .automation_id("g5-drawer"),
    ])
    .height(1.0)
    .automation_id("g5-overlay-lifecycle")
}

fn g5_release_root(shell: ViewNode, overlay_mode: &State<u8>) -> ViewNode {
    column([shell.flex_grow(1.0), g5_overlay_layer(overlay_mode)])
        .flex_grow(1.0)
        .bg(ColorValue::Neutral(NeutralRole::BgLayout))
}

fn log_g5_scenario(
    name: &str,
    category: &str,
    iteration: u64,
    page: usize,
    theme: &str,
    overlay: &str,
) {
    uix::core::perf_probe::set_internal_g5_scenario(&format!("{name}.{iteration}"));
    tracing::info!(
        "G5_SCENARIO schema=1 name={name} category={category} iteration={iteration} page={page} theme={theme} overlay={overlay}"
    );
}

const G5_MACRO_CYCLE_SECONDS: u64 = 10 * 60;
const G5_IDLE_SECONDS: u64 = 60;
const G5_WARMUP_SECONDS: u64 = 30;

struct G5ScenarioState {
    started: Instant,
    macro_cycle: u64,
    idle: bool,
    pulse: u64,
    phase: u8,
    theme_iteration: u64,
    modal_iteration: u64,
    drawer_iteration: u64,
}

impl G5ScenarioState {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            macro_cycle: u64::MAX,
            idle: false,
            pulse: 0,
            phase: 0,
            theme_iteration: 0,
            modal_iteration: 0,
            drawer_iteration: 0,
        }
    }
}

fn log_g5_phase(macro_cycle: u64, phase: &str, elapsed: Duration) {
    tracing::info!(
        "G5_PHASE schema=1 macro_cycle={macro_cycle} phase={phase} elapsed_us={}",
        elapsed.as_micros()
    );
}

fn set_g5_baseline(active: &State<usize>, overlay_mode: &State<u8>, theme_control: &ThemeControl) {
    if theme_control.is_dark() && !theme_control.toggle() {
        tracing::error!("G5 baseline theme transition failed");
    }
    active.set(PAGE_HOME);
    overlay_mode.set(0);
}

fn advance_g5_state_machine(
    active: &State<usize>,
    ticks: &State<u32>,
    overlay_mode: &State<u8>,
    theme_control: &ThemeControl,
    state: &mut G5ScenarioState,
) -> Duration {
    let elapsed = state.started.elapsed();
    let warmup = Duration::from_secs(G5_WARMUP_SECONDS);
    let protocol_elapsed = elapsed.saturating_sub(warmup);
    let macro_cycle = protocol_elapsed.as_secs() / G5_MACRO_CYCLE_SECONDS;
    let elapsed_in_cycle =
        protocol_elapsed.saturating_sub(Duration::from_secs(macro_cycle * G5_MACRO_CYCLE_SECONDS));
    let idle_start = Duration::from_secs(G5_MACRO_CYCLE_SECONDS - G5_IDLE_SECONDS);
    let should_idle = elapsed_in_cycle >= idle_start;

    if elapsed >= warmup && (macro_cycle != state.macro_cycle || should_idle != state.idle) {
        state.macro_cycle = macro_cycle;
        state.idle = should_idle;
        set_g5_baseline(active, overlay_mode, theme_control);
        if should_idle {
            uix::core::perf_probe::set_internal_g5_scenario(&format!("idle.{macro_cycle}"));
            log_g5_phase(macro_cycle, "idle", protocol_elapsed);
        } else {
            log_g5_scenario("page_home", "page", 0, PAGE_HOME, "light", "none");
            log_g5_phase(macro_cycle, "interaction", protocol_elapsed);
        }
    }

    if should_idle {
        let cycle_end = warmup + Duration::from_secs((macro_cycle + 1) * G5_MACRO_CYCLE_SECONDS);
        return cycle_end
            .saturating_sub(elapsed)
            .max(Duration::from_millis(1));
    }

    state.pulse = state.pulse.wrapping_add(1);
    ticks.update(|value| *value = value.wrapping_add(1));
    if state.pulse.is_multiple_of(125) {
        state.phase = (state.phase + 1) % 8;
        let (name, category, page, dark, overlay, iteration) = match state.phase {
            0 => ("page_home", "page", PAGE_HOME, false, 0, 0),
            1 => {
                state.theme_iteration = state.theme_iteration.wrapping_add(1);
                (
                    "page_general_dark",
                    "theme",
                    PAGE_GENERAL,
                    true,
                    0,
                    state.theme_iteration,
                )
            }
            2 => (
                "page_general_light",
                "theme",
                PAGE_GENERAL,
                false,
                0,
                state.theme_iteration,
            ),
            3 => {
                state.modal_iteration = state.modal_iteration.wrapping_add(1);
                (
                    "modal_feedback",
                    "modal",
                    PAGE_FEEDBACK,
                    true,
                    1,
                    state.modal_iteration,
                )
            }
            4 => (
                "modal_closed",
                "modal",
                PAGE_FEEDBACK,
                true,
                0,
                state.modal_iteration,
            ),
            5 => {
                state.drawer_iteration = state.drawer_iteration.wrapping_add(1);
                (
                    "drawer_feedback",
                    "drawer",
                    PAGE_FEEDBACK,
                    false,
                    2,
                    state.drawer_iteration,
                )
            }
            6 => (
                "drawer_closed",
                "drawer",
                PAGE_FEEDBACK,
                false,
                0,
                state.drawer_iteration,
            ),
            7 => ("page_charts", "page", PAGE_CHARTS, false, 0, 0),
            _ => unreachable!("G5 phase is modulo eight"),
        };
        if theme_control.is_dark() != dark && !theme_control.toggle() {
            tracing::error!("G5 theme transition failed");
        }
        active.set(page);
        overlay_mode.set(overlay);
        log_g5_scenario(
            name,
            category,
            iteration,
            page,
            if dark { "dark" } else { "light" },
            match overlay {
                1 => "modal",
                2 => "drawer",
                _ => "none",
            },
        );
    }
    Duration::from_millis(16)
}

fn schedule_g5_tick(
    handle: AppHandle,
    active: State<usize>,
    ticks: State<u32>,
    overlay_mode: State<u8>,
    theme_control: ThemeControl,
    mut state: G5ScenarioState,
    delay: Duration,
) {
    let callback_handle = handle.clone();
    handle
        .run_after(delay, move || {
            let next_delay = advance_g5_state_machine(
                &active,
                &ticks,
                &overlay_mode,
                &theme_control,
                &mut state,
            );
            schedule_g5_tick(
                callback_handle,
                active,
                ticks,
                overlay_mode,
                theme_control,
                state,
                next_delay,
            );
        })
        .detach();
}

fn start_g5_control_watcher() {
    let Some(path) = std::env::var_os("UIX_G5_STOP_FILE") else {
        tracing::info!(
            "G5_CAPABILITY schema=1 category=control status=blocked reason=missing_stop_file"
        );
        return;
    };
    let Ok(token) = std::env::var("UIX_G5_STOP_TOKEN") else {
        tracing::info!(
            "G5_CAPABILITY schema=1 category=control status=blocked reason=missing_stop_token"
        );
        return;
    };
    if token.is_empty() {
        tracing::info!(
            "G5_CAPABILITY schema=1 category=control status=blocked reason=empty_stop_token"
        );
        return;
    }
    if let Err(error) = std::thread::Builder::new()
        .name("uix-g5-control".to_string())
        .spawn(move || loop {
            let requested = std::fs::read_to_string(&path)
                .ok()
                .is_some_and(|value| value.trim() == token);
            if requested {
                tracing::info!("G5_CONTROL schema=1 outcome=complete");
                std::process::exit(0);
            }
            std::thread::sleep(Duration::from_millis(200));
        })
    {
        tracing::error!("G5 control watcher failed: {error}");
    }
}

fn start_g5_state_machine(
    handle: &AppHandle,
    active: &State<usize>,
    timer_ticks: &State<u32>,
    overlay_mode: &State<u8>,
    theme_control: &ThemeControl,
) {
    let _ = uix::core::perf_probe::process_monotonic_us();
    tracing::info!(
        "G5_CAPABILITY schema=1 category=theme status=ready reason=post_present_tree_resource_baseline_observer"
    );
    tracing::info!(
        "G5_CAPABILITY schema=1 category=modal status=ready reason=post_present_overlay_inventory_resource_baseline_observer"
    );
    tracing::info!(
        "G5_CAPABILITY schema=1 category=drawer status=ready reason=post_present_overlay_inventory_resource_baseline_observer"
    );
    tracing::info!(
        "G5_CAPABILITY schema=1 category=window status=blocked reason=no_release_safe_window_automation"
    );
    tracing::info!(
        "G5_CAPABILITY schema=1 category=dpi status=blocked reason=requires_controlled_multi_dpi_environment"
    );
    log_g5_scenario("page_home", "page", 0, PAGE_HOME, "light", "none");
    start_g5_control_watcher();
    schedule_g5_tick(
        handle.clone(),
        active.clone(),
        timer_ticks.clone(),
        overlay_mode.clone(),
        theme_control.clone(),
        G5ScenarioState::new(),
        Duration::from_millis(16),
    );
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

pub fn run(
    agent_control: bool,
    follow_system_theme: bool,
    graphics_recovery_acceptance: bool,
    component_qa: bool,
    g5_release_scenario: bool,
    empty: bool,
    crash_dir: Option<std::path::PathBuf>,
) {
    let active = State::new(if empty {
        PAGE_HOME
    } else if component_qa {
        PAGE_COMPONENT_QA
    } else {
        PAGE_HOME
    });
    let timer_ticks = State::new(0u32);
    let component_case = State::new(0usize);
    let home_count = State::new(0i32);
    let runtime_count = State::new(0i32);
    let theme_control = ThemeControl::new(follow_system_theme);
    let graphics_recovery_control = GraphicsRecoveryControl::new(graphics_recovery_acceptance);
    let framework_control = FrameworkControl::default();
    let g5_overlay_mode = State::new(0u8);

    let app = App::new()
        .title("UIX Demo")
        .size(INIT_W, INIT_H)
        .custom_title_bar(true)
        .theme(Theme::antd_light())
        .follow_system_theme(follow_system_theme);
    let app = match crash_dir {
        Some(directory) => app.diagnostics(
            uix::diagnostics::DiagnosticsConfig::default().crash_report_directory(directory),
        ),
        None => app,
    };
    let app = app.on_start(
        with_cloned!(theme_control, graphics_recovery_control, timer_ticks, active, g5_overlay_mode; |handle| {
                if empty {
                    // 内存剖析空窗口基线：不启动任何定时器/状态机。
                    return;
                }
                theme_control.set_handle(handle.clone());
                graphics_recovery_control.set_handle(handle.clone());
                if g5_release_scenario {
                    start_g5_state_machine(
                        &handle,
                        &active,
                        &timer_ticks,
                        &g5_overlay_mode,
                        &theme_control,
                    );
                } else {
                    let ticks = timer_ticks.clone();
                    handle
                        .run_interval(Duration::from_secs(1), move || {
                            ticks.update(|v| *v = v.wrapping_add(1));
                        })
                        .detach();
                }
                // 动画样例改走 WidgetAnimation（Spin 等）；不再全局 16ms 探活，
                // 否则 RegisteredActive 永不 DeepIdle，且曾把 orphan State 绑成整树 reconcile。
                // Component QA owns page navigation for deterministic real-window
                // tests; the standalone startup scenario must not replace its page.
                if !component_qa
                    && !g5_release_scenario
                    && std::env::var_os("UIX_PERF_PROBE").is_some()
                {
                    tracing::info!("PERF_SCENARIO=startup scheduled");
                    let page = active.clone();
                    // Delays are wall-clock from on_start; first paint can take seconds,
                    // so keep later scenarios well after that cost settles.
                    handle
                        .run_after(Duration::from_millis(5000), {
                            let page = page.clone();
                            move || {
                                tracing::info!("PERF_SCENARIO=page_switch_general");
                                page.set(2);
                            }
                        })
                        .detach();
                    handle
                        .run_after(Duration::from_millis(9000), {
                            let page = page.clone();
                            move || {
                                tracing::info!("PERF_SCENARIO=page_switch_input");
                                page.set(5);
                            }
                        })
                        .detach();
                    handle
                        .run_after(Duration::from_millis(13000), {
                            let page = page.clone();
                            move || {
                                tracing::info!("PERF_SCENARIO=page_switch_home");
                                page.set(0);
                            }
                        })
                        .detach();
                    handle
                        .run_after(Duration::from_millis(16000), || {
                            tracing::info!("PERF_SCENARIO=await_timer_tick");
                        })
                        .detach();
                    handle
                        .run_after(Duration::from_millis(18500), || {
                            tracing::info!("PERF_SCENARIO=idle_window_expect_no_frame");
                        })
                        .detach();
                    handle
                        .run_after(Duration::from_millis(20000), || {
                            tracing::info!("PERF_SCENARIO=done");
                            std::process::exit(0);
                        })
                        .detach();
                }
            })
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
            if empty {
                // 内存剖析空窗口基线：不渲染任何内容，评估框架+驱动固定开销。
                column(())
            } else {
                let shell = demo_window(app_shell_with_controls(
                    active,
                    timer_ticks,
                    &component_case,
                    &home_count,
                    &runtime_count,
                    &theme_control,
                    graphics_recovery_control.enabled().then_some(&graphics_recovery_control),
                    &framework_control,
                ));
                if g5_release_scenario {
                    g5_release_root(shell, &g5_overlay_mode)
                } else {
                    shell
                }
            }
        }
    ))
    .run();
}
