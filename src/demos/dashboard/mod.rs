//! Ant Design 5 全组件展示 — 分类多页面版。
//!
//! 运行：`cargo run --bin uix-demo`
pub mod more_pages;
pub mod pages_extra;
pub mod sections;
pub mod widgets;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use uix::app::{map_ui_event, App};
use uix::base::{EdgeInsets, KeyCode, Rect};
use uix::graphics::{AlignItems, Color, FlexDirection, GraphicsEngine};
use uix::platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix::tree;
use uix::ui::theme::{DesignTokens, DynTokens, Theme};
use uix::ui::widget::{WidgetCore, WidgetNode, WidgetTree};
use uix::ui::widgets::icon::init_lucide_font;
use uix::ui::{
    Card, Container, Icon, IntoWidgetNode, Label, Navigation, ScrollDirection,
    ScrollView, SharedActive, Space, SpaceSize,
};

use more_pages::{page_breadcrumb, page_nav, page_tabs};
use pages_extra::{page_colors, page_custom, page_layout};
use sections::{
    page_buttons, page_dashboard, page_data_display, page_feedback, page_inputs, page_typography,
};
use widgets::ThemeToggle;

// ════════════════════════════════════════════════════════════════════════════
// 布局常量
// ════════════════════════════════════════════════════════════════════════════

const INIT_W: i32 = 1200;
const INIT_H: i32 = 800;
const SIDEBAR_W: f32 = 200.0;
pub const CONTENT_W: f32 = (INIT_W as f32) - SIDEBAR_W;
const CONTENT_PAD_H: f32 = 40.0;
pub const INNER_W: f32 = CONTENT_W - CONTENT_PAD_H;
const STAT_CARD_GAP: f32 = 8.0;
const STAT_CARD_H: f32 = 100.0;

pub const PAGE_TITLES: &[(&str, &str)] = &[
    ("chart-bar", " 仪表盘"), ("type", " 排版"), ("square", " 按钮"),
    ("edit", " 输入与选择"), ("table", " 数据展示"), ("alert-circle", " 反馈"),
    ("menu", " 导航"), ("layout", " 标签页"), ("list", " 面包屑"),
    ("grid", " 布局"), ("palette", " 主题色"), ("settings", " 自定义组件"),
];

// ════════════════════════════════════════════════════════════════════════════
// PageBuilder — 声明式页面构建器
// ════════════════════════════════════════════════════════════════════════════

/// 页面构建器，用链式调用替代重复的 `Vec::new()` / `push` / `into_node` 模式。
///
/// # 用法
/// ```ignore
/// PageBuilder::new(tk)
///     .gap()                         // 顶部 12px 间距
///     .section("标题")               // 子章节标题
///     .push(some_widget)             // 内容
///     .build()                       // → WidgetNode
/// ```
pub struct PageBuilder<'a> {
    tk: &'a DesignTokens,
    items: Vec<WidgetNode>,
}

impl<'a> PageBuilder<'a> {
    pub fn new(tk: &'a DesignTokens) -> Self {
        Self { tk, items: Vec::new() }
    }

    /// 添加标准顶部间距（12px 占位 Space）。
    pub fn gap(mut self) -> Self {
        self.items.push(space_h(12.0));
        self
    }

    /// 添加子章节标题（12px 灰色小字）。
    pub fn section(mut self, title: &str) -> Self {
        self.items.push(section_title(self.tk, title));
        self
    }

    /// 添加任意 widget node。
    pub fn push(mut self, node: impl IntoWidgetNode) -> Self {
        self.items.push(node.into_node());
        self
    }

