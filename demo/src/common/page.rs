//! 页面构建辅助 — View DSL + `embed` 桥接 widget-tree 片段。

use uix::prelude::*;

pub const INIT_W: i32 = 1200;
pub const INIT_H: i32 = 800;
pub const SIDEBAR_W: f32 = 200.0;
pub const CONTENT_W: f32 = (INIT_W as f32) - SIDEBAR_W;
const CONTENT_PAD_H: f32 = 40.0;
pub const INNER_W: f32 = CONTENT_W - CONTENT_PAD_H;

pub const PAGE_TITLES: &[(&str, &str)] = &[
    ("type", " 通用"),
    ("layout", " 布局"),
    ("menu", " 导航"),
    ("edit", " 输入"),
    ("table", " 数据展示"),
    ("alert-circle", " 反馈"),
    ("bar-chart", " 图表"),
    ("settings", " 其他"),
];

/// 声明式页面构建器：`section` + `push` + `scroll(column(...))`。
pub struct PageBuilder<'a> {
    tk: &'a DesignTokens,
    items: Vec<ViewNode>,
}

impl<'a> PageBuilder<'a> {
    pub fn new(tk: &'a DesignTokens) -> Self {
        Self {
            tk,
            items: Vec::new(),
        }
    }

    pub fn gap(mut self) -> Self {
        self.items.push(space(12.0));
        self
    }

    pub fn section(mut self, title: &str) -> Self {
        self.items.push(section_title(self.tk, title));
        self
    }

    pub fn push(mut self, node: impl IntoWidgetNode) -> Self {
        self.items.push(embed(node));
        self
    }

    pub fn build(self) -> ViewNode {
        let mut children = self.items;
        children.push(label("").flex_grow(1.0));
        scroll(
            column(children)
                .gap(8.0)
                .padding((20.0, 4.0, 20.0, 10.0))
                .flex_grow(1.0),
        )
        .flex_grow(1.0)
        .build()
    }
}

pub fn section_title(tk: &DesignTokens, text: &str) -> ViewNode {
    label(text)
        .color(tk.color_text_secondary)
        .font_size(12.0)
}

/// 固定高度水平 Space 行（与 View `row` 组合子区分）。
pub fn demo_row(h: f32) -> Space {
    Space::new()
        .size(SpaceSize::Small)
        .height(h)
        .direction(FlexDirection::Row)
        .align(AlignItems::Center)
}

pub fn page_heading(tk: &DesignTokens, icon: &str, title: &str) -> ViewNode {
    row([
        embed(Icon::new(icon).size(22.0)),
        space(8.0),
        label(title).color(tk.color_text).font_size(22.0),
    ])
    .height(30.0)
    .padding((0.0, 4.0, 0.0, 0.0))
}
