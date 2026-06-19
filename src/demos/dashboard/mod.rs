//! Ant Design 5 全组件展示 — 分类多页面版。
//!
//! 每个组件分类独立成页，左侧导航切换页面，不再挤在一页内滚动。
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
use uix::base::KeyCode;
use uix::base::{EdgeInsets, Rect};
use uix::graphics::Color;
use uix::graphics::{AlignItems, FlexDirection, GraphicsEngine};
use uix::platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix::tree;
use uix::ui::theme::{DesignTokens, DynTokens, Theme};
use uix::ui::widget::{WidgetCore, WidgetNode, WidgetTree};
use uix::ui::widgets::icon::init_lucide_font;
use uix::ui::{
    Card, Container, Icon, IntoWidgetNode, Label, Navigation, ScrollDirection, ScrollView,
    SharedActive, Space, SpaceSize,
};

use more_pages::{page_breadcrumb, page_nav, page_tabs};
use pages_extra::{page_colors, page_custom, page_layout};
use sections::{
    page_buttons, page_dashboard, page_data_display, page_feedback, page_inputs, page_typography,
};
use widgets::ThemeToggle;

/// 窗口初始尺寸（引擎缓冲和窗口创建时的参考值，非硬编码给 widget 布局）。
/// widget 布局由 flex-grow/stretch 等布局系统决定，不使用这些常量。
const INIT_W: i32 = 1200;
const INIT_H: i32 = 800;
/// 侧边栏固定宽度。
const SIDEBAR_W: f32 = 200.0;
/// 内容区参考宽度（窗口初始宽度 - 侧边栏宽度）。
pub const CONTENT_W: f32 = (INIT_W as f32) - SIDEBAR_W;
/// 内容区水平内边距预留（左右合计，用于组件首次构建时的尺寸参考）。
const CONTENT_PAD_H: f32 = 40.0;
/// 内容区内边距后的可用宽度（用于组件首次构建时的尺寸参考）。
pub const INNER_W: f32 = CONTENT_W - CONTENT_PAD_H;
/// 统计卡片之间的间距。
const STAT_CARD_GAP: f32 = 8.0;
/// 统计卡片行高。
const STAT_CARD_H: f32 = 100.0;

/// 每页的标题信息（图标名, 标签文本）。
pub const PAGE_TITLES: &[(&str, &str)] = &[
    ("chart-bar", " 仪表盘"),
    ("type", " 排版"),
    ("square", " 按钮"),
    ("edit", " 输入与选择"),
    ("table", " 数据展示"),
    ("alert-circle", " 反馈"),
    ("menu", " 导航"),
    ("layout", " 标签页"),
    ("list", " 面包屑"),
    ("grid", " 布局"),
    ("palette", " 主题色"),
    ("settings", " 自定义组件"),
];

// ── 辅助构建函数 ──

pub fn page_title(tk: &DesignTokens, icon: &str, label: &str) -> WidgetNode {
    tree! { Container::new().dir(FlexDirection::Row) => [
        Icon::new(icon).size(22.0),
        Container::new().size(8.0, 0.0),
        Label::new(label).color(tk.color_text).font_size(22.0),
    ]}
    .into_node()
}
pub fn sub(tk: &DesignTokens, text: &str) -> Label {
    Label::new(text)
        .color(tk.color_text_secondary)
        .font_size(12.0)
}
pub fn row(h: f32) -> Space {
    Space::new()
        .size(SpaceSize::Small)
        .width(INNER_W)
        .height(h)
        .direction(FlexDirection::Row)
        .align(AlignItems::Center)
}
pub fn col(h: f32) -> Space {
    Space::new()
        .size(SpaceSize::Small)
        .width(INNER_W)
        .height(h)
        .direction(FlexDirection::Column)
        .align(AlignItems::Stretch)
}

