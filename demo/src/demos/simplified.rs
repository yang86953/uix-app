//! 简化 API 演示 — `prelude` + `App::new()` 一键启动。
//!
//! 运行：`cargo run --bin uix-demo -- --simple`
//!
//! 本演示展示入门推荐路径：View DSL（`column` / `row` / `button`）+
//! 响应式 `State` + `App` 生命周期，无需直接接触 `WidgetTree` 或平台 API。

use uix::prelude::*;

// ── 计数器 — State 自动脏标记 + dynamic_label ────────────────────────────

fn counter_section() -> ViewNode {
    let count = State::new(0);
    let label_count = count.clone();
    let increment_count = count.clone();
    let decrement_count = count.clone();
    let reset_count = count.clone();

    column([
        label("计数器").font_size(20.0).padding(8.0),
        dynamic_label(move || format!("当前值: {}", label_count.get()))
            .font_size(28.0)
            .padding(8.0),
        row([
            button("+1").primary().on_click(move || {
                increment_count.set(increment_count.get() + 1);
            }),
            button("-1").on_click(move || {
                let current = decrement_count.get();
                if current > 0 {
                    decrement_count.set(current - 1);
                }
            }),
            button("归零").on_click(move || reset_count.set(0)),
        ])
        .gap(8.0),
    ])
    .gap(8.0)
    .padding(16.0)
    .bg("#ffffff")
    .radius(8.0)
}

// ── 样式 — hex 颜色 + EdgeInsets 数字快捷构造 ────────────────────────────

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

// ── 布局 — column/row/gap/flex_grow ──────────────────────────────────────

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

// ── 入口 ──────────────────────────────────────────────────────────────────

pub fn run_simplified_demo() {
    App::new()
        .title("UIX 简化 API 演示")
        .size(500, 520)
        .theme(Theme::antd_light())
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
