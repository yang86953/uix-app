//! # 用户层简化 API — View 体系
//!
//! 本模块提供函数式组合子 API，让用户通过 `View` trait 和 `ViewNode` 声明 UI，
//! 完全不需要了解 `WidgetTree`、`WidgetNode`、`BoxedWidget` 等内部概念。

use crate::core::{EdgeInsets, Point, Rect};
use crate::draw::Color;
use crate::ui::event::{HandlerRegistration, SemanticEvent, SemanticKind};
use crate::ui::style::{ColorValue, Style, TypographyToken};
use crate::ui::traits::WidgetComponent;

pub mod adapter;
pub mod combinators;

pub use adapter::ViewAdapter;
pub use combinators::{
    button, column, dynamic_label, grid, input, label, row, scroll, space, ButtonBuilder,
    GridBuilder, InputBuilder, ScrollBuilder,
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
    pub(crate) z_index: i32,
    pub(crate) key: Option<String>,
    pub(crate) handlers: Vec<HandlerRegistration>,
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
            z_index: 0,
            key: None,
            handlers: Vec::new(),
        }
    }

    pub fn new(widget: impl WidgetComponent + 'static, children: Vec<ViewNode>) -> Self {
        Self {
            widget: Box::new(widget),
            children,
            style: Style::default(),
            z_index: 0,
            key: None,
            handlers: Vec::new(),
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
        self
    }

    pub fn gap(mut self, g: f32) -> Self {
        self.style.gap = g;
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

    pub fn z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }

    pub fn key(mut self, k: impl Into<String>) -> Self {
        self.key = Some(k.into());
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
}

impl crate::ui::IntoWidgetNode for ViewNode {
    fn into_node(self) -> crate::ui::core::widget::WidgetNode {
        adapter::ViewAdapter::expand(self)
    }
}

/// 为所有实现了 `Into<ViewNode>` 的类型提供链式样式设置方法。
pub trait StyleExt: Into<ViewNode> + Sized {
    fn color(self, color: impl Into<ColorValue>) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.color = color.into();
        node
    }

    fn font_size(self, size: impl Into<TypographyToken>) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.font_size = size.into();
        node
    }

    fn bg(self, color: impl Into<ColorValue>) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.background = Some(color.into());
        node
    }

    fn padding(self, p: impl Into<EdgeInsets>) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.padding = p.into();
        node
    }

    fn margin(self, m: impl Into<EdgeInsets>) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.margin = m.into();
        node
    }

    fn width(self, w: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.width = Some(w);
        node
    }

    fn height(self, h: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.height = Some(h);
        node
    }

    fn flex_grow(self, g: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.flex_grow = g;
        node
    }

    fn flex_shrink(self, s: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.flex_shrink = s;
        node
    }

    fn gap(self, g: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.gap = g;
        node
    }

    fn border(self, width: f32, color: impl Into<ColorValue>) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.border_width = EdgeInsets::uniform(width);
        node.style.border_color = Some(color.into());
        node
    }

    fn radius(self, r: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.border_radius = r;
        node
    }

    fn opacity(self, o: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.opacity = o;
        node
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