    /// 构建最终页面（包装 ScrollView + flex-grow 占位）。
    pub fn build(self) -> WidgetNode {
        let mut children = self.items;
        children.push(Container::new().flex_grow(1.0).into_node());
        WidgetNode::new(
            Box::new(ScrollView::new(ScrollDirection::Both).flex_grow(1.0)),
            vec![WidgetNode::new(
                Box::new(
                    Container::new()
                        .size(INNER_W, 0.0)
                        .dir(FlexDirection::Column)
                        .gap(8.0)
                        .pad(EdgeInsets::new(8.0, 4.0, 16.0, 10.0)),
                ),
                children,
            )],
        )
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 辅助构建函数
// ════════════════════════════════════════════════════════════════════════════

/// 子章节标题（12px 灰色）。
pub fn section_title(tk: &DesignTokens, text: &str) -> WidgetNode {
    Label::new(text)
        .color(tk.color_text_secondary)
        .font_size(12.0)
        .into_node()
}

/// 标准行容器（INNER_W 宽，Row 方向，居中对齐）。
pub fn row(h: f32) -> Space {
    Space::new()
        .size(SpaceSize::Small)
        .width(INNER_W)
        .height(h)
        .direction(FlexDirection::Row)
        .align(AlignItems::Center)
}

/// 标准列容器（INNER_W 宽，Column 方向，拉伸对齐）。
pub fn col(h: f32) -> Space {
    Space::new()
        .size(SpaceSize::Small)
        .width(INNER_W)
        .height(h)
        .direction(FlexDirection::Column)
        .align(AlignItems::Stretch)
}

/// 固定高度水平占位 Space（INNER_W 宽）。
fn space_h(h: f32) -> WidgetNode {
    Space::new()
        .size(SpaceSize::Small)
        .width(INNER_W)
        .height(h)
        .into_node()
}

/// 页面标题行（图标 + 标签）。
pub fn page_title(tk: &DesignTokens, icon: &str, label: &str) -> WidgetNode {
    tree! { Container::new().dir(FlexDirection::Row) => [
        Icon::new(icon).size(22.0),
        Container::new().size(8.0, 0.0),
        Label::new(label).color(tk.color_text).font_size(22.0),
    ]}
    .into_node()
}

/// 统计卡片。
pub fn stat_card(tk: &DesignTokens, title: &str, value: &str, color: Color, elev: u8) -> Card {
    let card_w = (INNER_W - STAT_CARD_GAP * 3.0) / 4.0;
    Card::new()
        .title(title)
        .elevation(elev)
        .hoverable()
        .size(card_w, STAT_CARD_H)
        .child(Label::new(value).color(color).font_size(26.0))
        .child(Label::new(title).color(tk.color_text_quaternary).font_size(11.0))
}

// ════════════════════════════════════════════════════════════════════════════
// 页面调度
// ════════════════════════════════════════════════════════════════════════════

fn build_page(page_index: usize, tk: &DesignTokens) -> WidgetNode {
    match page_index {
        0 => page_dashboard(tk),
        1 => page_typography(tk),
        2 => page_buttons(tk),
        3 => page_inputs(tk),
        4 => page_data_display(tk),
        5 => page_feedback(tk),
        6 => page_nav(tk),
        7 => page_tabs(tk),
        8 => page_breadcrumb(tk),
        9 => page_layout(tk),
        10 => page_colors(tk),
        11 => page_custom(tk),
        _ => page_dashboard(tk),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 树构建与重建
// ════════════════════════════════════════════════════════════════════════════

fn build_demo_tree(tk: &DesignTokens, active_page: usize) -> (WidgetNode, SharedActive) {
    let nav = Navigation::new("UIX 组件")
        .item(" 仪表盘", "chart-bar")
        .item(" 排版", "type")
        .item(" 按钮", "square")
        .item(" 输入与选择", "edit")
        .item(" 数据展示", "table")
        .item(" 反馈", "alert-circle")
        .item(" 导航", "menu")
        .item(" 标签页", "layout")
        .item(" 面包屑", "list")
        .item(" 布局", "grid")
        .item(" 主题色", "palette")
        .item(" 自定义", "settings")
        .active_index(active_page)
        .width(SIDEBAR_W);
    let nav_active = nav.active().clone();
    let nav_node = nav.build(tk);
    let (icon, label) = PAGE_TITLES[active_page.min(11)];
    let title_node = page_title(tk, icon, label);
    let page_content = build_page(active_page, tk);

    let content_container = WidgetNode::new(
        Box::new(
            Container::new()
                .bg(tk.color_bg_container)
                .dir(FlexDirection::Column)
                .flex_grow(1.0),
        ),
        vec![
            tree! { Container::new().bg(tk.color_bg_elevated).dir(FlexDirection::Row)
                .size(0.0, 44.0) => [
                Label::new("  UIX 组件库").color(tk.color_text)
                    .font_size(16.0).size(400.0, 44.0),
                Container::new().flex_grow(1.0),
                ThemeToggle::new(),
                Label::new("UIX v0.1.0").color(tk.color_text_quaternary)
                    .font_size(12.0).size(100.0, 44.0),
            ]},
            title_node,
            page_content,
        ],
    );

    let root = tree! {
        Container::new().flex_grow(1.0).bg(tk.color_bg_layout).dir(FlexDirection::Row) => [
            nav_node,
            content_container,
        ]
    };
    (root, nav_active)
}

fn rebuild_tree(
    tree: &mut WidgetTree,
    eng: &mut dyn GraphicsEngine,
    dyn_tokens: &DynTokens,
    dark_mode: &Cell<bool>,
    page_index: usize,
) -> SharedActive {
    let tk = dyn_tokens.snapshot();
    let (new_root, new_active) = build_demo_tree(&tk, page_index);
    tree.build(new_root);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, eng.width() as f32, eng.height() as f32));
    }
    tree.layout();
    tree.find_by_type_and_modify::<ThemeToggle>(|w| {
        w.dark.set(dark_mode.get());
    });
    tree.mark_full_frame_dirty();
    new_active
}

// ════════════════════════════════════════════════════════════════════════════
// 运行时状态
// ════════════════════════════════════════════════════════════════════════════

struct DemoState {
    prev_active: Cell<usize>,
    dark_mode: Cell<bool>,
    nav_active: Rc<std::cell::RefCell<SharedActive>>,
}

impl DemoState {
    fn new(nav_active: SharedActive) -> Self {
        Self {
            prev_active: Cell::new(0),
            dark_mode: Cell::new(false),
            nav_active: Rc::new(std::cell::RefCell::new(nav_active)),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// GUI 入口
// ════════════════════════════════════════════════════════════════════════════

/// 运行主事件循环。
fn run_event_loop(
    app: &mut App,
    engine: &mut Box<dyn GraphicsEngine>,
    tree: &mut WidgetTree,
    state: DemoState,
    dyn_tokens: Arc<DynTokens>,
    theme_cell: &std::cell::RefCell<Theme>,
) -> i32 {
    app.run_widget_with_tokens(
        &mut **engine,
        tree,
        INIT_W,
        INIT_H,
        theme_cell,
        map_ui_event,
        |ev: &UiEvent| -> bool {
            matches!(
                ev,
                UiEvent {
                    type_: UiEventType::KeyDown,
                    payload: UiEventPayload::Key(ref d),
                    ..
                } if d.key == KeyCode::Escape
            )
        },
        move |tree, eng, _platform| {
            let mut new_dark = false;
            tree.find_by_type_and_modify::<ThemeToggle>(|w| new_dark = w.dark.get());

            if new_dark != state.dark_mode.get() {
                state.dark_mode.set(new_dark);
                dyn_tokens.set_mode(new_dark);
                let a = rebuild_tree(tree, eng, &dyn_tokens, &state.dark_mode, state.prev_active.get());
                *state.nav_active.borrow_mut() = a;
            }

            let active = state.nav_active.borrow().get();
            if active != state.prev_active.get() {
                state.prev_active.set(active);
                let a = rebuild_tree(tree, eng, &dyn_tokens, &state.dark_mode, active);
                *state.nav_active.borrow_mut() = a;
            }
        },
    )
}

/// 运行 GUI 演示的主入口。
pub fn run_gui_demo() {
    let tk = DesignTokens::antd_light();
    let (root_node, nav_active) = build_demo_tree(&tk, 0);
    let mut tree = WidgetTree::new();
    tree.build(root_node);

    let state = DemoState::new(nav_active);
    let dyn_tokens = Arc::new(DynTokens::new(DesignTokens::antd_light()));
    let theme_cell = std::cell::RefCell::new(Theme::from_arc(dyn_tokens.clone()));

    let mut app = App::new();
    app.title("UIX — 组件库");

    // 创建引擎并加载 Lucide 字体
    let Some(mut engine) = app.take_engine(INIT_W, INIT_H) else {
        return;
    };
    if let Ok(ttf) = std::fs::read("assets/fonts/lucide.ttf") {
        init_lucide_font(&ttf, &mut *engine);
    } else {
        log::warn!("Lucide font not found — icons will be blank");
    }

    let exit_code = run_event_loop(&mut app, &mut engine, &mut tree, state, dyn_tokens, &theme_cell);
    engine.shutdown();
    std::process::exit(exit_code);
}

// ════════════════════════════════════════════════════════════════════════════
// 测试
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_constants() {
        assert_eq!(CONTENT_W, 1000.0);
        assert_eq!(INNER_W, 960.0);
        assert_eq!((INNER_W - STAT_CARD_GAP * 3.0) / 4.0, 234.0);
    }

    #[test]
    fn page_titles_and_build() {
        assert_eq!(PAGE_TITLES.len(), 12);
        let tk = DesignTokens::antd_light();
        for i in 0..PAGE_TITLES.len() {
            let _node = build_page(i, &tk);
        }
    }
}
