//! 页面构建辅助 — View DSL + `embed` 桥接 widget-tree 片段。

use uix::prelude::*;

pub const INIT_W: i32 = 1200;
pub const INIT_H: i32 = 800;
pub const SIDEBAR_W: f32 = 220.0;
pub const CONTENT_W: f32 = (INIT_W as f32) - SIDEBAR_W;
const CONTENT_PAD_H: f32 = 48.0;
pub const INNER_W: f32 = CONTENT_W - CONTENT_PAD_H;

pub const PAGE_HOME: usize = 0;
pub const PAGE_APP: usize = 1;
pub const PAGE_GENERAL: usize = 2;
pub const PAGE_LAYOUT: usize = 3;
pub const PAGE_NAV: usize = 4;
pub const PAGE_INPUT: usize = 5;
pub const PAGE_DATA: usize = 6;
pub const PAGE_FEEDBACK: usize = 7;
pub const PAGE_CHARTS: usize = 8;
pub const PAGE_OTHER: usize = 9;
pub const PAGE_GALLERY: usize = 10;

pub const PAGE_TITLES: &[(&str, &str)] = &[
    ("home", " 首页"),
    ("cpu", " 应用能力"),
    ("type", " 通用"),
    ("layout", " 布局"),
    ("menu", " 导航"),
    ("edit", " 输入"),
    ("table", " 数据展示"),
    ("alert-circle", " 反馈"),
    ("bar-chart", " 图表"),
    ("settings", " 其他"),
    ("list", " 覆盖清单"),
];

pub const PAGE_COUNT: usize = PAGE_TITLES.len();

const _: () = assert!(PAGE_COUNT > 0);

/// 侧边栏分组：入门 | 组件（8 类）| 参考。
pub const SIDEBAR_GROUPS: &[(&str, &[usize])] = &[
    ("入门", &[PAGE_HOME, PAGE_APP]),
    (
        "组件",
        &[
            PAGE_GENERAL,
            PAGE_LAYOUT,
            PAGE_NAV,
            PAGE_INPUT,
            PAGE_DATA,
            PAGE_FEEDBACK,
            PAGE_CHARTS,
            PAGE_OTHER,
        ],
    ),
    ("参考", &[PAGE_GALLERY]),
];

/// 覆盖矩阵 / 文案中的页面名 → 侧边栏索引。
pub fn page_index_by_label(label: &str) -> Option<usize> {
    let key = label.trim();
    PAGE_TITLES
        .iter()
        .position(|(_, title)| title.trim() == key)
        .or_else(|| match key {
            "运行时" | "App 能力" | "应用能力" => Some(PAGE_APP),
            "目录" | "覆盖清单" => Some(PAGE_GALLERY),
            "数据" | "数据展示" => Some(PAGE_DATA),
            "--cli" => None,
            "—" | "-" => None,
            _ => None,
        })
}

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
        scroll(
            column(self.items)
                .gap(12.0)
                .padding((24.0, 8.0, 24.0, 16.0))
                .flex_grow(0.0),
        )
        .flex_grow(1.0)
        .build()
    }
}

pub fn section_title(tk: &DesignTokens, text: &str) -> ViewNode {
    row([
        label("")
            .width(3.0)
            .height(14.0)
            .bg(tk.color_primary)
            .radius(tk.border_radius_sm),
        space(8.0),
        label(text)
            .color(tk.color_text)
            .font_size(13.0),
    ])
    .padding(EdgeInsets::new(0.0, 0.0, 4.0, 0.0))
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
    column([
        row([
            embed(Icon::new(icon).size(24.0)),
            space(12.0),
            label(title)
                .color(tk.color_text)
                .font_size(24.0),
        ]),
        space(12.0),
        embed(Divider::new()),
    ])
    .flex_grow(0.0)
    .padding(EdgeInsets::new(0.0, 0.0, 8.0, 0.0))
}
