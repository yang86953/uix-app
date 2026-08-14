//! 组合子函数 — column, row, label, button, space, input 等。
//!
//! 提供函数式声明式 API。异质子节点用元组或 [`crate::views!`]：
//! `column((label("..."), button("...")))` / `column(views![...])`。
//!
//! # 使用示例
//!
//! ```ignore
//! use crate::ui::view::*;
//!
//! let ui = column((
//!     label("Hello").font_size(24.0).color(Color::blue()),
//!     button("Click").primary().on_click_fn(|| println!("clicked")),
//! )).padding(16.0);
//! ```

use crate::platform::windowing::ScrollDirection;
use crate::ui::layout::{FlexDirection, GridTrack};
use crate::ui::theme::style::{DisplayMode, Style};
use crate::ui::view::{View, ViewNode};

use crate::core::{Constraints, Rect, Size};
use crate::ui::WidgetTree;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::component::traits::{
    WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetRender,
};
use crate::ui::reactive::state::{Computed, State};
use std::any::Any;

// 拆分命令式节点嵌入转换，保持组合器主体在文件规模约束内。
#[path = "combinators/embed.rs"]
// 编译命令式节点嵌入转换的私有实现模块。
mod embed_node;

/// 将异质 / 同质子节点收成 `Vec<ViewNode>`（公开用法见仓库 `docs/使用/布局.md`）。
///
/// - 同质：`column([label("a"), label("b")])`、`Vec<_>`
/// - 异质：`column((label("a"), button("b")))` 或 `column(views![...])`
pub trait IntoViewChildren {
    fn into_view_children(self) -> Vec<ViewNode>;
}

impl<T: View, const N: usize> IntoViewChildren for [T; N] {
    fn into_view_children(self) -> Vec<ViewNode> {
        self.into_iter().map(View::build).collect()
    }
}

impl<T: View> IntoViewChildren for Vec<T> {
    fn into_view_children(self) -> Vec<ViewNode> {
        self.into_iter().map(View::build).collect()
    }
}

/// 条件子节点：`true` 时包含 `child`，否则为空列表。
///
/// ```ignore
/// column(show(visible, label("详情")))
/// ```
impl View for () {
    fn build(self) -> ViewNode {
        ViewNode::leaf(crate::ui::widgets::Space::new())
    }
}

pub fn show(when: bool, child: impl View) -> Vec<ViewNode> {
    if when {
        vec![child.build()]
    } else {
        Vec::new()
    }
}

/// 条件子节点（`Option`）：`Some` 构建为子项，`None` 跳过。
///
/// ```ignore
/// column((label("标题"), optional_banner))
/// ```
impl<T: View> View for Option<T> {
    fn build(self) -> ViewNode {
        match self {
            Some(child) => child.build(),
            None => ViewNode::leaf(crate::ui::widgets::Space::new()),
        }
    }
}

macro_rules! impl_into_view_children_tuple {
    () => {
        impl IntoViewChildren for () {
            fn into_view_children(self) -> Vec<ViewNode> {
                Vec::new()
            }
        }
    };
    ($($T:ident),+) => {
        impl<$($T: View),+> IntoViewChildren for ($($T,)+) {
            fn into_view_children(self) -> Vec<ViewNode> {
                #[allow(non_snake_case)]
                let ($($T,)+) = self;
                vec![$($T.build(),)+]
            }
        }
    };
}

impl_into_view_children_tuple!();
impl_into_view_children_tuple!(A);
impl_into_view_children_tuple!(A, B);
impl_into_view_children_tuple!(A, B, C);
impl_into_view_children_tuple!(A, B, C, D);
impl_into_view_children_tuple!(A, B, C, D, E);
impl_into_view_children_tuple!(A, B, C, D, E, F);
impl_into_view_children_tuple!(A, B, C, D, E, F, G);
impl_into_view_children_tuple!(A, B, C, D, E, F, G, H);
impl_into_view_children_tuple!(A, B, C, D, E, F, G, H, I);
impl_into_view_children_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_into_view_children_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_into_view_children_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

