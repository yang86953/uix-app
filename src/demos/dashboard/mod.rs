//! Ant Design 5 全组件展示 — 分类多页面版。
//!
//! 每个组件分类独立成页，左侧导航切换页面，不再挤在一页内滚动。
//!
//! 运行：`cargo run --bin uix-demo`

pub mod more_pages;
pub mod sections;
pub mod widgets;

use std::rc::Rc;
use std::sync::Arc;
use uix::app::{map_ui_event, App};
use uix::base::KeyCode;
use uix::base::{EdgeInsets, Rect};
use uix::graphics::Color;
use uix::graphics::{AlignItems, FlexDirection};
use uix::platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix::tree;
use uix::ui::theme::{DesignTokens, DynTokens, Theme};
use uix::ui::widget::{WidgetCore, WidgetNode, WidgetTree};
use uix::ui::widgets::icon::init_lucide_font;
use uix::ui::{
    Card, Container, Icon, IntoWidgetNode, Label, Navigation, ScrollDirection, ScrollView,
    SharedActive, Space, SpaceSize,
};

use more_pages::{page_breadcrumb, page_colors, page_custom, page_layout, page_nav, page_tabs};
use sections::{
    page_buttons, page_dashboard, page_data_display, page_feedback, page_inputs, page_typography,
};
use widgets::ThemeToggle;

/// 窗口初始尺寸（引擎缓冲和窗口创建时的参考值，非硬编码给 widget 布局）。
/// widget 布局由 flex-grow/stretch 等布局系统决定，不使用这些常量。
const INIT_W: i32 = 1200;
const INIT_H: i32 = 800;
/// 侧边栏固定宽度。
const SB: f32 = 200.0;

/// 内容区宽度参考值（基于窗口初始宽度，用于组件首次构建时的尺寸参考）。
pub fn cw() -> f32 {
    INIT_W as f32 - SB
}
/// 内容区内边距后的可用宽度（用于组件首次构建时的尺寸参考）。
pub fn iw() -> f32 {
    cw() - 40.0
}

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
        .width(iw())
        .height(h)
        .direction(FlexDirection::Row)
        .align(AlignItems::Center)
}
pub fn col(h: f32) -> Space {
    Space::new()
        .size(SpaceSize::Small)
        .width(iw())
        .height(h)
        .direction(FlexDirection::Column)
        .align(AlignItems::Stretch)
}

/// 统计卡片
pub fn stat_card(tk: &DesignTokens, title: &str, value: &str, color: Color, elev: u8) -> Card {
    let w = (iw() - 24.0) / 4.0;
    Card::new()
        .title(title)
        .elevation(elev)
        .hoverable()
        .size(w, 100.0)
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
                    .size(iw(), 0.0)
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
        .width(SB);
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
    let nav_active: Rc<std::cell::RefCell<uix::ui::widgets::nav::SharedActive>> =
        Rc::new(std::cell::RefCell::new(nav_active));
    let mut tree = WidgetTree::new();
    tree.build(root_node);

    // 记录上一次激活索引，变化时重建页面
    let prev_active = std::cell::Cell::new(0usize);

    // 使用 DynTokens 实现运行时主题切换
    let dyn_tokens = Arc::new(DynTokens::new(DesignTokens::antd_light()));
    let dark_mode = std::cell::Cell::new(false);
    let mut app = App::new();
    app.title("UIX — 组件库");

    let mut engine = match app.take_engine(INIT_W, INIT_H) {
        Some(e) => e,
        None => return,
    };

    // 加载 Lucide 图标字体（通过 downcast 访问 FontService）
    if let Ok(ttf) = std::fs::read("assets/fonts/lucide.ttf") {
        if let Some(sw) = engine
            .as_any_mut()
            .downcast_mut::<uix::graphics::software_engine::SoftwareEngine>()
        {
            init_lucide_font(&ttf, &mut sw.font_service);
        } else {
            log::warn!("Engine is not SoftwareEngine — Lucide font not loaded");
        }
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
            if new_dark != dark_mode.get() {
                dark_mode.set(new_dark);
                dyn_tokens.set_mode(new_dark);
                let new_tk = dyn_tokens.snapshot();
                let (new_root, new_active) = build_demo_tree(&new_tk, prev_active.get());
                *nav_active.borrow_mut() = new_active;
                tree.build(new_root);
                // 重建后恢复 root frame 为实际窗口尺寸
                if let Some(root) = tree.root_mut() {
                    root.set_frame(Rect::new(0.0, 0.0, eng.width() as f32, eng.height() as f32));
                }
                tree.layout();
                // 同步新树中 ThemeToggle 的状态
                tree.find_by_type_and_modify::<ThemeToggle>(|w| {
                    w.dark.set(dark_mode.get());
                });
                tree.mark_full_frame_dirty();
            }

            // ── 导航切换 → 重建内容页 ──
            let active = nav_active.borrow().get();
            if active != prev_active.get() {
                prev_active.set(active);
                let tk = dyn_tokens.snapshot();
                let (new_root, new_active) = build_demo_tree(&tk, active);
                *nav_active.borrow_mut() = new_active;
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
            }
        },
    );
    
    engine.shutdown();
    std::process::exit(exit_code);
}
