//! 组合子函数 — column, row, label, button, space, input 等。
//!
//! 提供函数式声明式 API，让用户用 `column([label("..."), button("...")])` 方式组合 UI。
//!
//! # 使用示例
//!
//! ```ignore
//! use crate::ui::view::*;
//!
//! let ui = column([
//!     label("Hello").font_size(24.0).color(Color::blue()),
//!     button("Click").primary().on_click(|| println!("clicked")),
//! ]).padding(16.0);
//! ```

use crate::native::traits::input::ScrollDirection;
use crate::ui::layout::{FlexDirection, GridTrack};
use crate::ui::style::{DisplayMode, Style};
use crate::ui::view::{View, ViewNode};

use crate::core::{Constraints, Rect, Size};
use crate::draw::compositor::PicturePolicy;
use crate::draw::painting::PaintContext;
use crate::draw::pipeline::InvalidationQueueHandle;
use crate::ui::state::{drain_pending_state_binds, StatePaintBind};
use crate::ui::traits::{WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetRender};
use crate::ui::{WidgetId, WidgetTree};
use std::any::Any;
use std::sync::Arc;

// ── 基础组合子 ──────────────────────────────────────────────

/// 列容器（Flex 方向为 Column），默认 flex_grow(1.0) 填满父容器高度。
///
/// 接受数组或 Vec 作为子节点，支持混合传入任意 `View` 实现。
pub fn column<I>(children: I) -> ViewNode
where
    I: IntoIterator,
    I::Item: View,
{
    let children: Vec<ViewNode> = children.into_iter().map(|v| v.build()).collect();
    ViewNode::new(
        crate::ui::widgets::Container::new()
            .dir(FlexDirection::Column)
            .flex_grow(1.0),
        children,
    )
}

/// 行容器（Flex 方向为 Row）。
///
/// 接受数组或 Vec 作为子节点，支持混合传入任意 `View` 实现。
pub fn row<I>(children: I) -> ViewNode
where
    I: IntoIterator,
    I::Item: View,
{
    let children: Vec<ViewNode> = children.into_iter().map(|v| v.build()).collect();
    ViewNode::new(
        crate::ui::widgets::Container::new().dir(FlexDirection::Row),
        children,
    )
}

/// Grid 容器。
///
/// 默认不预设轨道；调用 `.columns(...)` / `.rows(...)` 明确声明轨道。
pub fn grid<I>(children: I) -> GridBuilder
where
    I: IntoIterator,
    I::Item: View,
{
    GridBuilder {
        children: children.into_iter().map(|v| v.build()).collect(),
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
    }
}

pub struct ScrollBuilder {
    child: ViewNode,
    direction: ScrollDirection,
    fixed_size: Option<(f32, f32)>,
    flex_grow: f32,
    show_scrollbar: bool,
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
}

impl View for ScrollBuilder {
    fn build(self) -> ViewNode {
        let mut widget = crate::ui::widgets::ScrollView::new(self.direction)
            .flex_grow(self.flex_grow)
            .show_scrollbar(self.show_scrollbar);
        if let Some((w, h)) = self.fixed_size {
            widget = widget.size(w, h);
        }
        ViewNode::new(widget, vec![self.child])
    }
}

impl From<ScrollBuilder> for ViewNode {
    fn from(builder: ScrollBuilder) -> Self {
        builder.build()
    }
}

/// 文本标签（静态文本）。
pub fn label(text: impl Into<String>) -> ViewNode {
    ViewNode::leaf(crate::ui::widgets::Label::new(text))
}

/// 响应式文本标签——每次渲染时调用闭包获取最新文本。
/// 配合 `State` 使用时，状态变更自动触发重绘，标签文本自动更新。
///
/// # 示例
///
/// ```ignore
/// let count = State::new(0);
/// label(move || format!("计数: {}", count.get()))  // count 变化时自动刷新
///     .font_size(24.0);
/// ```
pub fn dynamic_label<F: Fn() -> String + 'static>(f: F) -> ViewNode {
    ViewNode::leaf(DynamicLabel::new(f))
}

/// 响应式标签的内部 Widget 实现。
pub(crate) struct DynamicLabel {
    text_fn: Box<dyn Fn() -> String>,
    state_sources: Vec<Arc<dyn StatePaintBind>>,
}