/// 将 `tree!` / widget-tree 节点嵌入 View DSL（组件库演示等高级 interop）。
pub fn embed(node: impl crate::ui::IntoWidgetNode) -> ViewNode {
    // 将命令式节点转换为可由声明适配器继续处理的节点。
    embed_node::adopt_widget_node(node.into_node())
}

// ── 基础组合子 ──────────────────────────────────────────────

/// 列容器（Flex 方向为 Column），默认 flex_grow(1.0) 填满父容器高度。
///
/// 嵌套内容组若只需 intrinsic 高度，用 [`column_fit`]（公开用法见仓库 `docs/使用/布局.md`）。
///
/// 同质数组 / `Vec`，或异质元组 / [`crate::views!`]（公开用法见仓库 `docs/使用/布局.md`）。
pub fn column(children: impl IntoViewChildren) -> ViewNode {
    ViewNode::new(
        crate::ui::widgets::Container::new()
            .dir(FlexDirection::Column)
            .flex_grow(1.0),
        children.into_view_children(),
    )
}

/// 列容器，保持 intrinsic 高度（flex_grow = 0）。
///
/// 用于顶栏、侧栏品牌区、卡片内文案组等不应吞掉父列剩余空间的局部内容。
pub fn column_fit(children: impl IntoViewChildren) -> ViewNode {
    ViewNode::new(
        crate::ui::widgets::Container::new().dir(FlexDirection::Column),
        children.into_view_children(),
    )
}

/// 行容器（Flex 方向为 Row）。
///
/// 同质数组 / `Vec`，或异质元组 / [`crate::views!`]（公开用法见仓库 `docs/使用/布局.md`）。
pub fn row(children: impl IntoViewChildren) -> ViewNode {
    ViewNode::new(
        crate::ui::widgets::Container::new().dir(FlexDirection::Row),
        children.into_view_children(),
    )
}

/// Grid 容器。
///
/// 默认不预设轨道；调用 `.columns(...)` / `.rows(...)` 明确声明轨道。
pub fn grid(children: impl IntoViewChildren) -> GridBuilder {
    GridBuilder {
        children: children.into_view_children(),
        widget: crate::ui::widgets::Grid::new(),
        style: Style::default().with_display(DisplayMode::Grid),
    }
}

pub struct GridBuilder {
    children: Vec<ViewNode>,
    widget: crate::ui::widgets::Grid,
    style: Style,
}

impl GridBuilder {
    pub fn columns(mut self, columns: Vec<GridTrack>) -> Self {
        self.style = self.style.with_grid_columns(columns.clone());
        self.widget = self.widget.columns(columns);
        self
    }

    pub fn rows(mut self, rows: Vec<GridTrack>) -> Self {
        self.style = self.style.with_grid_rows(rows.clone());
        self.widget = self.widget.rows(rows);
        self
    }

    /// 切换为 24 单元响应式 Grid。
    pub fn responsive(mut self) -> Self {
        self.style.grid_template_columns.clear();
        self.widget = crate::ui::widgets::Grid::responsive();
        self
    }

    pub fn breakpoints(mut self, breakpoints: crate::ui::widgets::Breakpoints) -> Self {
        self.style.grid_template_columns.clear();
        self.widget = self.widget.breakpoints(breakpoints);
        self
    }

    pub fn cols(mut self, cols: Vec<crate::ui::widgets::Col>) -> Self {
        self.style.grid_template_columns.clear();
        self.widget = self.widget.cols(cols);
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.style = self.style.with_gap(gap).with_grid_gap(gap, gap);
        self.widget = self.widget.gap(gap);
        self
    }

    pub fn col_gap(mut self, gap: f32) -> Self {
        self.style.grid_column_gap = gap;
        self.widget = self.widget.col_gap(gap);
        self
    }

    pub fn row_gap(mut self, gap: f32) -> Self {
        self.style.grid_row_gap = gap;
        self.widget = self.widget.row_gap(gap);
        self
    }

    pub fn two_columns(mut self) -> Self {
        let columns = vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)];
        self.style = self.style.with_grid_columns(columns.clone());
        self.widget = self.widget.columns(columns);
        self
    }

    pub fn three_columns(mut self) -> Self {
        let columns = vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0), GridTrack::Fr(1.0)];
        self.style = self.style.with_grid_columns(columns.clone());
        self.widget = self.widget.columns(columns);
        self
    }
}

