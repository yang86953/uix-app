//! 页面构建辅助 — 声明式 View DSL 组合。

use uix::prelude::*;

pub const INIT_W: i32 = 1200;
pub const INIT_H: i32 = 800;
pub const SIDEBAR_W: f32 = 220.0;
pub const CONTENT_W: f32 = (INIT_W as f32) - SIDEBAR_W;
const CONTENT_PAD_H: f32 = 48.0;
/// 初始窗口下内容区内宽（演示页固定样例尺寸用）。
/// **不要**拿它当「当前窗口内容宽」——窗口 resize 后仍是编译期常量。
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

/// 声明式页面构建器：`section` + `push` 自动收入抬升面板。
pub struct PageBuilder<'a> {
    tk: &'a DesignTokens,
    items: Vec<ViewNode>,
    open_section: Option<String>,
    section_body: Vec<ViewNode>,
}

impl<'a> PageBuilder<'a> {
    pub fn new(tk: &'a DesignTokens) -> Self {
        Self {
            tk,
            items: Vec::new(),
            open_section: None,
            section_body: Vec::new(),
        }
    }

    fn flush_section(&mut self) {
        let Some(title) = self.open_section.take() else {
            return;
        };
        let body = std::mem::take(&mut self.section_body);
        self.items.push(section_title(self.tk, &title));
        let panel_body = if body.len() == 1 {
            body.into_iter().next().expect("one body node")
        } else {
            column(body).gap(12.0)
        };
        self.items
            .push(crate::common::showcase::panel(self.tk, panel_body));
    }

    pub fn gap(mut self) -> Self {
        self.flush_section();
        self.items.push(space(12.0));
        self
    }

    pub fn section(mut self, title: &str) -> Self {
        self.flush_section();
        self.open_section = Some(title.to_string());
        self
    }

    /// 分区标题 + 抬升面板（画廊主模式）。
    pub fn block(mut self, title: &str, body: ViewNode) -> Self {
        self.flush_section();
        self.items.push(section_title(self.tk, title));
        self.items
            .push(crate::common::showcase::panel(self.tk, body));
        self
    }

    pub fn push(mut self, node: impl IntoViewChildren) -> Self {
        let children = node.into_view_children();
        if self.open_section.is_some() {
            self.section_body.extend(children);
        } else {
            self.items.extend(children);
        }
        self
    }

    pub fn push_view(mut self, node: ViewNode) -> Self {
        if self.open_section.is_some() {
            self.section_body.push(node);
        } else {
            self.items.push(node);
        }
        self
    }

    pub fn build(mut self) -> ViewNode {
        self.flush_section();
        scroll(
            column(self.items)
                .gap(16.0)
                .padding((20.0, 8.0, 28.0, 20.0))
                .overflow_content(),
        )
        .both()
        .flex_grow(1.0)
        .build()
    }
}

pub fn section_title(tk: &DesignTokens, text: &str) -> ViewNode {
    row((
        column(())
            .width(3.0)
            .height(14.0)
            .bg(tk.color_primary)
            .radius(tk.border_radius_sm),
        label(text).fg(tk.color_text).font_size(13.0).flex_grow(0.0),
        column(())
            .height(1.0)
            .flex_grow(1.0)
            .bg(tk.color_border_secondary),
    ))
    .align(AlignItems::Center)
    .gap(8.0)
    .padding(EdgeInsets::new(0.0, 0.0, 4.0, 0.0))
}

/// 固定高度水平行。
pub fn demo_row(h: f32) -> ViewNode {
    row(()).height(h).align(AlignItems::Center).gap(8.0)
}

pub fn page_heading(tk: &DesignTokens, icon: &str, title: &str) -> ViewNode {
    column((
        row((
            Icon::new(icon).size(24.0),
            label(title).fg(tk.color_text).font_size(24.0),
        ))
        .align(AlignItems::Center)
        .gap(12.0),
        space(12.0),
        Divider::new(),
    ))
    .padding(EdgeInsets::new(0.0, 0.0, 8.0, 0.0))
}
