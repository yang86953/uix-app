//! # 用户层简化 API — View 体系
//!
//! 本模块提供函数式组合子 API，让用户通过 `View` trait 和 `ViewNode` 声明 UI，
//! 完全不需要了解 `WidgetTree`、`WidgetNode`、`BoxedWidget` 等内部概念。

use crate::core::{EdgeInsets, Point, Rect};
use crate::draw::Color;
use crate::ui::event::{HandlerRegistration, SemanticEvent, SemanticKind};
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::style::{BoxShadowDef, ColorValue, Style, TypographyToken};
use crate::ui::traits::WidgetComponent;

pub(crate) mod adapter;
pub mod combinators;

pub use crate::ui::window_chrome::{
    window_control, window_control_named, window_drag_region, WindowControl,
};
pub(crate) use adapter::ViewAdapter;
pub use combinators::{
    button, canvas, column, column_fit, dynamic_label, embed, grid, input, label, row, scroll,
    show, space, ButtonBuilder, GridBuilder, InputBuilder, IntoLabelContent, IntoViewChildren,
    ScrollBuilder,
};

/// 用户层 UI 声明 trait。
pub trait View: 'static {
    fn build(self) -> ViewNode;
}

/// 中间节点表示。
pub struct ViewNode {
    pub(crate) widget: Box<dyn WidgetComponent>,
    pub(crate) children: Vec<ViewNode>,
    pub(crate) style: Style,
    /// DSL 显式设置的 flex_grow（含 0.0）；与 Style::default 区分，避免被 apply 吞掉。
    pub(crate) flex_grow_override: Option<f32>,
    /// DSL 显式设置的 flex_shrink（含 1.0）。
    pub(crate) flex_shrink_override: Option<f32>,
    pub(crate) z_index: i32,
    pub(crate) key: Option<String>,
    pub(crate) automation_id: Option<String>,
    pub(crate) handlers: Vec<HandlerRegistration>,
    pub(crate) render_handlers: Vec<RenderHandlerRegistration>,
}

impl View for ViewNode {
    fn build(self) -> ViewNode {
        self
    }
}

impl ViewNode {
    pub(crate) fn widget_type_id(&self) -> std::any::TypeId {
        self.widget.as_any().type_id()
    }

    pub fn leaf(widget: impl WidgetComponent + 'static) -> Self {
        Self {
            widget: Box::new(widget),
            children: vec![],
            style: Style::default(),
            flex_grow_override: None,
            flex_shrink_override: None,
            z_index: 0,
            key: None,
            automation_id: None,
            handlers: Vec::new(),
            render_handlers: Vec::new(),
        }
    }

    pub fn new(widget: impl WidgetComponent + 'static, children: Vec<ViewNode>) -> Self {
        Self {
            widget: Box::new(widget),
            children,
            style: Style::default(),
            flex_grow_override: None,
            flex_shrink_override: None,
            z_index: 0,
            key: None,
            automation_id: None,
            handlers: Vec::new(),
            render_handlers: Vec::new(),
        }
    }

    pub fn color(mut self, color: impl Into<ColorValue>) -> Self {
        self.style.color = color.into();
        self
    }

    pub fn font_size(mut self, size: impl Into<TypographyToken>) -> Self {
        self.style.font_size = size.into();
        self
    }

    pub fn bg(mut self, color: impl Into<ColorValue>) -> Self {
        self.style.background = Some(color.into());
        self
    }

    pub fn padding(mut self, p: impl Into<EdgeInsets>) -> Self {
        self.style.padding = p.into();
        self
    }

    /// 设置水平内边距，并保留已有的垂直内边距。
    pub fn padding_h(mut self, value: f32) -> Self {
        self.style.padding.left = value;
        self.style.padding.right = value;
        self
    }

    /// 设置垂直内边距，并保留已有的水平内边距。
    pub fn padding_v(mut self, value: f32) -> Self {
        self.style.padding.top = value;
        self.style.padding.bottom = value;
        self
    }

    pub fn margin(mut self, m: impl Into<EdgeInsets>) -> Self {
        self.style.margin = m.into();
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.style.width = Some(w);
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.style.height = Some(h);
        self
    }

    pub fn flex_grow(mut self, g: f32) -> Self {
        self.style.flex_grow = g;
        self.flex_grow_override = Some(g);
        self
    }

    pub fn flex_shrink(mut self, s: f32) -> Self {
        self.style.flex_shrink = s;
        self.flex_shrink_override = Some(s);
        self
    }

    /// 交叉轴对齐（flex `align-items`）。
    pub fn align(mut self, a: crate::ui::layout::AlignItems) -> Self {
        self.style.align_items = a;
        self
    }