impl View for GridBuilder {
    fn build(self) -> ViewNode {
        let mut node = ViewNode::new(self.widget, self.children);
        node.style = self.style;
        node
    }
}

impl From<GridBuilder> for ViewNode {
    fn from(builder: GridBuilder) -> Self {
        builder.build()
    }
}

/// Scroll 容器。
///
/// 默认垂直滚动；可链式切换为 `.horizontal()` 或 `.both()`。
pub fn scroll(child: impl View) -> ScrollBuilder {
    ScrollBuilder {
        child: child.build(),
        direction: ScrollDirection::Vertical,
        fixed_size: None,
        flex_grow: 1.0,
        show_scrollbar: true,
        scroll_offset: None,
    }
}

pub struct ScrollBuilder {
    child: ViewNode,
    direction: ScrollDirection,
    fixed_size: Option<(f32, f32)>,
    flex_grow: f32,
    show_scrollbar: bool,
    scroll_offset: Option<State<crate::core::Point>>,
}

impl ScrollBuilder {
    pub fn vertical(mut self) -> Self {
        self.direction = ScrollDirection::Vertical;
        self
    }

    pub fn horizontal(mut self) -> Self {
        self.direction = ScrollDirection::Horizontal;
        self
    }

    pub fn both(mut self) -> Self {
        self.direction = ScrollDirection::Both;
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_size = Some((w, h));
        self
    }

    pub fn flex_grow(mut self, value: f32) -> Self {
        self.flex_grow = value;
        self
    }

    pub fn show_scrollbar(mut self, value: bool) -> Self {
        self.show_scrollbar = value;
        self
    }

    /// 将视口运行态偏移双向绑定到应用 State。
    pub fn scroll_offset(mut self, state: &State<crate::core::Point>) -> Self {
        self.scroll_offset = Some(state.clone());
        self
    }
}

impl View for ScrollBuilder {
    fn build(self) -> ViewNode {
        let mut widget = crate::ui::widgets::ScrollView::new(self.direction)
            .flex_grow(self.flex_grow)
            .show_scrollbar(self.show_scrollbar);
        if let Some((w, h)) = self.fixed_size {
            widget = widget.size(w, h);
        }
        if let Some(offset) = self.scroll_offset.as_ref() {
            widget = widget.scroll_offset(offset);
        }
        ViewNode::new(widget, vec![self.child])
    }
}

impl From<ScrollBuilder> for ViewNode {
    fn from(builder: ScrollBuilder) -> Self {
        builder.build()
    }
}

/// 文本内容：静态字符串或动态闭包（公开用法见仓库 `docs/使用/组件.md`）。
pub trait IntoLabelContent {
    fn into_label_node(self) -> ViewNode;
}

impl IntoLabelContent for String {
    fn into_label_node(self) -> ViewNode {
        ViewNode::leaf(crate::ui::widgets::Label::new(self))
    }
}

impl IntoLabelContent for &str {
    fn into_label_node(self) -> ViewNode {
        ViewNode::leaf(crate::ui::widgets::Label::new(self))
    }
}

impl IntoLabelContent for &String {
    fn into_label_node(self) -> ViewNode {
        ViewNode::leaf(crate::ui::widgets::Label::new(self.as_str()))
    }
}

impl<F> IntoLabelContent for F
where
    F: Fn() -> String + 'static,
{
    fn into_label_node(self) -> ViewNode {
        ViewNode::leaf(crate::ui::component::dynamic_label::DynamicLabel::new(self))
    }
}

/// 文本标签 — 静态或动态统一入口。
///
/// ```ignore
/// label("Hello");
/// label(move || format!("计数: {}", count.get()));
/// count.map_text(|n| format!("计数: {n}")); // 等价，少手写 clone
/// ```
pub fn label(content: impl IntoLabelContent) -> ViewNode {
    content.into_label_node()
}

