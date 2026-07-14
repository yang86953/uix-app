//! App 应用能力 — State、Timer、Theme、View DSL、PicturePolicy 说明。

use uix::prelude::*;

use crate::common::page::PageBuilder;
use crate::common::showcase::{flow_row, info_note, sample_block};
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
            embed(ThemeToggle::new().dark(control.is_dark())).on_click_fn(move || {
                toggle_control.toggle();
            }),
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
        .block(
            "响应式 State",
            column_fit([
                info_note(
                    tk,
                    "State 变更触发 reconcile；label(闭包) / State::map_text 读取最新值。",
                ),
                {
                    let counter = ctx.runtime_count();
                    column_fit((
                        counter
                            .map_text(|n| format!("本地计数: {n}"))
                            .font_size(22.0)
                            .color(ColorValue::Palette(PaletteColor::Primary)),
                        row((
                            button("+1")
                                .primary()
                                .on_click(&counter, |c| c.update(|v| *v += 1)),
                            button("-1").on_click(&counter, |c| {
                                c.update(|v| {
                                    if *v > 0 {
                                        *v -= 1;
                                    }
                                });
                            }),
                        ))
                        .gap(8.0),
                    ))
                    .gap(10.0)
                },
            ])
            .gap(12.0),
        )
        .block(
            "App Timer — run_interval",
            column_fit([
                ticks
                    .map_text(|n| format!("全局 tick: {n}（每秒 +1）"))
                    .font_size(16.0)
                    .color(ColorValue::Neutral(NeutralRole::Text)),
                info_note(
                    tk,
                    "App.on_start 中 handle.run_interval(1s) 更新 timer_ticks；回调已 detach。",
                ),
            ])
            .gap(12.0),
        )
        .block(
            "View DSL + component!",
            column_fit([
                row([
                    embed(Tag::new("views").color(TagColor::Info)),
                    embed(Tag::new("embed").color(TagColor::Success)),
                    embed(Tag::new("component!").color(TagColor::Warning)),
                ])
                .gap(8.0),
                embed(sample_block(tk, "Counter", Counter { count: 0 })),
            ])
            .gap(12.0),
        )
        .block(
            "动画 time（State 驱动）",
            column_fit([
                embed(
                    flow_row(72.0)
                        .child(sample_block(
                            tk,
                            "PulseRing",
                            PulseRing { time: 0.8 },
                        ))
                        .child(sample_block(
                            tk,
                            "BounceBall",
                            BounceBall { time: 1.2 },
                        )),
                ),
                info_note(
                    tk,
                    "PulseRing/BounceBall 为静态样例；Spin/ProgressBar 走 WidgetAnimation（无全局 16ms 探活）。",
                ),
            ])
            .gap(12.0),
        )
        .block(
            "Theme",
            column_fit([
                row([
                    theme_toggle,
                    label("AppHandle::set_theme — 全局亮/暗")
                        .color(ColorValue::Neutral(NeutralRole::TextSecondary))
                        .font_size(12.0),
                ])
                .align(AlignItems::Center)
                .gap(12.0),
                info_note(
                    tk,
                    "follow_system_theme(true) 为 opt-in；本 Demo 默认 false。",
                ),
            ])
            .gap(12.0),
        )
        .block(
            "PicturePolicy",
            column_fit([
                info_note(
                    tk,
                    "Eligible 静态子树可 Picture 缓存；动态 widget 走窄 paint。",
                ),
                embed(
                    flow_row(56.0)
                        .child(sample_block(tk, "Spin=动态", Spin::new()))
                        .child(sample_block(
                            tk,
                            "Card=静态",
                            Card::new().title("Card").elevation(1).size(120.0, 40.0),
                        )),
                ),
            ])
            .gap(12.0),
        )
        .block(
            "多窗口 / post_to_ui",
            column_fit([
                open_window,
                info_note(
                    tk,
                    "open_window 独立 WindowSession；post_to_ui 仅入目标 session。",
                ),
            ])
            .gap(12.0),
        )
        .build()
}
