//! App 应用能力 — State、Timer、Theme、View DSL、PicturePolicy 说明。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder};
use crate::common::showcase::{info_note, labeled_row};
use crate::common::widgets::{BounceBall, Counter, PulseRing};
use crate::demos::context::DemoCtx;

pub fn page_runtime(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;
    let ticks = ctx.timer_ticks;
    let anim = ctx.anim_time;

    let tick_label = ticks.clone();
    let anim_for_ball = anim.clone();
    let anim_for_ring = anim.clone();

    PageBuilder::new(tk)
        .gap()
        .section("响应式 State + dynamic_label")
        .push(info_note(
            tk,
            "State 变更触发 reconcile；dynamic_label 绑定闭包读取最新值。",
        ))
        .push({
            let counter = State::new(0i32);
            let display = counter.clone();
            let inc = counter.clone();
            let dec = counter.clone();
            tree! { Container::new().dir(FlexDirection::Column).gap(8.0) => [
                embed(
                    dynamic_label(move || format!("本地计数: {}", display.get()))
                        .font_size(18.0)
                        .color(tk.color_primary),
                ),
                embed(demo_row(36.0)
                    .child(button("+1").primary().on_click(move || inc.set(inc.get() + 1)).widget())
                    .child(button("-1").on_click(move || {
                        let v = dec.get();
                        if v > 0 { dec.set(v - 1); }
                    }).widget())),
            ]}
        })
        .section("App Timer — run_interval (on_start 注册)")
        .push(
            dynamic_label(move || format!("全局 tick 计数: {} (每秒 +1)", tick_label.get()))
                .font_size(16.0)
                .color(tk.color_text),
        )
        .push(info_note(
            tk,
            "本应用在 App.on_start 中调用 handle.run_interval(Duration::from_secs(1), …) 更新 timer_ticks State。",
        ))
        .section("View DSL + embed + component! 自定义")
        .push(
            row([
                label("View DSL row/column").font_size(13.0).color(tk.color_text),
                space(8.0),
                embed(Icon::new("layers").size(16.0)),
                space(4.0),
                label("+ embed(widget-tree)").font_size(13.0).color(tk.color_text_secondary),
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
            demo_row(40.0)
                .child(ThemeToggle::new())
                .child(
                    Label::new("点击切换暗色/亮色 (ThemeToggle)")
                        .color(tk.color_text_secondary)
                        .font_size(12.0),
                ),
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
                        .color(tk.color_text_tertiary)
                        .font_size(11.0),
                )
                .child(Card::new().title("Card").elevation(1).size(120.0, 36.0))
                .child(
                    Label::new("Card = 静态候选")
                        .color(tk.color_text_tertiary)
                        .font_size(11.0),
                ),
        )
        .section("post_to_ui — 跨线程投递主线程闭包")
        .push(info_note(
            tk,
            "AppHandle::post_to_ui(FnOnce) 将闭包入队到当前 WindowSession 主线程；跨线程 State 更新须经此路径。见 application.md #133。",
        ))
        .section("多窗口")
        .push(info_note(
            tk,
            "App 支持多 WindowSession（AppHandle 含 window_id）；post_to_ui 仅入目标 session 队列。单进程多窗 live demo 待产品化 — 见覆盖清单说明。",
        ))
        .build()
}