/// 响应式文本标签（`label(closure)` 的别名，保留兼容）。
pub fn dynamic_label<F: Fn() -> String + 'static>(f: F) -> ViewNode {
    label(f)
}

impl<T: Clone + Send + Sync + 'static> State<T> {
    /// 由当前 State 值构建子视图；结构更新由根 View 构建期捕获的依赖触发。
    pub fn map<F, V>(&self, f: F) -> ViewNode
    where
        F: FnOnce(&T) -> V,
        V: View,
    {
        f(&self.get()).build()
    }

    /// 由 State 生成响应式文本节点；内部 clone 句柄，调用方只保留一个名字。
    pub fn map_text<F>(&self, f: F) -> ViewNode
    where
        F: Fn(&T) -> String + 'static,
    {
        let state = self.clone();
        label(move || f(&state.get()))
    }

    /// 按当前 State 值构建可选子视图；`None` 使用空节点参与 reconcile。
    pub fn map_opt<F, V>(&self, f: F) -> Option<ViewNode>
    where
        F: FnOnce(&T) -> Option<V>,
        V: View,
    {
        f(&self.get()).map(View::build)
    }
}

impl<T: Clone + Send + Sync + 'static> Computed<T> {
    /// 由派生值构建当前子视图；结构更新由根 View 构建期捕获的依赖触发。
    pub fn map<F, V>(&self, f: F) -> ViewNode
    where
        F: FnOnce(&T) -> V,
        V: View,
    {
        f(&self.get()).build()
    }

    /// 由派生值生成响应式文本节点，依赖变化仅触发窄 Paint 失效。
    pub fn map_text<F>(&self, f: F) -> ViewNode
    where
        F: Fn(&T) -> String + 'static,
    {
        let computed = self.clone();
        label(move || f(&computed.get()))
    }

    /// 按派生值构建可选子视图；`None` 使用空节点参与 reconcile。
    pub fn map_opt<F, V>(&self, f: F) -> Option<ViewNode>
    where
        F: FnOnce(&T) -> Option<V>,
        V: View,
    {
        f(&self.get()).map(View::build)
    }
}

type CanvasPaint = dyn Fn(Rect, &mut PaintContext<'_, '_>);

struct Canvas {
    size: Size,
    paint: Box<CanvasPaint>,
}

impl WidgetComponent for Canvas {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER)
    }

    fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        Some(self)
    }

    fn as_render(&self) -> Option<&dyn WidgetRender> {
        Some(self)
    }

    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        Some(self)
    }
}

impl WidgetLayout for Canvas {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.size)
    }
}

impl WidgetRender for Canvas {
    fn render(&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        (self.paint)(frame, ctx);
    }
}

/// 创建固定 logical 尺寸的轻量绘制节点，无需声明完整组件。
///
/// 闭包只在节点需要绘制时执行；其中读取的 `State` / `Computed` 会自动绑定
/// 到该节点的 Paint 失效，不会形成每帧回调。
pub fn canvas<F>(width: f32, height: f32, paint: F) -> ViewNode
where
    F: Fn(Rect, &mut PaintContext) + 'static,
{
    let finite_extent = |value: f32| {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    };
    ViewNode::leaf(Canvas {
        size: Size::new(finite_extent(width), finite_extent(height)),
        paint: Box::new(paint),
    })
}

/// 空白占位，通过 `height` 控制垂直间距。
pub fn space(height: f32) -> ViewNode {
    ViewNode::leaf(
        crate::ui::widgets::Space::new()
            .size(crate::ui::widgets::SpaceSize::Small)
            .height(height),
    )
}

// ── 按钮（View DSL 唯一入口）──────────────────────────────────

use crate::ui::event::{HandlerRegistration, SemanticEvent, SemanticKind};
use crate::ui::theme::style::StyleSet;
// 引入按钮组件与连体位置契约。
use crate::ui::widgets::{Button, ButtonGroupPosition};

