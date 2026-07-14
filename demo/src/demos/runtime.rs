//! App 应用能力 — State、Computed、Effect、Timer、Theme、Animation、View DSL。
//! 对照 [`使用.md`](../../docs/使用.md) 核心概念。

use uix::prelude::*;

use crate::common::page::PageBuilder;
use crate::common::showcase::{flow_row, info_note, sample_block};
use crate::common::widgets::{BounceBall, Counter, PulseRing};
use crate::demos::context::DemoCtx;

fn theme_window(control: crate::demos::context::ThemeControl) -> impl View {
    let toggle_control = control.clone();
    column((
        row((
            icon("layers"),
            label("UIX 多窗口主题联动")
                .font_size(22.0)
                .fg(ColorValue::Neutral(NeutralRole::Text)),
            label("").flex_grow(1.0),
            ThemeToggle::new()
                .dark(control.is_dark())
                .on_click_fn(move || {
                    toggle_control.toggle();
                }),
        ))
        .gap(12.0),
        label("此窗口拥有独立 WindowSession，与主窗共享 AppState 和 Theme。")
            .font_size(13.0)
            .fg(ColorValue::Neutral(NeutralRole::TextSecondary)),
        column((
            label("palette-only 广播")
                .font_size(15.0)
                .fg(ColorValue::Palette(PaletteColor::Primary)),
            label("任一窗口切换主题后，两个 WidgetTree 仅重绘使用语义色的节点。")
                .font_size(12.0)
                .fg(ColorValue::Neutral(NeutralRole::TextTertiary)),
        ))
        .gap(8.0)
        .padding(18.0)
        .bg(ColorValue::Neutral(NeutralRole::BgElevated))
        .border(1.0, ColorValue::Neutral(NeutralRole::BorderSecondary))
        .radius(8.0),
        label("").flex_grow(1.0),
        label("AppHandle::open_window + AppHandle::set_theme")
            .font_size(11.0)
            .fg(ColorValue::Neutral(NeutralRole::TextQuaternary)),
    ))
    .gap(18.0)
    .padding(24.0)
    .bg(ColorValue::Neutral(NeutralRole::BgLayout))
    .flex_grow(1.0)
}

