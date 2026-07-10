//! UIX 多页 GUI 演示 — `cargo run --bin uix-demo`

use std::sync::{Arc, Mutex};
use std::time::Duration;

use uix::prelude::*;

use crate::common::page::{page_heading, INIT_H, INIT_W, PAGE_TITLES, SIDEBAR_GROUPS, SIDEBAR_W};
use crate::demos::context::ThemeControl;
use crate::demos::{build_page, DemoCtx};

fn sidebar_item(
    active: State<usize>,
    index: usize,
    icon: &str,
    label_text: &str,
    tk: &DesignTokens,
) -> ViewNode {
    let active_set = active.clone();
    let title = label_text.trim().to_string();
    let is_active = active.get() == index;

    let text_color = if is_active {
        ColorValue::Palette(PaletteColor::Primary)
    } else {
        ColorValue::Neutral(NeutralRole::TextSecondary)
    };

    // Center：避免默认 Stretch 把 Label 拉高后顶对齐，导致文字相对图标偏上。
    // margin 12：左右对称，高亮不贴边；gap 替代 space(10)（后者在 row 中宽为 0）。
    let mut item = row([
        embed(Icon::new(icon).size(16.0)),
        label(title).font_size(13.0).color(text_color),
    ])
    .align(AlignItems::Center)
    .gap(10.0)
    .margin(EdgeInsets::new(12.0, 0.0, 12.0, 0.0))
    .padding(EdgeInsets::new(12.0, 8.0, 12.0, 8.0))
    .radius(tk.border_radius_sm);

    if is_active {
        item = item.bg(ColorValue::Palette(PaletteColor::PrimaryBg));
    }

    item.on_semantic(SemanticKind::Click, move |_| active_set.set(index))
}

fn sidebar_group_label(_tk: &DesignTokens, text: &str) -> ViewNode {
    // 左缘与导航项图标对齐：item.margin(12) + item.padding(12) = 24
    label(text)
        .font_size(11.0)
        .color(ColorValue::Neutral(NeutralRole::TextQuaternary))
        .margin(EdgeInsets::new(24.0, 16.0, 12.0, 8.0))
}

fn sidebar_brand(_tk: &DesignTokens) -> ViewNode {
    row([
        embed(
            Icon::new("box")
                .size(22.0),
        ),
        column([
            label("UIX Demo")
                .font_size(17.0)
                .color(ColorValue::Neutral(NeutralRole::Text)),
            label("Component Showcase")
                .font_size(11.0)
                .color(ColorValue::Neutral(NeutralRole::TextTertiary)),
        ])
        .flex_grow(0.0),
    ])
    .align(AlignItems::Center)
    .gap(10.0)
    .padding(EdgeInsets::new(16.0, 20.0, 12.0, 16.0))
}

fn sidebar(active: State<usize>, tk: &DesignTokens) -> ViewNode {
    let mut items = vec![sidebar_brand(tk), embed(Divider::new())];
    for (group_name, indices) in SIDEBAR_GROUPS {
        items.push(sidebar_group_label(tk, group_name));
        for &i in *indices {
            let (icon, title) = PAGE_TITLES[i];
            items.push(sidebar_item(active.clone(), i, icon, title, tk));
        }
    }
    items.push(label("").flex_grow(1.0));
    items.push(embed(Divider::new()));
    items.push(
        label("cargo run --bin uix-demo")
            .font_size(10.0)
            .color(ColorValue::Neutral(NeutralRole::TextQuaternary))
            .padding(EdgeInsets::new(12.0, 16.0, 2.0, 8.0)),
    );
    items.push(
        label("UIX v0.1.0")
            .font_size(11.0)
            .color(ColorValue::Neutral(NeutralRole::TextQuaternary))
            .padding(EdgeInsets::new(2.0, 16.0, 16.0, 8.0)),
    );
    column(items)
        .width(SIDEBAR_W)
        .bg(ColorValue::Neutral(NeutralRole::BgElevated))
        .flex_grow(0.0)
}

