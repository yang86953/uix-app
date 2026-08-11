//! UIX 多页 GUI 演示 — `cargo run --manifest-path demo/Cargo.toml --bin uix-demo`

// 常规演示只需要秒级计时器。
use std::time::Duration;

use uix::prelude::*;

// 演示只导入常规页面与组件测试页面索引。
use crate::common::page::{
    page_heading, INIT_H, INIT_W, PAGE_COMPONENT_QA, PAGE_HOME, PAGE_TITLES, SIDEBAR_GROUPS,
    SIDEBAR_W,
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
        label("cargo run --manifest-path demo/Cargo.toml --bin uix-demo")
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

pub fn run(
    agent_control: bool,
    follow_system_theme: bool,
    // 图形故障控制只供自动测试入口启用。
    graphics_recovery_test: bool,
    // 组件测试入口直接打开隔离测试页面。
    component_test: bool,
    crash_dir: Option<std::path::PathBuf>,
) {
    // 常规演示从首页启动，组件测试从隔离页面启动。
    let active = State::new(if component_test {
        PAGE_COMPONENT_QA
    } else {
        PAGE_HOME
    });
    let timer_ticks = State::new(0u32);
    let component_case = State::new(0usize);
    let home_count = State::new(0i32);
    let runtime_count = State::new(0i32);
    let theme_control = ThemeControl::new(follow_system_theme);
    // 自动测试按启动参数决定是否启用图形故障控制。
    let graphics_recovery_control = GraphicsRecoveryControl::new(graphics_recovery_test);
    let framework_control = FrameworkControl::default();

    // Windows 与 Wayland 都通过平台窗口能力隐藏系统装饰并显示同一自定义标题栏。
    let app = App::new()
        // 设置演示窗口标题。
        .title("UIX Demo")
        // 设置演示窗口初始尺寸。
        .size(INIT_W, INIT_H)
        // 启用由 UIX 组件绘制和交互的标题栏。
        .custom_title_bar(true);
    let app = app
        .theme(Theme::antd_light())
        .follow_system_theme(follow_system_theme);
    let app = match crash_dir {
        Some(directory) => app.diagnostics(
            uix::diagnostics::DiagnosticsConfig::default().crash_report_directory(directory),
        ),
        None => app,
    };
    // 启动后只注册演示本身需要的状态与秒级计时器。
    let app = app.on_start(with_cloned!(
        theme_control,
        graphics_recovery_control,
        timer_ticks;
        |handle| {
            theme_control.set_handle(handle.clone());
            graphics_recovery_control.set_handle(handle.clone());
            // 秒级计数器驱动常规演示中的时间变化。
            let ticks = timer_ticks.clone();
            handle
                .run_interval(Duration::from_secs(1), move || {
                    ticks.update(|value| *value = value.wrapping_add(1));
                })
                .detach();
        }
    ));
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

    // 根视图始终渲染常规演示外壳，测试入口只改变初始页面和故障控制。
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
