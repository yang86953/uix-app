//! App 应用能力 — State、Timer、Theme、View DSL、PicturePolicy 说明。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder};
use crate::common::showcase::{info_note, labeled_row};
use crate::common::widgets::{BounceBall, Counter, PulseRing};
use crate::demos::context::DemoCtx;

fn theme_window(control: crate::demos::context::ThemeControl) -> ViewNode {
    let toggle_control = control.clone();
    column([
        row([
            embed(Icon::new("layers").size(24.0)),
            label("UIX 多窗口主题联动")
                .font_size(22.0)
                .color(ColorValue::Neutral(NeutralRole::Text)),
            label("").flex_grow(1.0),
            embed(ThemeToggle::new().dark(control.is_dark())).on_semantic(
                SemanticKind::Click,
                move |_| {
                    toggle_control.toggle();
                },
            ),
        ])
        .align(AlignItems::Center)
        .gap(12.0),
        label("此窗口拥有独立 WindowSession，与主窗共享 AppState 和 Theme。")
            .font_size(13.0)
            .color(ColorValue::Neutral(NeutralRole::TextSecondary)),
        column([
            label("palette-only 广播")
                .font_size(15.0)
                .color(ColorValue::Palette(PaletteColor::Primary)),
            label("任一窗口切换主题后，两个 WidgetTree 仅重绘使用语义色的节点。")
                .font_size(12.0)
                .color(ColorValue::Neutral(NeutralRole::TextTertiary)),
        ])
        .gap(8.0)
        .padding(EdgeInsets::uniform(18.0))
        .bg(ColorValue::Neutral(NeutralRole::BgElevated))
        .border(1.0, ColorValue::Neutral(NeutralRole::BorderSecondary))
        .radius(8.0),
        label("").flex_grow(1.0),
        label("AppHandle::open_window + AppHandle::set_theme")
            .font_size(11.0)
            .color(ColorValue::Neutral(NeutralRole::TextQuaternary)),
    ])
    .gap(18.0)
    .padding(EdgeInsets::uniform(24.0))
    .bg(ColorValue::Neutral(NeutralRole::BgLayout))
    .flex_grow(1.0)
}