    pub fn align_self(mut self, a: crate::ui::layout::AlignItems) -> Self {
        self.style.align_self = Some(a);
        self
    }

    pub fn grid_cell(mut self, cell: usize) -> Self {
        self.style.grid_cell = Some(cell);
        self
    }

    pub fn grid_span(mut self, columns: u32, rows: u32) -> Self {
        self.style.grid_column_span = columns.max(1);
        self.style.grid_row_span = rows.max(1);
        self
    }

    pub fn gap(mut self, g: f32) -> Self {
        self.style.gap = g;
        self
    }

    /// 保留子项的自然主轴尺寸，用于 ScrollView 的内容容器。
    pub fn overflow_content(mut self) -> Self {
        self.style.overflow_content = true;
        self
    }

    pub fn border(mut self, width: f32, color: impl Into<ColorValue>) -> Self {
        self.style.border_width = EdgeInsets::uniform(width);
        self.style.border_color = Some(color.into());
        self
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.style.border_radius = r;
        self
    }

    pub fn opacity(mut self, o: f32) -> Self {
        self.style.opacity = o;
        self
    }

    /// 设置以节点边界为中心、无偏移的盒阴影。
    pub fn shadow(mut self, blur: f32, color: impl Into<Color>) -> Self {
        self.style.box_shadow = Some(BoxShadowDef::new(color.into(), blur, 0.0, 0.0));
        self
    }

    pub fn z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }

    pub fn key(mut self, k: impl Into<String>) -> Self {
        self.key = Some(k.into());
        self
    }

    /// Assigns a stable selector for test automation without affecting
    /// reconciliation identity or runtime behavior.
    pub fn automation_id(mut self, id: impl Into<String>) -> Self {
        self.automation_id = Some(id.into());
        self
    }

    pub fn on_semantic(
        mut self,
        kind: SemanticKind,
        handler: impl FnMut(&mut SemanticEvent) + 'static,
    ) -> Self {
        self.handlers
            .push(HandlerRegistration::new(kind, Box::new(handler)));
        self
    }

    pub fn on_semantic_capture<T>(
        mut self,
        kind: SemanticKind,
        state: &crate::ui::state::State<T>,
        handler: impl FnMut(&mut SemanticEvent) + 'static,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        self.handlers
            .push(HandlerRegistration::new(kind, Box::new(handler)).with_state_capture(state));
        self
    }

    pub fn on_semantic_computed_capture<T>(
        mut self,
        kind: SemanticKind,
        computed: &crate::ui::state::Computed<T>,
        handler: impl FnMut(&mut SemanticEvent) + 'static,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(kind, Box::new(handler)).with_computed_capture(computed),
        );
        self
    }

    pub fn on_semantic_window_capture(
        mut self,
        kind: SemanticKind,
        window_id: crate::core::WindowId,
        handler: impl FnMut(&mut SemanticEvent) + 'static,
    ) -> Self {
        self.handlers
            .push(HandlerRegistration::new(kind, Box::new(handler)).with_window_capture(window_id));
        self
    }

    /// 默认点击路径：绑定 `State` 指纹，reconcile 可稳定复用（[使用](docs/使用.md) · [使用](docs/使用.md)）。
    ///
    /// 与 [`button`](combinators::button) 的 `on_click` 对齐，可用于 `label` / `embed` 等任意 View。
    pub fn on_click<T, F>(mut self, state: &crate::ui::state::State<T>, mut f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut(&crate::ui::state::State<T>) + 'static,
    {
        let captured = state.clone();
        self.handlers.push(
            HandlerRegistration::new(SemanticKind::Click, Box::new(move |_| f(&captured)))
                .with_state_capture(state),
        );
        self
    }

    /// 无 State 的点击闭包；每次 reconcile **保守重绑**（[使用](docs/使用.md)）。
    ///
    /// 命名保留 `_fn`：Rust 无法与 [`Self::on_click`] 重载；有 State 时优先 `on_click(&state, …)`（[使用](docs/使用.md)）。
    pub fn on_click_fn<F: FnMut() + 'static>(mut self, mut f: F) -> Self {
        self.handlers.push(HandlerRegistration::new(
            SemanticKind::Click,
            Box::new(move |_| f()),
        ));
        self
    }

    /// 兼容别名：指纹同 [`Self::on_click`]，闭包不接收 `&State`。
    ///
    /// 新代码优先 `on_click(&state, |s| …)`；无 State 用 [`Self::on_click_fn`]。
    #[doc(alias = "on_click")]
    pub fn on_click_capture<T, F>(self, state: &crate::ui::state::State<T>, mut f: F) -> Self
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