/// 统计卡片
pub fn stat_card(tk: &DesignTokens, title: &str, value: &str, color: Color, elev: u8) -> Card {
    let card_w = (INNER_W - STAT_CARD_GAP * 3.0) / 4.0;
    Card::new()
        .title(title)
        .elevation(elev)
        .hoverable()
        .size(card_w, STAT_CARD_H)
        .child(Label::new(value).color(color).font_size(26.0))
        .child(
            Label::new(title)
                .color(tk.color_text_quaternary)
                .font_size(11.0),
        )
}

// ── 自定义 widget 定义（移入 widgets.rs）──

// ── 页面包装工具 ──

/// 把内容列表包装进 ScrollView（不含 page_title，title 由 build_demo_tree 在外部添加）
/// 末尾添加 flex-grow 占位容器，确保页面内容占满 ScrollView 可视高度。
pub fn wrap_page(content: Vec<WidgetNode>) -> WidgetNode {
    const GAP: f32 = 8.0;
    let mut children = content;
    // 末尾占位：flex-grow 占满 Column 剩余空间，短页面底部不空
    children.push(Container::new().flex_grow(1.0).into_node());
    WidgetNode::new(
        Box::new(ScrollView::new(ScrollDirection::Both).flex_grow(1.0)),
        vec![WidgetNode::new(
            Box::new(
                Container::new()
                    .size(INNER_W, 0.0)
                    .dir(FlexDirection::Column)
                    .gap(GAP)
                    .pad(EdgeInsets::new(8.0, 4.0, 16.0, 10.0)),
            ),
            children,
        )],
    )
}

// 页面调度 — 根据 nav_active 索引返回对应页面内容（不含 page_title）
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
// UI 组装
// ════════════════════════════════════════════════════════════════════════════

/// 根据当前状态重建 widget tree，同步 nav_active 和 ThemeToggle 状态。
///
/// 返回新的 `SharedActive`（调用方需要将其写入 nav_active 变量）。
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
    // 重建后恢复 root frame 为实际窗口尺寸
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, eng.width() as f32, eng.height() as f32));
    }
    tree.layout();
    // 同步 ThemeToggle 状态到重建后的树
    tree.find_by_type_and_modify::<ThemeToggle>(|w| {
        w.dark.set(dark_mode.get());
    });
    tree.mark_full_frame_dirty();
    new_active
}

/// 构建完整的 demo widget tree（侧边栏 + 内容区 + 页面标题）。
///
/// 页面标题固定在内容区顶部（ScrollView 外部），不受滚动影响。
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
    // 页面标题固定在 ScrollView 外部
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
            // Top bar
            tree! { Container::new().bg(tk.color_bg_elevated).dir(FlexDirection::Row)
                .size(0.0, 44.0) => [
                Label::new("  UIX 组件库").color(tk.color_text)
                    .font_size(16.0).size(400.0, 44.0),
                Container::new().flex_grow(1.0),
                ThemeToggle::new(),
                Label::new("UIX v0.1.0").color(tk.color_text_quaternary)
                    .font_size(12.0).size(100.0, 44.0),
            ]},
            // 页面标题（固定在顶部，不受 ScrollView 滚动影响）
            title_node,
            // 页面内容（可滚动）
            page_content,
        ],
    );

    let root = tree! {
        // Root 不使用固定 size()，由 flex_grow(1.0) 填满引擎缓冲
        Container::new().flex_grow(1.0).bg(tk.color_bg_layout).dir(FlexDirection::Row) => [
            nav_node,
            content_container,
        ]
    };
    (root, nav_active)
}

