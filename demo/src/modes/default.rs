//! 默认 GUI 演示 — `prelude` + `App::new().root()` 入门路径。
//!
//! 运行：`cargo run --bin uix-demo`

use uix::prelude::*;

use crate::common::widgets::Counter;

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
            button("+1")
                .primary()
                .on_click(move || increment_count.set(increment_count.get() + 1)),
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

fn component_section() -> ViewNode {
    column([
        label("component! 自定义 Widget").font_size(20.0).padding(8.0),
        label("点击 Counter 区域递增（PointerDown）")
            .font_size(12.0)
            .color("#888888"),
        embed(Counter { count: 0 }),
    ])
    .gap(8.0)
    .padding(16.0)
    .bg("#ffffff")
    .radius(8.0)
}

fn style_section() -> ViewNode {
    column([
        label("样式 — hex 颜色与 EdgeInsets")
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
            label("浅灰圆角")
                .bg("#f5f5f5")
                .padding(12.0)
                .radius(12.0),
        ])
        .gap(8.0),
    ])
    .gap(4.0)
    .padding(16.0)
    .bg("#ffffff")
    .radius(8.0)
}

fn layout_section() -> ViewNode {
    column([
        label("布局 — flex_grow 等宽分栏")
            .font_size(20.0)
            .padding((0.0, 0.0, 0.0, 8.0)),
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

pub fn run() {
    App::new()
        .title("UIX 入门演示")
        .size(520, 620)
        .theme(Theme::antd_light())
        .root(|| {
            scroll(column([
                label("UIX 入门演示")
                    .font_size(24.0)
                    .padding((16.0, 12.0, 16.0, 4.0)),
                counter_section(),
                space(8.0),
                component_section(),
                space(8.0),
                style_section(),
                space(8.0),
                layout_section(),
            ]))
            .build()
        })
        .run();
}