impl crate::ui::IntoWidgetNode for ViewNode {
    fn into_node(self) -> crate::ui::core::widget::WidgetNode {
        adapter::ViewAdapter::expand(self)
    }
}

/// 为所有 `Into<ViewNode>` 类型提供样式链（[使用](docs/使用.md)）。
///
/// 实现委托 [`ViewNode`] 同名方法，避免双份逻辑漂移。
/// **链式顺序**：先写 builder 专有方法（如 `button(…).primary().on_click(…)`），再写本 trait
/// （`bg` / `padding`…）——一旦进入 `ViewNode`，`primary` 等 builder 方法不可再调。
pub trait StyleExt: Into<ViewNode> + Sized {
    /// Assigns a stable selector for test automation. Put builder-specific
    /// methods before this call because it materializes a [`ViewNode`].
    fn automation_id(self, id: impl Into<String>) -> ViewNode {
        self.into().automation_id(id)
    }

    fn color(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().color(color)
    }

    fn font_size(self, size: impl Into<TypographyToken>) -> ViewNode {
        self.into().font_size(size)
    }

    fn bg(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().bg(color)
    }

    fn padding(self, p: impl Into<EdgeInsets>) -> ViewNode {
        self.into().padding(p)
    }

    fn margin(self, m: impl Into<EdgeInsets>) -> ViewNode {
        self.into().margin(m)
    }

    fn width(self, w: f32) -> ViewNode {
        self.into().width(w)
    }

    fn height(self, h: f32) -> ViewNode {
        self.into().height(h)
    }

    fn flex_grow(self, g: f32) -> ViewNode {
        self.into().flex_grow(g)
    }

    fn flex_shrink(self, s: f32) -> ViewNode {
        self.into().flex_shrink(s)
    }

    fn align(self, a: crate::ui::layout::AlignItems) -> ViewNode {
        self.into().align(a)
    }

    fn align_self(self, a: crate::ui::layout::AlignItems) -> ViewNode {
        self.into().align_self(a)
    }

    fn grid_cell(self, cell: usize) -> ViewNode {
        self.into().grid_cell(cell)
    }

    fn grid_span(self, columns: u32, rows: u32) -> ViewNode {
        self.into().grid_span(columns, rows)
    }

    fn gap(self, g: f32) -> ViewNode {
        self.into().gap(g)
    }

    /// 保留子项的自然主轴尺寸，用于 ScrollView 的内容容器。
    fn overflow_content(self) -> ViewNode {
        self.into().overflow_content()
    }

    fn border(self, width: f32, color: impl Into<ColorValue>) -> ViewNode {
        self.into().border(width, color)
    }

    fn radius(self, r: f32) -> ViewNode {
        self.into().radius(r)
    }

    fn opacity(self, o: f32) -> ViewNode {
        self.into().opacity(o)
    }
}

impl<T: Into<ViewNode>> StyleExt for T {}

/// 简化渲染上下文——用户层进行自定义绘制时使用的 API。
pub struct Ui<'a> {
    pub(crate) fill_rect_fn: Option<Box<dyn FnMut(Rect, Color, Option<f32>) + 'a>>,
    pub(crate) stroke_rect_fn: Option<Box<dyn FnMut(Rect, Color, f32) + 'a>>,
    pub(crate) text_fn: Option<Box<dyn FnMut(&str, Point, Color, f32) + 'a>>,
    pub(crate) text_center_fn: Option<Box<dyn FnMut(&str, Rect, Color, f32) + 'a>>,
    pub(crate) color_fn: Option<Box<dyn FnMut(&str) -> Color + 'a>>,
}

impl<'a> Ui<'a> {
    pub fn fill_rect(&mut self, rect: Rect, color: impl Into<Color>, radius: Option<f32>) {
        if let Some(ref mut f) = self.fill_rect_fn {
            f(rect, color.into(), radius);
        }
    }

    pub fn stroke_rect(&mut self, rect: Rect, color: impl Into<Color>, width: f32) {
        if let Some(ref mut f) = self.stroke_rect_fn {
            f(rect, color.into(), width);
        }
    }

    pub fn text(&mut self, text: &str, pos: Point, color: impl Into<Color>, font_size: f32) {
        if let Some(ref mut f) = self.text_fn {
            f(text, pos, color.into(), font_size);
        }
    }

    pub fn text_center(&mut self, text: &str, rect: Rect, color: impl Into<Color>, font_size: f32) {
        if let Some(ref mut f) = self.text_center_fn {
            f(text, rect, color.into(), font_size);
        }
    }

    pub fn color(&mut self, name: &str) -> Color {
        if let Some(ref mut f) = self.color_fn {
            f(name)
        } else {
            Color::black()
        }
    }
}