fn header_bar(
    active: &State<usize>,
    timer_ticks: &State<u32>,
    theme_control: &ThemeControl,
) -> ViewNode {
    let active_for_title = active.clone();
    let ticks = timer_ticks.clone();
    // 顶栏不可 flex_grow：column() 默认 grow=1 会与 page_shell 对半分高，
    // 切到内容更高的页后顶栏被撑开，正文被推到窗口外（表现为空白页）。
    column([
        row([
            dynamic_label(move || {
                let idx = active_for_title.get();
                let (_, title) = PAGE_TITLES[idx];
                title.trim().to_string()
            })
            .font_size(15.0)
            .color(ColorValue::Neutral(NeutralRole::TextSecondary))
            .padding(EdgeInsets::new(24.0, 0.0, 0.0, 0.0)),
            label("").flex_grow(1.0),
            dynamic_label(move || format!("⏱ {}", ticks.get()))
                .font_size(11.0)
                .color(ColorValue::Neutral(NeutralRole::TextTertiary))
                .padding(EdgeInsets::new(0.0, 0.0, 0.0, 12.0)),
            embed(ThemeToggle::new().dark(theme_control.is_dark())).on_semantic(
                SemanticKind::Click,
                {
                    let theme_control = theme_control.clone();
                    move |_| {
                        theme_control.toggle();
                    }
                },
            ),
        ])
        .bg(ColorValue::Neutral(NeutralRole::BgContainer)),
        embed(Divider::new()),
    ])
    .flex_grow(0.0)
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
    column([
        header_bar(&active, timer_ticks, theme_control),
        page_content(
            active,
            tk,
            timer_ticks,
            anim_time,
            home_count,
            runtime_count,
            theme_control,
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
    anim_time: &State<f32>,
    home_count: &State<i32>,
    runtime_count: &State<i32>,
    theme_control: &ThemeControl,
) -> ViewNode {
    let (icon, title) = PAGE_TITLES[idx];
    let ctx = DemoCtx::new(tk, timer_ticks, anim_time, Some(active))
        .with_counters(home_count, runtime_count)
        .with_theme_control(theme_control);
    column([
        page_heading(icon, title.trim()).key(format!("heading-{idx}")),
        build_page(idx, &ctx).key(format!("body-{idx}")),
    ])
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
    column([
        row([
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
        .border(1.0, ColorValue::Neutral(NeutralRole::BorderSecondary)),
    ])
    .flex_grow(1.0)
    .bg(ColorValue::Neutral(NeutralRole::BgLayout))
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

pub fn run() {
    let active = State::new(0usize);
    let timer_ticks = State::new(0u32);
    let anim_time = State::new(0.0f32);
    let home_count = State::new(0i32);
    let runtime_count = State::new(0i32);
    let theme_control = ThemeControl::default();

    let active_root = active.clone();
    let timer_for_root = timer_ticks.clone();
    let anim_for_root = anim_time.clone();
    let home_count_for_root = home_count.clone();
    let runtime_count_for_root = runtime_count.clone();
    let theme_control_for_root = theme_control.clone();

    let timer_for_start = timer_ticks.clone();
    let anim_for_start = anim_time.clone();
    // Interval timers are canceled when `TimerHandle` drops — keep alive for app lifetime.
    let interval_handles = Arc::new(Mutex::new(Vec::<TimerHandle>::new()));

    App::new()
        .title("UIX Demo")
        .size(INIT_W, INIT_H)
        .theme(Theme::antd_light())
        .on_start({
            let interval_handles = interval_handles.clone();
            let theme_control = theme_control.clone();
            move |handle| {
                theme_control.set_handle(handle.clone());
                let ticks = timer_for_start.clone();
                interval_handles
                    .lock()
                    .expect("timer keepalive")
                    .push(handle.run_interval(Duration::from_secs(1), move || {
                        ticks.set(ticks.get().wrapping_add(1));
                    }));
                let anim = anim_for_start.clone();
                interval_handles
                    .lock()
                    .expect("timer keepalive")
                    .push(handle.run_interval(Duration::from_millis(16), move || {
                        anim.set(anim.get() + 0.016);
                    }));
            }
        })
        .root(move || {
            app_shell_with_counters(
                active_root.clone(),
                timer_for_root.clone(),
                anim_for_root.clone(),
                &home_count_for_root,
                &runtime_count_for_root,
                &theme_control_for_root,
            )
        })
        .run();
}

#[cfg(test)]
#[path = "../tests/gui.rs"]
mod tests;