/// 运行 GUI 演示的主入口。
pub fn run_gui_demo() {
    let tk = DesignTokens::antd_light();
    let (root_node, nav_active) = build_demo_tree(&tk, 0);
    let mut tree = WidgetTree::new();
    tree.build(root_node);

    // 统一的应用运行时状态
    let state = DemoState::new(nav_active);

    // 使用 DynTokens 实现运行时主题切换
    let dyn_tokens = Arc::new(DynTokens::new(DesignTokens::antd_light()));
    let mut app = App::new();
    app.title("UIX — 组件库");

    let mut engine = match app.take_engine(INIT_W, INIT_H) {
        Some(e) => e,
        None => return,
    };

    // 加载 Lucide 图标字体（通过 GraphicsEngine trait 的 load_font 方法）
    if let Ok(ttf) = std::fs::read("assets/fonts/lucide.ttf") {
        init_lucide_font(&ttf, &mut *engine);
    } else {
        log::warn!("Lucide font not found at assets/fonts/lucide.ttf — icons will be blank");
    }

    let theme_cell = std::cell::RefCell::new(Theme::from_arc(dyn_tokens.clone()));
    let tc_ref = &theme_cell;

    let exit_code = app.run_widget_with_tokens(
        &mut *engine,
        &mut tree,
        INIT_W,
        INIT_H,
        tc_ref,
        map_ui_event,
        |ev: &UiEvent| -> bool {
            if let UiEventType::KeyDown = ev.type_ {
                if let UiEventPayload::Key(ref d) = ev.payload {
                    return d.key == KeyCode::Escape;
                }
            }
            false
        },
        move |tree, eng, _platform| {
            // ── 主题切换 ──
            let mut new_dark = false;
            tree.find_by_type_and_modify::<ThemeToggle>(|w| {
                new_dark = w.dark.get();
            });
            if new_dark != state.dark_mode.get() {
                state.dark_mode.set(new_dark);
                dyn_tokens.set_mode(new_dark);
                let new_active = rebuild_tree(
                    tree, eng, &dyn_tokens, &state.dark_mode, state.prev_active.get(),
                );
                *state.nav_active.borrow_mut() = new_active;
            }

            // ── 导航切换 → 重建内容页 ──
            let active = state.nav_active.borrow().get();
            if active != state.prev_active.get() {
                state.prev_active.set(active);
                let new_active = rebuild_tree(
                    tree, eng, &dyn_tokens, &state.dark_mode, active,
                );
                *state.nav_active.borrow_mut() = new_active;
            }
        },
    );
    
    engine.shutdown();
    std::process::exit(exit_code);
}

// ════════════════════════════════════════════════════════════════════════════
// 运行时状态
// ════════════════════════════════════════════════════════════════════════════

/// Demo 应用的运行时状态集合。
///
/// 将分散的 `Cell`/`RefCell` 统一到单一结构体中，
/// 替代原先散落在 `run_gui_demo` 中的 4 个独立状态变量。
struct DemoState {
    /// 上一次激活的页面索引（检测导航变化）。
    prev_active: Cell<usize>,
    /// 当前是否为暗色模式。
    dark_mode: Cell<bool>,
    /// 导航栏的共享激活状态引用。
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
// 测试
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证布局常量的计算一致性。
    #[test]
    fn test_layout_constants() {
        // CONTENT_W = INIT_W - SIDEBAR_W = 1200 - 200 = 1000
        assert_eq!(CONTENT_W, 1000.0);
        // INNER_W = CONTENT_W - CONTENT_PAD_H = 1000 - 40 = 960
        assert_eq!(INNER_W, 960.0);
    }

    /// 验证统计卡片宽度计算正确（4 列 + 3 个间隙）。
    #[test]
    fn test_stat_card_width_calculation() {
        let expected_w = (INNER_W - STAT_CARD_GAP * 3.0) / 4.0;
        // 960 - 24 = 936, / 4 = 234
        assert_eq!(expected_w, 234.0);
    }

    /// 验证 PAGE_TITLES 的索引安全性。
    #[test]
    fn test_page_titles_len() {
        assert_eq!(PAGE_TITLES.len(), 12);
    }

    /// 验证所有页面索引在 build_page 中都有对应分支。
    #[test]
    fn test_build_page_all_indices() {
        let tk = DesignTokens::antd_light();
        for i in 0..PAGE_TITLES.len() {
            let _node = build_page(i, &tk);
        }
    }
}
