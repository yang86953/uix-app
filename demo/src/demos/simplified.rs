//! 简化 API 演示 — 展示 View 组合子、State 自动脏标记、App 一键启动。
//!
//! 运行：`cargo run --bin uix-demo -- --simple`

use uix::ui::view::*;
use uix::ui::{App, State};

// ── 计数器 —— 展示 State 自动脏标记 + 响应式 Label ─────────────

fn counter_section() -> ViewNode {
    let count = State::new(0);

    column([
        label("计数器").font_size(20.0).padding(8.0),
        // 响应式 label：每次 count 变更自动触发重绘，文本自动更新
        dynamic_label(move || format!("当前值: {}", count.get()))
            .font_size(28.0)
            .padding(8.0),
        row([
            button("+1").primary().on_click(move || count += 1),
            button("-1").on_click(move || {
                if count.get() > 0 {
                    count -= 1;
                }
            }),
            button("归零").on_click(move || count.set(0)),
        ])
        .gap(8.0),
    ])
    .gap(8.0)
    .padding(16.0)
    .bg("#ffffff")
    .radius(8.0)
}

// ── 样式展示 —— 展示 hex 颜色 + EdgeInsets 数字快捷构造 ────────

fn style_demo_section() -> ViewNode {
    column([
        label("样式展示（hex 颜色 + 数字 EdgeInsets）")
            .font_size(20.0)
            .padding((0.0, 0.0, 0.0, 8.0)),
        row([
            label("红色背景")
                .bg("#ff4d4f")
                .color("#ffffff")
                .padding(12.0)
                .radius(6.0),
            label("蓝色边框")
                .border(2.0, "#1677ff")
                .radius(6.0)
                .padding(12.0),
            label("浅灰圆角").bg("#f5f5f5").padding(12.0).radius(12.0),
        ])
        .gap(8.0),
    ])
    .gap(4.0)
    .padding(16.0)
    .bg("#ffffff")
    .radius(8.0)
}

// ── 布局展示 —— 展示 column/row/gap/flex_grow ──────────────────

fn layout_section() -> ViewNode {
    column([
        label("布局展示（flex_grow 等宽分栏）")
            .font_size(20.0)
            .padding((0.0, 0.0, 0.0, 8.0)),
        label("三列等宽："),
        row([
            label("A")
                .bg("#1677ff")
                .color("#ffffff")
                .padding(12.0)
                .flex_grow(1.0)
                .radius(4.0),
            label("B")
                .bg("#52c41a")
                .color("#ffffff")
                .padding(12.0)
                .flex_grow(1.0)
                .radius(4.0),
            label("C")
                .bg("#ff4d4f")
                .color("#ffffff")
                .padding(12.0)
                .flex_grow(1.0)
                .radius(4.0),
        ])
        .gap(8.0),
    ])
    .gap(4.0)
    .padding(16.0)
    .bg("#ffffff")
    .radius(8.0)
}

// ── 入口 ──────────────────────────────────────────────────────

pub fn run_simplified_demo() {
    let theme = uix::ui::theme::Theme::antd_light();

    App::new()
        .title("UIX 简化 API 演示")
        .size(500, 520)
        .theme(theme)
        .root(
            column([
                label("UIX 简化 API 演示")
                    .font_size(24.0)
                    .padding((16.0, 12.0, 16.0, 4.0)),
                counter_section(),
                space(8.0),
                style_demo_section(),
                space(8.0),
                layout_section(),
            ])
            .gap(0.0),
        )
        .run();
}