pub fn page_runtime(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;
    let ticks = ctx.timer_ticks;
    let anim = ctx.anim_time;

    let anim_for_ball = anim.clone();
    let anim_for_ring = anim.clone();
    let theme_toggle = if let Some(control) = ctx.theme_control() {
        let toggle_control = control.clone();
        embed(ThemeToggle::new().dark(control.is_dark())).on_semantic(
            SemanticKind::Click,
            move |_| {
                toggle_control.toggle();
            },
        )
    } else {
        embed(ThemeToggle::new())
    };
    let open_window = if let Some(control) = ctx.theme_control() {
        let open_control = control.clone();
        button("打开主题联动窗口")
            .on_click_fn(move || {
                let Some(handle) = open_control.handle() else {
                    return;
                };
                let window_control = open_control.clone();
                if let Err(error) =
                    handle.open_window(WindowConfig::new("UIX Theme Window", 520, 320, move || {
                        theme_window(window_control.clone())
                    }))
                {
                    handle.notify_error(&error);
                }
            })
            .build()
    } else {
        button("打开主题联动窗口").disabled(true).build()
    };

    PageBuilder::new(tk)
        .gap()
        .section("响应式 State + label")
        .push(info_note(
            tk,
            "State 变更触发 reconcile；label(闭包) / State::map_text 读取最新值。",
        ))
        .push({
            let counter = ctx.runtime_count();
            column((
                counter
                    .map_text(|n| format!("本地计数: {n}"))
                    .font_size(18.0)
                    .color(ColorValue::Palette(PaletteColor::Primary)),
                row((
                    button("+1")
                        .primary()
                        .on_click(&counter, |c| c.set(c.get() + 1)),
                    button("-1").on_click(&counter, |c| {
                        let v = c.get();
                        if v > 0 {
                            c.set(v - 1);
                        }
                    }),
                ))
                .height(36.0)
                .gap(8.0),
            ))
            // 局部内容组保持 intrinsic 高度，不占用页面内容列的剩余空间。
            .gap(8.0)
            .flex_grow(0.0)
        })
        .section("App Timer — run_interval (on_start 注册)")
        .push(
            ticks
                .map_text(|n| format!("全局 tick 计数: {n} (每秒 +1)"))
                .font_size(16.0)
                .color(ColorValue::Neutral(NeutralRole::Text)),
        )
        .push(info_note(
            tk,
            "本应用在 App.on_start 中调用 handle.run_interval(Duration::from_secs(1), …) 更新 timer_ticks State。",
        ))
        .section("View DSL + embed + component! 自定义")
        .push(
            row([
                label("View DSL row/column")
                    .font_size(13.0)
                    .color(ColorValue::Neutral(NeutralRole::Text)),
                space(8.0),
                embed(Icon::new("layers").size(16.0)),
                space(4.0),
                label("+ embed(widget-tree)")
                    .font_size(13.0)
                    .color(ColorValue::Neutral(NeutralRole::TextSecondary)),
            ])
            .padding(8.0),
        )
        .push(labeled_row(tk, 40.0, "Counter", Counter { count: 0 }))
        .section("动画 time (State 驱动)")
        .push(labeled_row(
            tk,
            52.0,
            "PulseRing",
            PulseRing {
                time: anim_for_ring.get(),
            },
        ))
        .push(labeled_row(
            tk,
            64.0,
            "BounceBall",
            BounceBall {
                time: anim_for_ball.get(),
            },
        ))
        .push(info_note(
            tk,
            "anim_time 由 run_interval(16ms) 更新；内置 Spin/ProgressBar 则通过 WidgetAnimation 自驱动。",
        ))
        .section("Theme — light/dark + follow_system_theme")
        .push(
            row([
                theme_toggle,
                label("点击切换 App 全局暗色/亮色 (AppHandle::set_theme)")
                    .color(ColorValue::Neutral(NeutralRole::TextSecondary))
                    .font_size(12.0),
            ])
            .height(40.0)
            .align(AlignItems::Center)
            .gap(8.0),
        )
        .push(info_note(
            tk,
            "App.follow_system_theme(true) 为 opt-in：运行中自动跟 OS 明暗切换。本 Demo 默认 false。",
        ))
        .section("PicturePolicy — 静态 vs 动态")
        .push(info_note(
            tk,
            "Eligible 静态子树可 Picture 缓存；Spin/ProgressBar/动画 widget 走窄 paint。见 rendering.md #122。",
        ))
        .push(
            demo_row(40.0)
                .child(Spin::new())
                .child(
                    Label::new("Spin = 动态")
                        .style(Style {
                            color: ColorValue::Neutral(NeutralRole::TextTertiary),
                            ..Style::default()
                        })
                        .font_size(11.0),
                )
                .child(Card::new().title("Card").elevation(1).size(120.0, 36.0))
                .child(
                    Label::new("Card = 静态候选")
                        .style(Style {
                            color: ColorValue::Neutral(NeutralRole::TextTertiary),
                            ..Style::default()
                        })
                        .font_size(11.0),
                ),
        )
        .section("post_to_ui — 跨线程投递主线程闭包")
        .push(info_note(
            tk,
            "AppHandle::post_to_ui(FnOnce) 将闭包入队到当前 WindowSession 主线程；跨线程 State 更新须经此路径。见 application.md #133。",
        ))
        .section("多窗口")
        .push(open_window)
        .push(info_note(
            tk,
            "AppHandle::open_window 创建独立 WindowSession；主题全局共享，post_to_ui 仍仅进入目标 session 队列。",
        ))
        .build()
}