/// 按钮构建器 — `button("text").primary().on_click(&state, |s| …)`。
///
/// 样式（背景、颜色、边距等）在 builder 之后链式调用 `StyleExt` 方法：
/// `button("保存").primary().bg(color).padding(8.0)`。
///
/// 嵌入 `tree!` / `Space::child` 等非 View 容器时，末尾调用 `.widget()`。
pub struct ButtonBuilder {
    text: String,
    style_set: StyleSet,
    disabled: bool,
    block: bool,
    size: crate::platform::windowing::ControlSize,
    handlers: Vec<HandlerRegistration>,
}

impl ButtonBuilder {
    fn into_parts(self) -> (Button, Vec<HandlerRegistration>) {
        (
            Button::assemble(
                self.text,
                self.style_set.into(),
                self.disabled,
                self.block,
                self.size,
            ),
            self.handlers,
        )
    }

    /// 构建底层 `Button`（用于 `tree!`、`Space::child` 等）。
    pub fn widget(self) -> Button {
        self.into_parts().0
    }
    pub fn style_set(mut self, style_set: StyleSet) -> Self {
        self.style_set = style_set;
        self
    }

    pub fn primary(mut self) -> Self {
        self.style_set = StyleSet::button_primary();
        self
    }
    pub fn ghost(mut self) -> Self {
        self.style_set = StyleSet::button_ghost();
        self
    }
    pub fn danger(mut self) -> Self {
        self.style_set = StyleSet::button_danger();
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn block(mut self, v: bool) -> Self {
        self.block = v;
        self
    }
    pub fn size(mut self, size: crate::platform::windowing::ControlSize) -> Self {
        self.size = size;
        self
    }

    /// 将底层按钮标记为 ButtonGroup 中的连体位置并物化为 ViewNode。
    pub fn group_position(self, position: ButtonGroupPosition) -> ViewNode {
        // 拆分底层按钮与已登记事件处理器。
        let (button, handlers) = self.into_parts();
        // 把连体位置交给既有 Button 视觉语义。
        let mut node = ViewNode::leaf(button.group_position(position));
        // 保留物化前已经登记的按钮事件。
        node.handlers = handlers;
        // 返回可继续应用公共样式与事件的节点。
        node
    }

    /// 默认点击路径：绑定 `State` 指纹，reconcile 可稳定复用（公开用法见仓库 `docs/使用/事件.md`）。
    ///
    /// 框架传入 `&State<T>`，调用方无需再 clone 句柄进闭包。
    pub fn on_click<T, F>(mut self, state: &State<T>, mut f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut(&State<T>) + 'static,
    {
        let captured = state.clone();
        self.handlers.push(
            HandlerRegistration::new(
                SemanticKind::Click,
                Box::new(move |event| {
                    if event.is_primary_click() {
                        f(&captured);
                    }
                }),
            )
            .with_state_capture(state),
        );
        self
    }

    /// 无 State 的点击闭包；每次 reconcile **保守重绑**（公开用法见仓库 `docs/使用/事件.md`）。
    ///
    /// 命名保留 `_fn`：Rust 无法与 [`Self::on_click`] 重载；有 State 时优先 `on_click(&state, …)`（公开用法见仓库 `docs/使用/事件.md`）。
    pub fn on_click_fn<F: FnMut() + 'static>(mut self, mut f: F) -> Self {
        self.handlers.push(HandlerRegistration::new(
            SemanticKind::Click,
            Box::new(move |event| {
                if event.is_primary_click() {
                    f();
                }
            }),
        ));
        self
    }

    /// 兼容别名：指纹同 [`Self::on_click`]，闭包不接收 `&State`。
    ///
    /// 新代码优先 `on_click(&state, |s| …)`；无 State 用 [`Self::on_click_fn`]。
    #[doc(alias = "on_click")]
    pub fn on_click_capture<T, F>(self, state: &State<T>, mut f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut() + 'static,
    {
        self.on_click(state, move |_| f())
    }

    pub fn on_click_window_capture<F>(mut self, window_id: crate::core::WindowId, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(
                SemanticKind::Click,
                Box::new(move |event| {
                    if event.is_primary_click() {
                        f();
                    }
                }),
            )
            .with_window_capture(window_id),
        );
        self
    }

    pub fn on_click_event<F: FnMut(&mut SemanticEvent) + 'static>(mut self, f: F) -> Self {
        self.handlers
            .push(HandlerRegistration::new(SemanticKind::Click, Box::new(f)));
        self
    }