pub fn page_runtime(ctx: &DemoCtx<'_>) -> impl View {
    let tk = ctx.tk;
    let ticks = ctx.timer_ticks;

    // ── ThemeToggle ──────────────────────────────────────────────
    let theme_toggle = if let Some(control) = ctx.theme_control() {
        let toggle_control = control.clone();
        ThemeToggle::new()
            .dark(control.is_dark())
            .on_click_fn(move || {
                toggle_control.toggle();
            })
    } else {
        ThemeToggle::new()
    };

    // ── 打开子窗口按钮 ──────────────────────────────────────────
    let open_window = if let Some(control) = ctx.theme_control() {
        let open_control = control.clone();
        button("打开主题联动窗口").on_click_fn(move || {
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
    } else {
        button("打开主题联动窗口").disabled(true)
    };

    PageBuilder::new(tk)
        .gap()
        // ── 1. 响应式 State ──────────────────────────────────────
        .block(
            "响应式 State",
            column((
                info_note(
                    tk,
                    "State 变更触发 reconcile；label(闭包) / State::map_text 读取最新值。写入合并为至多一次重绘。",
                ),
                {
                    let counter = ctx.runtime_count();
                    column((
                        counter
                            .map_text(|n| format!("本地计数: {n}"))
                            .font_size(22.0)
                            .fg(tk.color_primary),
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
            ))
            .gap(12.0),
        )
        // ── 2. Computed —— 派生状态 ──────────────────────────────
        .block(
            "Computed —— 派生状态",
            column((
                info_note(
                    tk,
                    "Computed 自动追踪依赖，惰性求值；仅在依赖变化时重算。map_text 转为响应式 View。",
                ),
                {
                    let first = State::new("Ada");
                    let last = State::new("Lovelace");
                    let full = Computed::new(move || format!("{} {}", first.get(), last.get()));
                    column((
                        full.map_text(|s| s.clone())
                            .font_size(20.0)
                            .fg(tk.color_primary),
                        row((
                            button("改姓").on_click_fn({
                                let last = last.clone();
                                move || last.set("Turing")
                            }),
                            button("改名为 Ada")
                                .primary()
                                .on_click_fn({
                                    let first = first.clone();
                                    let last = last.clone();
                                    move || {
                                        first.set("Ada");
                                        last.set("Lovelace");
                                    }
                                }),
                        ))
                        .gap(8.0),
                    ))
                    .gap(10.0)
                },
            ))
            .gap(12.0),
        )
        // ── 3. Effect —— 副作用 ──────────────────────────────────
        .block(
            "Effect —— 副作用",
            column((
                info_note(
                    tk,
                    "Effect 依赖变化时执行（不产生视图），在事件轮次末尾、重绘前批量运行。",
                ),
                {
                    let effect_count = State::new(0);
                    // Effect 自动追踪依赖，每次 count 变化打印日志
                    let _effect = Effect::new({
                        let cnt = effect_count.clone();
                        move || {
                            let v = cnt.get();
                            if v > 0 {
                                log::info!("Effect: count 变为 {v}");
                            }
                        }
                    });
                    row((
                        button("触发 Effect")
                            .primary()
                            .on_click(&effect_count, |c| c.update(|v| *v += 1)),
                        effect_count
                            .map_text(|n| format!("已触发 {n} 次"))
                            .font_size(14.0)
                            .fg(tk.color_text_secondary),
                    ))
                    .gap(12.0)
                },
            ))
            .gap(12.0),
        )
        // ── 4. Animation —— 缓动插值 ─────────────────────────────
        .block(
            "Animation —— 缓动插值",
            column((
                info_note(
                    tk,
                    "Animation 由框架自动推进；窗口不可呈现时暂停，恢复首帧重基。勿用 run_interval 模拟帧动画。",
                ),
                {
                    let anim = Animation::new(0.0, 1.0, 2.0).easing(Easing::ease_in_out);
                    column((
                        label(format!("{:.2}", anim.value()))
                            .font_size(24.0)
                            .fg(tk.color_primary),
                        ProgressBar::new()
                            .percent((anim.value() * 100.0) as f32),
                        row((
                            button("重启动画").on_click_fn(|| {
                                anim.restart();
                            }),
                            label("0.0 → 1.0，ease_in_out，2s")
                                .font_size(12.0)
                                .fg(tk.color_text_tertiary),
                        ))
                        .gap(12.0),
                    ))
                    .gap(10.0)
                },
            ))
            .gap(12.0),
        )
        // ── 5. App Timer — run_interval / run_after ──────────────
        .block(
            "App Timer — run_interval / run_after",
            column((
                ticks
                    .map_text(|n| format!("全局 tick: {n}（每秒 +1）"))
                    .font_size(16.0)
                    .fg(tk.color_text),
                info_note(
                    tk,
                    "App.on_start 中 handle.run_interval(1s) 更新 timer_ticks；drop 自动取消，需存活则 detach。run_after 延时执行一次。",
                ),
            ))
            .gap(12.0),
        )
        // ── 6. View DSL + component! ─────────────────────────────
        .block(
            "View DSL + component!",
            column((
                row((
                    Tag::new("views").color(TagColor::Info),
                    Tag::new("embed").color(TagColor::Success),
                    Tag::new("component!").color(TagColor::Warning),
                ))
                .gap(8.0),
                sample_block(tk, "Counter", Counter { count: 0 }),
            ))
            .gap(12.0),
        )
        // ── 7. 动画 time（State 驱动）─────────────────────────────
        .block(
            "动画 time（State 驱动）",
            column((
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
                info_note(
                    tk,
                    "PulseRing/BounceBall 为静态样例；Spin/ProgressBar 走 WidgetAnimation（无全局 16ms 探活）。",
                ),
            ))
            .gap(12.0),
        )
        // ── 8. Theme ──────────────────────────────────────────────
        .block(
            "Theme",
            column((
                row((
                    theme_toggle,
                    label("AppHandle::set_theme — 全局亮/暗")
                        .fg(tk.color_text_secondary)
                        .font_size(12.0),
                ))
                .gap(12.0),
                info_note(
                    tk,
                    "follow_system_theme(true) 为 opt-in；本 Demo 默认 false。通过 ThemeToggle 或 handle.set_theme(Theme::antd_dark()) 切换。",
                ),
            ))
            .gap(12.0),
        )
        // ── 9. 多窗口 / post_to_ui ───────────────────────────────
        .block(
            "多窗口 / post_to_ui",
            column((
                open_window,
                info_note(
                    tk,
                    "open_window 独立 WindowSession；post_to_ui 仅入目标 session，不唤醒其他窗。从任意线程更新 UI 只走 post_to_ui。",
                ),
            ))
            .gap(12.0),
        )
        .build()
}
