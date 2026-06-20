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
use uix::graphics::GraphicsEngine;
use uix::ui::layout::{AlignItems, FlexDirection};
use uix::platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix::tree;
use uix::ui::theme::{DesignTokens, DynTokens, Theme};
use uix::ui::widget::{WidgetCore, WidgetNode, WidgetTree};
use uix::ui::widgets::icon::init_lucide_font;
use uix::ui::{
    Container, Icon, IntoWidgetNode, Label, Navigation, ScrollDirection,
    ScrollView, SharedActive, Space, SpaceSize,
};

use more_pages::{page_nav, page_tabs};
use pages_extra::{page_colors, page_custom, page_layout};
use sections::{
    page_buttons, page_data_display, page_date_picker, page_feedback, page_inputs, page_typography,
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


pub const PAGE_TITLES: &[(&str, &str)] = &[
    ("type", " 排版"), ("square", " 按钮"),
    ("edit", " 输入"), ("calendar", " 日期"),
    ("table", " 数据展示"), ("alert-circle", " 反馈"),
    ("menu", " 导航"), ("layout", " 标签页"),
    ("grid", " 布局"), ("palette", " 主题色"), ("settings", " 自定义"),
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
    /// 内部容器使用 flex_grow(1.0) 而非固定 INNER_W，实现响应式自适应。
    pub fn build(self) -> WidgetNode {
        let mut children = self.items;
        children.push(Container::new().flex_grow(1.0).into_node());
        WidgetNode::new(
            Box::new(ScrollView::new(ScrollDirection::Both).flex_grow(1.0)),
            vec![WidgetNode::new(
                Box::new(
                    Container::new()
                        .flex_grow(1.0)
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

/// 标准行容器（Row 方向，居中对齐，宽度由父容器 Stretch 自动拉伸）。
pub fn row(h: f32) -> Space {
    Space::new()
        .size(SpaceSize::Small)
        .height(h)
        .direction(FlexDirection::Row)
        .align(AlignItems::Center)
}

/// 标准列容器（Column 方向，拉伸对齐，宽度由父容器 Stretch 自动拉伸）。
pub fn col(h: f32) -> Space {
    Space::new()
        .size(SpaceSize::Small)
        .height(h)
        .direction(FlexDirection::Column)
        .align(AlignItems::Stretch)
}

/// 固定高度水平占位 Space（宽度由父容器 Stretch 自动拉伸）。
fn space_h(h: f32) -> WidgetNode {
    Space::new()
        .size(SpaceSize::Small)
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

/// 统计卡片（flex-grow 响应式宽度，适合放在 Row Space 中均匀分布）。


/// 构建顶部标题栏（"UIX 组件库" + ThemeToggle + 版本号）。
fn build_header_bar(tk: &DesignTokens) -> WidgetNode {
    tree! { Container::new().bg(tk.color_bg_elevated).dir(FlexDirection::Row)
        .size(0.0, 44.0) => [
        Label::new("  UIX 组件库").color(tk.color_text)
            .font_size(16.0).size(400.0, 44.0),
        Container::new().flex_grow(1.0),
        ThemeToggle::new(),
        Label::new("UIX v0.1.0").color(tk.color_text_quaternary)
            .font_size(12.0).size(100.0, 44.0),
    ]}
    .into_node()
}

// ════════════════════════════════════════════════════════════════════════════
// 页面调度
// ════════════════════════════════════════════════════════════════════════════

fn build_page(page_index: usize, tk: &DesignTokens) -> WidgetNode {
    match page_index {
        0 => page_typography(tk),
        1 => page_buttons(tk),
        2 => page_inputs(tk),
        3 => page_date_picker(tk),
        4 => page_data_display(tk),
        5 => page_feedback(tk),
        6 => page_nav(tk),
        7 => page_tabs(tk),
        8 => page_layout(tk),
        9 => page_colors(tk),
        10 => page_custom(tk),
        _ => page_typography(tk),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 树构建 — 基础框架 + 页面内容分步构建
// ════════════════════════════════════════════════════════════════════════════

/// 构建完整的 demo widget tree（侧边栏 + 内容区 + 页面标题）。
/// 每次导航切换或主题变更时重建整棵树，确保 NavItem 的 SharedActive 连接正确。
///
/// 树结构：
/// ```
/// root (Row, flex-grow)
/// ├── nav_container (Column, 200px, bg_elevated)
/// │   ├── Title "UIX 组件"
/// │   ├── Divider
/// │   ├── NavItem[0..10]
/// │   ├── Spacer
/// │   └── Version label
/// └── content_container (Column, flex-grow, bg_container)
///     ├── header_bar (Row, 44px) — "UIX 组件库" + ThemeToggle + 版本号
///     ├── title_node — 当前页面标题（图标 + 标签）
///     └── page_content — ScrollView 包裹的页面内容
/// ```
fn build_demo_tree(tk: &DesignTokens, active_page: usize) -> (WidgetNode, SharedActive) {
    let nav = Navigation::new("UIX 组件")
        .item(" 排版", "type")
        .item(" 按钮", "square")
        .item(" 输入", "edit")
        .item(" 日期", "calendar")
        .item(" 数据展示", "table")
        .item(" 反馈", "alert-circle")
        .item(" 导航", "menu")
        .item(" 标签页", "layout")
        .item(" 布局", "grid")
        .item(" 主题色", "palette")
        .item(" 自定义", "settings")
        .active_index(active_page)
        .width(SIDEBAR_W);
    let nav_active = nav.active().clone();
    let nav_node = nav.build(tk);

    let (icon, label) = PAGE_TITLES[active_page.min(PAGE_TITLES.len() - 1)];
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
            build_header_bar(tk),
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

/// 重建整棵 widget 树，保持主题/导航/页面同步。
fn rebuild_tree(
    tree: &mut WidgetTree,
    eng: &mut dyn GraphicsEngine,
    dyn_tokens: &DynTokens,
    dark_mode: bool,
    page_index: usize,
) -> SharedActive {
    eprintln!("[TRACE:L2] rebuild_tree: page={} dark={}", page_index, dark_mode);
    let tk = dyn_tokens.snapshot();
    let (new_root, new_active) = build_demo_tree(&tk, page_index);
    eprintln!("[TRACE:L2]   build_demo_tree done, new_active.get()={}", new_active.get());
    tree.build(new_root);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, eng.width() as f32, eng.height() as f32));
    }
    tree.layout();
    tree.find_by_type_and_modify::<ThemeToggle>(|w| {
        w.dark.set(dark_mode);
    });
    tree.mark_full_frame_dirty();
    eprintln!("[TRACE:L2]   rebuild complete");
    new_active
}

/// 运行主事件循环。
fn run_event_loop(
    app: &mut App,
    engine: &mut Box<dyn GraphicsEngine>,
    tree: &mut WidgetTree,
    state: DemoState,
    dyn_tokens: Arc<DynTokens>,
    theme_cell: &std::cell::RefCell<Theme>,
) -> i32 {
    let on_exit = |ev: &UiEvent| -> bool {
        matches!(
            ev,
            UiEvent {
                type_: UiEventType::KeyDown,
                payload: UiEventPayload::Key(ref d),
                ..
            } if d.key == KeyCode::Escape
        )
    };

    app.run_widget_with_tokens(
        &mut **engine,
        tree,
        INIT_W,
        INIT_H,
        theme_cell,
        map_ui_event,
        on_exit,
        move |tree, eng, _platform| {
            // ── 主题切换 → 重建整棵树 ──
            let mut new_dark = false;
            tree.find_by_type_and_modify::<ThemeToggle>(|w| new_dark = w.dark.get());

            if new_dark != state.dark_mode.get() {
                eprintln!("[TRACE:L2] on_frame: THEME CHANGE dark={}", new_dark);
                state.dark_mode.set(new_dark);
                dyn_tokens.set_mode(new_dark);
                let a = rebuild_tree(tree, eng, &dyn_tokens, new_dark, state.prev_active.get());
                *state.nav_active.borrow_mut() = a;
                eprintln!("[TRACE:L2] on_frame: theme done, nav_active={}", state.nav_active.borrow().get());
                return;
            }

            // ── 导航切换 → 重建整棵树 ──
            let active = state.nav_active.borrow().get();
            eprintln!("[TRACE:L2] on_frame: nav_active={} prev_active={}", active, state.prev_active.get());
            if active != state.prev_active.get() {
                eprintln!("[TRACE:L2] on_frame: NAV CHANGE {} -> {}", state.prev_active.get(), active);
                state.prev_active.set(active);
                let a = rebuild_tree(tree, eng, &dyn_tokens, state.dark_mode.get(), active);
                *state.nav_active.borrow_mut() = a;
                eprintln!("[TRACE:L2] on_frame: nav done");
            }
        },
    )
}

/// 运行 GUI 演示的主入口。
pub fn run_gui_demo() {
    let tk = DesignTokens::antd_light();

    // 构建完整 demo 树（导航栏 + 标题栏 + 第 0 页内容）
    let (root_node, nav_active) = build_demo_tree(&tk, 0);
    let mut tree = WidgetTree::new();
    tree.build(root_node);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }
    tree.layout();
    tree.mark_full_frame_dirty();

    let state = DemoState::new(nav_active);
    let dyn_tokens = Arc::new(DynTokens::new(tk));
    let theme_cell = std::cell::RefCell::new(Theme::from_arc(dyn_tokens.clone()));

    let mut app = App::new();
    app.title("UIX — 组件库");

    let Some(mut engine) = app.take_engine(INIT_W, INIT_H) else {
        return;
    };
    if let Ok(ttf) = std::fs::read("assets/fonts/lucide.ttf") {
        init_lucide_font(&ttf, &mut *engine);
    } else {
        log::warn!("Lucide font not found — icons will be blank");
    }

    // 首帧后打印内存诊断
    engine.diagnose_memory();

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
    }

    #[test]
    fn page_titles_and_build() {
        assert_eq!(PAGE_TITLES.len(), 11);
        let tk = DesignTokens::antd_light();
        for i in 0..PAGE_TITLES.len() {
            let _node = build_page(i, &tk);
        }
    }
}
