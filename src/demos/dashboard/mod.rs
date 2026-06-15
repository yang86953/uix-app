//! Ant Design 5 全组件展示 — 分类多页面版。
//!
//! 每个组件分类独立成页，左侧导航切换页面，不再挤在一页内滚动。
//!
//! 运行：`cargo run --bin uix-demo`

pub mod widgets;
pub mod sections;
pub mod more_pages;

use std::sync::Arc;
use uix::app::{map_ui_event, App};
use uix::base::{EdgeInsets, Rect};
use uix::graphics::Color;
use uix::platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix::platform::types::KeyCode;
use uix::ui::theme::{DesignTokens, DynTokens, Theme};
use uix::ui::widget::{WidgetCore, WidgetNode, WidgetTree};
use uix::ui::widgets::icon::init_lucide_font;
use uix::ui::{
    AlignItems, Card, Container, FlexDirection, Icon, IntoWidgetNode, Label, Navigation,
    ScrollDirection, ScrollView, SharedActive, Space, SpaceSize,
};
use uix::tree;

use widgets::{ThemeToggle};
use sections::{
    page_dashboard, page_typography, page_buttons, page_inputs, page_data_display, page_feedback,
};
use more_pages::{
    page_nav, page_tabs, page_breadcrumb, page_layout, page_colors, page_custom,
};

const GW: i32 = 1100;
const GH: i32 = 740;
const SB: f32 = 200.0;         // sidebar width
pub fn cw() -> f32 { GW as f32 - SB }
pub fn iw() -> f32 { cw() - 40.0 }

/// 每页的标题信息（图标名, 标签文本）。
pub const PAGE_TITLES: &[(&str, &str)] = &[
    ("chart-bar",    " 仪表盘"),
    ("type",         " 排版"),
    ("square",       " 按钮"),
    ("edit",         " 输入与选择"),
    ("table",        " 数据展示"),
    ("alert-circle", " 反馈"),
    ("menu",         " 导航"),
    ("layout",       " 标签页"),
    ("list",         " 面包屑"),
    ("grid",         " 布局"),
    ("palette",      " 主题色"),
    ("settings",     " 自定义组件"),
];

// ── 辅助构建函数 ──

pub fn page_title(tk: &DesignTokens, icon: &str, label: &str) -> WidgetNode {
    tree! { Container::new().dir(FlexDirection::Row) => [
        Icon::new(icon).size(22.0),
        Container::new().size(8.0, 0.0),
        Label::new(label).color(tk.color_text).font_size(22.0),
    ]}.into_node()
}
pub fn sub(tk: &DesignTokens, text: &str) -> Label {
    Label::new(text).color(tk.color_text_secondary).font_size(12.0)
}
pub fn row(h: f32) -> Space {
    Space::new().size(SpaceSize::Small).width(iw()).height(h)
        .direction(FlexDirection::Row).align(AlignItems::Center)
}
pub fn col(h: f32) -> Space {
    Space::new().size(SpaceSize::Small).width(iw()).height(h)
        .direction(FlexDirection::Column).align(AlignItems::Stretch)
}

/// 统计卡片
pub fn stat_card(tk: &DesignTokens, title: &str, value: &str, color: Color, elev: u8) -> Card {
    let w = (iw() - 24.0) / 4.0;
    Card::new().title(title).elevation(elev).hoverable()
        .size(w, 100.0)
        .child(Label::new(value).color(color).font_size(26.0))
        .child(Label::new(title).color(tk.color_text_quaternary).font_size(11.0))
}

// ── 自定义 widget 定义（移入 widgets.rs）──

// ── 页面包装工具 ──

/// 把内容列表包装进 ScrollView（不含 page_title，title 由 build_demo_tree 在外部添加）
/// Container 高度根据子 widget 的 preferred_size 动态计算，
/// 而非固定值，使 ScrollView 能正确判断内容是否溢出。
pub fn wrap_page(_tk: &DesignTokens, content: Vec<WidgetNode>) -> WidgetNode {
    const GAP: f32 = 8.0;
    let n = content.len() as f32;
    let total_gap = GAP * (n - 1.0).max(0.0);
    // 计算内容所需高度（子 widget 高度总和 + gap 总和 + padding）
    let content_h = content_height(&content) + total_gap + 24.0; // 8px top + 16px bottom padding
    WidgetNode::new(
            Box::new(ScrollView::new(ScrollDirection::Both).flex_grow(1.0)),
        vec![WidgetNode::new(
            Box::new(Container::new()
                  .size(iw(), content_h.max(100.0))
                .dir(FlexDirection::Column)
                .gap(GAP)
                .pad(EdgeInsets::new(8.0, 4.0, 16.0, 10.0))),
            content,
        )],
    )
}