impl DynamicLabel {
    pub fn new<F: Fn() -> String + 'static>(f: F) -> Self {
        Self {
            text_fn: Box::new(f),
            state_sources: drain_pending_state_binds(),
        }
    }

    /// 将关联 State 绑定到 widget 的 Paint 失效。
    pub(crate) fn bind_state_invalidation(
        &self,
        widget_id: WidgetId,
        queue: InvalidationQueueHandle,
        rect: Option<Rect>,
        reconcile_key: usize,
        reconcile: Arc<dyn Fn() + Send + Sync>,
    ) {
        for source in &self.state_sources {
            source.bind_reconcile_site(reconcile_key, reconcile.clone());
            source.bind_paint(widget_id, queue.clone(), rect);
        }
    }

    /// layout 后探测闭包依赖：执行一次文本闭包以捕获 `State::get()`。
    pub(crate) fn probe_dependencies(&self) {
        let _ = (self.text_fn)();
    }
}

impl WidgetComponent for DynamicLabel {
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
        let mut c = WidgetCapabilities::new();
        c.insert(WidgetCapabilities::LAYOUT);
        c.insert(WidgetCapabilities::RENDER);
        c
    }
    fn picture_policy(&self) -> PicturePolicy {
        PicturePolicy::Never
    }
    fn has_dynamic_content(&self) -> bool {
        true
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

impl WidgetLayout for DynamicLabel {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }
}

impl DynamicLabel {
    fn intrinsic_size(&self) -> Size {
        let text = (self.text_fn)();
        let len = text.len() as f32;
        Size::new(len * 7.0, 18.0)
    }
}

impl WidgetRender for DynamicLabel {
    fn render(&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let text = (self.text_fn)();
        if !text.is_empty() {
            ctx.draw_text(
                &text,
                crate::core::Point::new(frame.x, frame.y),
                ctx.tokens().color_text(),
                14.0,
            );
        }
    }
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
use crate::ui::style::StyleSet;
use crate::ui::widgets::Button;

/// 按钮构建器 — 通过 `button("text").primary().on_click(fn)` 创建。
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
    handlers: Vec<HandlerRegistration>,
}

impl ButtonBuilder {
    fn into_parts(self) -> (Button, Vec<HandlerRegistration>) {
        (
            Button::assemble(self.text, self.style_set, self.disabled, self.block),
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

    pub fn on_click<F: FnMut() + 'static>(mut self, mut f: F) -> Self {
        self.handlers.push(HandlerRegistration::new(
            SemanticKind::Click,
            Box::new(move |_| f()),
        ));
        self
    }

    pub fn on_click_capture<T, F>(mut self, state: &crate::ui::state::State<T>, mut f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut() + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(SemanticKind::Click, Box::new(move |_| f()))
                .with_state_capture(state),
        );
        self
    }

    pub fn on_click_window_capture<F>(mut self, window_id: crate::core::WindowId, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(SemanticKind::Click, Box::new(move |_| f()))
                .with_window_capture(window_id),
        );
        self
    }

    pub fn on_click_event<F: FnMut(&mut SemanticEvent) + 'static>(mut self, f: F) -> Self {
        self.handlers
            .push(HandlerRegistration::new(SemanticKind::Click, Box::new(f)));
        self
    }

    pub fn on_click_event_capture<T, F>(mut self, state: &crate::ui::state::State<T>, f: F) -> Self
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
/// button("保存").primary().on_click(|| save())
/// ```
pub fn button(text: impl Into<String>) -> ButtonBuilder {
    ButtonBuilder {
        text: text.into(),
        style_set: StyleSet::button_default(),
        disabled: false,
        block: false,
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
    handlers: Vec<HandlerRegistration>,
}

impl InputBuilder {
    /// 设置占位文本。
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
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

    pub fn on_change_capture<T, F>(mut self, state: &crate::ui::state::State<T>, mut f: F) -> Self
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
        let mut node = ViewNode::leaf(crate::ui::widgets::Input::new(self.placeholder));
        node.handlers = self.handlers;
        node
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
        handlers: Vec::new(),
    }
}

#[cfg(test)]
#[path = "../../tests/ui/view/combinators.rs"]
mod tests;
