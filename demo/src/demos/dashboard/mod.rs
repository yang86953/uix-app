//! Ant Design 5 全组件展示 — 分类多页面版。
//!
//! 运行：`cargo run --bin uix-demo`（可选 `--gpu`）
//!
//! 本演示使用**高级 API**（`WidgetTree` + `run_widget_loop`），展示多页面切换、
//! 主题热重建与 GPU 引擎。入门路径见 `simplified` 演示（`prelude` + `App`）。

pub mod more_pages;
pub mod runtime;
pub mod sections;
pub mod widgets;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use uix::core::log::{debug_fn, error_fn};
use uix::prelude::*;

use more_pages::{page_charts, page_other};
use runtime::{create_engine, create_font_service, run_event_loop};
use sections::{page_data, page_feedback, page_general, page_input, page_layout, page_nav};

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
    ("type", " 通用"),
    ("layout", " 布局"),
    ("menu", " 导航"),
    ("edit", " 输入"),
    ("table", " 数据展示"),
    ("alert-circle", " 反馈"),
    ("bar-chart", " 图表"),
    ("settings", " 其他"),
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
        Self {
            tk,
            items: Vec::new(),
        }
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
    /// 内部容器使用 overflow_content 禁止收缩，ScrollView 负责滚动。
    pub fn build(self) -> WidgetNode {
        let mut children = self.items;
        children.push(Container::new().flex_grow(1.0).into_node());
        WidgetNode::new(
            Box::new(ScrollView::new(ScrollDirection::Vertical).flex_grow(1.0)),
            vec![WidgetNode::new(
                Box::new(
                    Container::new()
                        .flex_grow(1.0)
                        .dir(FlexDirection::Column)
                        .gap(8.0)
                        .pad(EdgeInsets::new(20.0, 4.0, 20.0, 10.0))
                        .overflow_content(),
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
/// 当前未被 demo 页面使用，保留以备将来扩展。
#[allow(dead_code)]
pub fn col(h: f32) -> Space {
    Space::new()
        .size(SpaceSize::Small)
        .height(h)
        .direction(FlexDirection::Column)
        .align(AlignItems::Stretch)
}

/// 固定高度水平占位 Space（宽度由父容器 Stretch 自动拉伸）。
fn space_h(h: f32) -> WidgetNode {
    Space::new().size(SpaceSize::Small).height(h).into_node()
}

/// 页面标题行（图标 + 标签）。
pub fn page_title(tk: &DesignTokens, icon: &str, label: &str) -> WidgetNode {
    tree! { Container::new().dir(FlexDirection::Row).h(30.0).pad(EdgeInsets::new(0.0, 4.0, 0.0, 0.0)) => [
        Icon::new(icon).size(22.0),
        Container::new().size(8.0, 0.0),
        Label::new(label).color(tk.color_text).font_size(22.0),
    ]}
    .into_node()
}

/// 统计卡片（flex-grow 响应式宽度，适合放在 Row Space 中均匀分布）。
/// 构建顶部标题栏（"UIX 组件库" + ThemeToggle + 版本号）。
fn build_header_bar(tk: &DesignTokens) -> WidgetNode {
    tree! { Container::new().bg(tk.color_bg_container).dir(FlexDirection::Row)
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
        0 => page_general(tk),
        1 => page_layout(tk),
        2 => page_nav(tk),
        3 => page_input(tk),
        4 => page_data(tk),
        5 => page_feedback(tk),
        6 => page_charts(tk),
        7 => page_other(tk),
        _ => page_general(tk),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 树构建 — 基础框架 + 页面内容分步构建
// ════════════════════════════════════════════════════════════════════════════

/// 构建完整的 demo widget tree——所有页面共存于同一棵树中，
/// 通过 `set_visible` 切换显示，无需重建。
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
///     ├── header_bar (Row, 44px)
///     └── page_panel (Container, flex-grow, Column)
///         ├── page_0 (Container, flex-grow, visible=active==0, Column)
///         │   ├── title_node
///         │   └── ScrollView(...)
///         ├── page_1 (Container, flex-grow, visible=active==1, Column)
///         │   ├── title_node
///         │   └── ScrollView(...)
///         └── ...
/// ```
/// 返回 `(root_node, nav_active, page_ids)`。
fn build_demo_tree(
    tk: &DesignTokens,
    active_page: usize,
) -> (WidgetNode, SharedActive, Vec<WidgetId>) {
    let nav = Navigation::new("UIX 组件")
        .item(" 通用", "type")
        .item(" 布局", "layout")
        .item(" 导航", "menu")
        .item(" 输入", "edit")
        .item(" 数据展示", "table")
        .item(" 反馈", "alert-circle")
        .item(" 图表", "bar-chart")
        .item(" 其他", "settings")
        .active_index(active_page)
        .width(SIDEBAR_W);
    let nav_active = nav.active().clone();
    let nav_node = nav.build(tk);

    // 为每页构建独立的页面容器（标题 + 内容），全部添加到 page_panel
    let mut page_nodes = Vec::new();
    let mut page_ids = Vec::new();
    for (i, &(icon, label)) in PAGE_TITLES.iter().enumerate() {
        let title_node = page_title(tk, icon, label);
        let page_content = build_page(i, tk);
        page_nodes.push(WidgetNode::new(
            Box::new(Container::new().dir(FlexDirection::Column).flex_grow(1.0)),
            vec![title_node, page_content],
        ));
        // WidgetId 在树构建后才能得到，这里先占位
        page_ids.push(WidgetId::default());
    }

    // page_panel 是页面容器的父节点，不可见子节点不参与布局（见 Container::layout_children）
    let page_panel = WidgetNode::new(
        Box::new(Container::new().dir(FlexDirection::Column).flex_grow(1.0)),
        page_nodes,
    );

    let content_container = WidgetNode::new(
        Box::new(
            Container::new()
                .bg(tk.color_bg_container)
                .dir(FlexDirection::Column)
                .flex_grow(1.0),
        ),
        vec![build_header_bar(tk), page_panel],
    );

    let root = tree! {
        Container::new().flex_grow(1.0).bg(tk.color_bg_layout).dir(FlexDirection::Row) => [
            nav_node,
            content_container,
        ]
    };
    (root, nav_active, page_ids)
}

// ════════════════════════════════════════════════════════════════════════════
// 运行时状态
// ════════════════════════════════════════════════════════════════════════════

pub(super) struct DemoState {
    pub(super) prev_active: Cell<usize>,
    pub(super) dark_mode: Cell<bool>,
    pub(super) nav_active: Rc<std::cell::RefCell<SharedActive>>,
    /// 每页在树中的根容器 WidgetId（用于切换可见性）
    pub(super) page_ids: std::cell::RefCell<Vec<WidgetId>>,
}

impl DemoState {
    fn new(nav_active: SharedActive, page_ids: Vec<WidgetId>) -> Self {
        Self {
            prev_active: Cell::new(0),
            dark_mode: Cell::new(false),
            nav_active: Rc::new(std::cell::RefCell::new(nav_active)),
            page_ids: std::cell::RefCell::new(page_ids),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// GUI 入口
// ════════════════════════════════════════════════════════════════════════════

// ── 页面切换（可见性切换，不重建树）──

/// 切换到指定页面：隐藏旧页面，显示新页面。
pub(super) fn switch_page(tree: &mut WidgetTree, state: &DemoState, active: usize) {
    let prev = state.prev_active.get();
    if prev == active {
        return;
    }
    debug_fn(format!("switch_page: {} -> {}", prev, active));
    state.prev_active.set(active);

    let ids = state.page_ids.borrow();
    // 隐藏旧页面
    if prev < ids.len() {
        tree.set_visible(ids[prev], false);
    }
    // 显示新页面
    if active < ids.len() {
        tree.set_visible(ids[active], true);
        tree.invalidate_paint_subtree(ids[active]);
    }
    // 标记全场重绘（树结构未变，render loop 的 tree_version 检查会触发再布局）
    tree.mark_full_frame_dirty();
}

/// 主题切换：重建整棵树（主题 token 全局变化，无法增量更新）。
pub(super) fn rebuild_for_theme(
    tree: &mut WidgetTree,
    eng: &mut dyn GraphicsEngine,
    dyn_tokens: &DynTokens,
    state: &DemoState,
) -> SharedActive {
    debug_fn(format!("rebuild_for_theme: dark={}", state.dark_mode.get()));
    // 先更新内存中的 tokens，供后续 snapshot 和渲染使用
    dyn_tokens.set_mode(state.dark_mode.get());
    let tk = dyn_tokens.snapshot();
    let active = state.prev_active.get();
    let (new_root, new_active, _) = build_demo_tree(&tk, active);
    tree.build(new_root);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(
            0.0,
            0.0,
            eng.canvas_2d().width() as f32,
            eng.canvas_2d().height() as f32,
        ));
    }
    // 树构建后通过遍历获取每页容器的真实 WidgetId
    let page_ids: Vec<WidgetId> = tree
        .root_id()
        .and_then(|root| tree.get(root))
        .map(|r| r.children().to_vec())
        .and_then(|c| if c.len() >= 2 { tree.get(c[1]) } else { None })
        .map(|cc| cc.children().to_vec())
        .and_then(|c| if c.len() >= 2 { tree.get(c[1]) } else { None })
        .map(|pp| pp.children().to_vec())
        .unwrap_or_default();
    *state.page_ids.borrow_mut() = page_ids.clone();
    // 隐藏所有非活跃页面（build_demo_tree 默认全部可见）
    for (i, &id) in page_ids.iter().enumerate() {
        if i != active {
            tree.set_visible(id, false);
        }
    }
    tree.layout();
    tree.find_by_type_and_modify::<ThemeToggle>(|w| {
        w.dark.set(state.dark_mode.get());
    });
    tree.mark_full_frame_dirty();
    new_active
}

/// 运行 GUI 演示的主入口（高级 API：`WidgetTree` + `run_widget_loop`）。
pub fn run_gui_demo() {
    let use_gpu = std::env::args().any(|a| a == "--gpu");
    let tk = DesignTokens::antd_light();

    let (root_node, nav_active, _) = build_demo_tree(&tk, 0);
    let mut tree = WidgetTree::new();
    tree.build(root_node);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, INIT_W as f32, INIT_H as f32));
    }

    let page_ids: Vec<WidgetId> = collect_page_ids(&tree);
    for (i, &id) in page_ids.iter().enumerate() {
        if i != 0 {
            tree.set_visible(id, false);
        }
    }
    tree.layout();
    tree.mark_full_frame_dirty();

    let state = DemoState::new(nav_active, page_ids);
    let dyn_tokens = Arc::new(DynTokens::new(tk));

    let mut platform = match create_platform() {
        Ok(p) => p,
        Err(e) => {
            error_fn(format!("create_platform: {}", e.short_what()));
            return;
        }
    };

    let mut platform_window =
        match platform
            .window_manager()
            .create_window("UIX — 组件库", INIT_W, INIT_H)
        {
            Ok(w) => w,
            Err(e) => {
                error_fn(format!("create_window: {}", e.short_what()));
                return;
            }
        };
    platform_window.center_on_screen();
    platform_window.show();
    platform_window.raise();

    let mut engine = create_engine(use_gpu, &*platform_window, INIT_W, INIT_H);
    let font_service = create_font_service(&*platform);
    let image_service = ImageService::new();

    let exit_code = run_event_loop(
        &mut *platform,
        &mut *platform_window,
        &mut *engine,
        &mut tree,
        &font_service,
        &image_service,
        &state,
        dyn_tokens,
    );

    engine.shutdown();
    std::process::exit(exit_code);
}

/// 从 demo 树中提取各页面容器的 WidgetId。
fn collect_page_ids(tree: &WidgetTree) -> Vec<WidgetId> {
    tree.root_id()
        .and_then(|root| tree.get(root))
        .map(|r| r.children().to_vec())
        .and_then(|c| if c.len() >= 2 { tree.get(c[1]) } else { None })
        .map(|cc| cc.children().to_vec())
        .and_then(|c| if c.len() >= 2 { tree.get(c[1]) } else { None })
        .map(|pp| pp.children().to_vec())
        .unwrap_or_default()
}

// ════════════════════════════════════════════════════════════════════════════
// 测试
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
#[path = "../../tests/demos/dashboard.rs"]
mod tests;