/// 递归计算 widget 节点树的 preferred 高度总和（按 flex column 排列）
fn content_height(nodes: &[WidgetNode]) -> f32 {
    nodes.iter().map(|node| {
        let pref = node.widget.preferred_size(None);
        if pref.h > 0.0 {
            pref.h
        } else if !node.children.is_empty() {
            // 自身未指定高度时，尝试递归子节点
            content_height(&node.children)
        } else {
            // 既无高度又无子节点，用保底值
            20.0
        }
    }).sum::<f32>()
}

// ════════════════════════════════════════════════════════════════════════════
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

/// 构建完整的 demo widget tree（侧边栏 + 内容区 + 页面标题）。
///
/// 页面标题固定在内容区顶部（ScrollView 外部），不受滚动影响。
fn build_demo_tree(tk: &DesignTokens, nav_active: &SharedActive) -> WidgetNode {
    let nav = Navigation::new("UIX 组件")
        .shared_active(nav_active.clone())
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
        .active_index(nav_active.get())
        .width(SB)
        .height(GH as f32);
    let nav_node = nav.build(tk);

    let active_page = nav_active.get();
    // 页面标题固定在 ScrollView 外部
    let (icon, label) = PAGE_TITLES[active_page.min(11)];
    let title_node = page_title(tk, icon, label);
    let page_content = build_page(active_page, tk);

    let content_container = WidgetNode::new(
        Box::new(Container::new().bg(tk.color_bg_container).dir(FlexDirection::Column)
            .flex_grow(1.0)),
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

    tree! {
        Container::new().size(GW as f32, GH as f32).bg(tk.color_bg_layout).dir(FlexDirection::Row) => [
            nav_node,
            content_container,
        ]
    }
}

/// 运行 GUI 演示的主入口。
pub fn run_gui_demo() {
    let tk = DesignTokens::antd_light();
    let nav_active: SharedActive = std::rc::Rc::new(std::cell::Cell::new(0));
    let root_node = build_demo_tree(&tk, &nav_active);
    let mut tree = WidgetTree::new();
    tree.build(root_node);

    // 记录上一次激活索引，变化时重建页面
    let prev_active = std::cell::Cell::new(0usize);

    // 使用 DynTokens 实现运行时主题切换
    let dyn_tokens = Arc::new(DynTokens::new(DesignTokens::antd_light()));
    let dark_mode = std::cell::Cell::new(false);
    let mut app = App::new();
    app.title("UIX — 组件库");

    let mut engine = match app.take_engine(GW, GH) {
        Some(e) => e,
        None => return,
    };

    // 加载 Lucide 图标字体
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
        GW, GH, tc_ref, map_ui_event,
        |ev: &UiEvent| -> bool {
            if let UiEventType::KeyDown = ev.type_ {
                if let UiEventPayload::Key(ref d) = ev.payload {
                    return d.key == KeyCode::Escape;
                }
            }
            false
        },
        move |tree, eng| {
            // ── 主题切换 ──
            let mut new_dark = false;
            tree.find_by_type_and_modify::<ThemeToggle>(|w| {
                new_dark = w.dark.get();
            });
            if new_dark != dark_mode.get() {
                dark_mode.set(new_dark);
                dyn_tokens.set_mode(new_dark);
                let new_tk = dyn_tokens.snapshot();
                let new_root = build_demo_tree(&new_tk, &nav_active);
                tree.build(new_root);
                // 重建后恢复 root frame 为实际窗口尺寸
                if let Some(root) = tree.root_mut() {
                    root.set_frame(Rect::new(0.0, 0.0, eng.width() as f32, eng.height() as f32));
                }
                // 同步新树中 ThemeToggle 的状态
                tree.find_by_type_and_modify::<ThemeToggle>(|w| { w.dark.set(dark_mode.get()); });
                tree.mark_full_frame_dirty();
            }

            // ── 导航切换 → 重建内容页 ──
            let active = nav_active.get();
            if active != prev_active.get() {
                prev_active.set(active);
                let tk = dyn_tokens.snapshot();
                let new_root = build_demo_tree(&tk, &nav_active);
                tree.build(new_root);
                // 重建后恢复 root frame 为实际窗口尺寸（否则 layout 使用 preferred_size 1100x740）
                if let Some(root) = tree.root_mut() {
                    root.set_frame(Rect::new(0.0, 0.0, eng.width() as f32, eng.height() as f32));
                }
                // 同步 ThemeToggle 状态到重建后的树
                tree.find_by_type_and_modify::<ThemeToggle>(|w| { w.dark.set(dark_mode.get()); });
                tree.mark_full_frame_dirty();
            }
        },
    );

    engine.shutdown();
    std::process::exit(exit_code);
}