    pub fn on_click_event_capture<T, F>(mut self, state: &State<T>, f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut(&mut SemanticEvent) + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(SemanticKind::Click, Box::new(f)).with_state_capture(state),
        );
        self
    }

    pub fn on_click_event_window_capture<F>(
        mut self,
        window_id: crate::core::WindowId,
        f: F,
    ) -> Self
    where
        F: FnMut(&mut SemanticEvent) + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(SemanticKind::Click, Box::new(f))
                .with_window_capture(window_id),
        );
        self
    }
}

impl View for ButtonBuilder {
    fn build(self) -> ViewNode {
        let (button, handlers) = self.into_parts();
        let mut node = ViewNode::leaf(button);
        node.handlers = handlers;
        node
    }
}

impl From<ButtonBuilder> for ViewNode {
    fn from(b: ButtonBuilder) -> Self {
        b.build()
    }
}

/// 创建按钮。
///
/// ```ignore
/// button("保存").primary().on_click(&state, |s| save(s));
/// button("关闭").on_click_fn(|| close());
/// ```
pub fn button(text: impl Into<String>) -> ButtonBuilder {
    let config = crate::ui::component::config::use_config();
    ButtonBuilder {
        text: text.into(),
        style_set: config
            .overrides
            .button
            .style_set
            .unwrap_or_else(StyleSet::button_default),
        disabled: config.disabled,
        block: false,
        size: config.size,
        handlers: Vec::new(),
    }
}

// ── 输入框构建器 ────────────────────────────────────────────

/// 输入框构建器。通过 `input().placeholder("...").on_change(fn)` 链式调用。
///
/// # 示例
///
/// ```ignore
/// input().placeholder("请输入用户名").on_change(|v| println!("输入: {}", v))
/// ```
pub struct InputBuilder {
    placeholder: String,
    value: Option<State<String>>,
    handlers: Vec<HandlerRegistration>,
}

impl InputBuilder {
    /// 设置占位文本。
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// 绑定输入值；组件编辑与外部 `State` 更新保持双向同步。
    pub fn value(mut self, state: &State<String>) -> Self {
        self.value = Some(state.clone());
        self
    }

    /// 绑定值变更回调（输入内容变化时触发）。
    pub fn on_change<F: FnMut(&str) + 'static>(mut self, mut f: F) -> Self {
        self.handlers.push(HandlerRegistration::new(
            SemanticKind::Change,
            Box::new(move |event| {
                if let Some(value) = event.text_payload() {
                    f(value);
                }
            }),
        ));
        self
    }

    pub fn on_change_capture<T, F>(
        mut self,
        state: &crate::ui::reactive::state::State<T>,
        mut f: F,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut(&str) + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(
                SemanticKind::Change,
                Box::new(move |event| {
                    if let Some(value) = event.text_payload() {
                        f(value);
                    }
                }),
            )
            .with_state_capture(state),
        );
        self
    }

    pub fn on_change_window_capture<F>(mut self, window_id: crate::core::WindowId, mut f: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(
                SemanticKind::Change,
                Box::new(move |event| {
                    if let Some(value) = event.text_payload() {
                        f(value);
                    }
                }),
            )
            .with_window_capture(window_id),
        );
        self
    }
}

impl View for InputBuilder {
    fn build(self) -> ViewNode {
        let mut input = crate::ui::widgets::Input::new(self.placeholder);
        if let Some(value) = self.value.as_ref() {
            input = input.value(value);
        }
        let mut node = ViewNode::leaf(input);
        node.handlers = self.handlers;
        node
    }
}

impl From<InputBuilder> for ViewNode {
    fn from(builder: InputBuilder) -> Self {
        builder.build()
    }
}

/// 创建输入框。返回 `InputBuilder` 以链式设置属性。
///
/// # 示例
///
/// ```ignore
/// input().placeholder("搜索...").on_change(|v| search(v))
/// ```
pub fn input() -> InputBuilder {
    InputBuilder {
        placeholder: String::new(),
        value: None,
        handlers: Vec::new(),
    }
}
