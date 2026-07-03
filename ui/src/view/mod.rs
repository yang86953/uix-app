//! # 用户层简化 API — View 体系
//!
//! 本模块提供函数式组合子 API，让用户通过 `View` trait 和 `ViewNode` 声明 UI，
//! 完全不需要了解 `WidgetTree`、`WidgetNode`、`BoxedWidget` 等内部概念。
//!
//! ## 设计原则
//!
//! - `View` 是用户层 UI 声明的核心 trait，所有组件（label、button、column 等）都实现它。
//! - `ViewNode` 是中间表示，用户通过链式 Builder 方法设置样式，语法类似 SwiftUI。
//! - 适配器（`adapter` 模块）将 ViewNode 树展开为 WidgetTree，用户无感知。
//!
//! ## 使用示意
//!
//! ```ignore
//! use uix_ui::view::*;
//!
//! let ui = column((
//!     label("Hello").font_size(24.0).color(Color::blue()),
//!     button("点击").primary().on_click(|| println!("clicked")),
//! )).padding(16.0);
//! ```

use crate::api::traits::WidgetComponent;
use crate::style::Style;
use uix_graphics::Color;
use uix_platform::{EdgeInsets, Point, Rect};

// ── 子模块 ──
pub mod adapter;
pub mod app;
pub mod combinators;

// ── 便利重导出：用户只需 `use uix::ui::view::*` ──
pub use adapter::ViewAdapter;
pub use app::App;
pub use combinators::{
    button, column, dynamic_label, input, label, row, space, ButtonBuilder, InputBuilder,
};

/// 用户层 UI 声明 trait。
///
/// 所有组件（label、button、column 等）都实现此 trait。
/// 调用 `build()` 展开为 `ViewNode` 中间表示，再通过适配器转为 `WidgetTree`。
pub trait View: 'static {
    /// 将 View 展开为 ViewNode（中间表示）。
    fn build(self) -> ViewNode;
}

/// 中间节点表示。
///
/// 适配器将 ViewNode 树展开为 WidgetTree，用户不需要接触 `WidgetNode` 等概念。
///
/// 支持链式 Builder 方法设置样式：
///
/// ```ignore
/// let node = ViewNode::leaf(label("Hello"))
///     .font_size(24.0)
///     .color(Color::blue())
///     .padding(EdgeInsets::all(8.0));
/// ```
pub struct ViewNode {
    /// 组件实例（如 Label::new("..."), Button::new("...")）
    pub(crate) widget: Box<dyn WidgetComponent>,
    /// 子节点
    pub(crate) children: Vec<ViewNode>,
    /// 样式覆盖（适配器负责应用到 widget）
    pub(crate) style: Style,
    /// 叠加顺序
    pub(crate) z_index: i32,
    /// 可选的 key
    pub(crate) key: Option<String>,
}

impl View for ViewNode {
    fn build(self) -> ViewNode {
        self
    }
}

impl ViewNode {
    /// 创建一个叶子节点（无子节点）。
    pub fn leaf(widget: impl WidgetComponent + 'static) -> Self {
        Self {
            widget: Box::new(widget),
            children: vec![],
            style: Style::default(),
            z_index: 0,
            key: None,
        }
    }

    /// 创建一个父节点，包含子节点列表。
    pub fn new(widget: impl WidgetComponent + 'static, children: Vec<ViewNode>) -> Self {
        Self {
            widget: Box::new(widget),
            children,
            style: Style::default(),
            z_index: 0,
            key: None,
        }
    }

    // ── 通用样式 builder 方法 ──────────────────────────────

    /// 设置文字颜色。
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.style.color = color.into();
        self
    }

    /// 设置字号。
    pub fn font_size(mut self, size: f32) -> Self {
        self.style.font_size = size;
        self
    }

    /// 设置背景色。
    pub fn bg(mut self, color: impl Into<Color>) -> Self {
        self.style.background = Some(color.into());
        self
    }

    /// 设置内边距。
    pub fn padding(mut self, p: impl Into<EdgeInsets>) -> Self {
        self.style.padding = p.into();
        self
    }

    /// 设置固定宽度。
    pub fn width(mut self, w: f32) -> Self {
        self.style.width = Some(w);
        self
    }

    /// 设置固定高度。
    pub fn height(mut self, h: f32) -> Self {
        self.style.height = Some(h);
        self
    }

    /// 设置 Flex 扩展比例。
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.style.flex_grow = g;
        self
    }

    /// 设置子项间距。
    pub fn gap(mut self, g: f32) -> Self {
        self.style.gap = g;
        self
    }

    /// 设置边框（宽度 + 颜色）。
    pub fn border(mut self, width: f32, color: impl Into<Color>) -> Self {
        self.style.border_width = width;
        self.style.border_color = Some(color.into());
        self
    }

    /// 设置边框圆角。
    pub fn radius(mut self, r: f32) -> Self {
        self.style.border_radius = r;
        self
    }

    /// 设置叠加顺序（值越大越靠前）。
    pub fn z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }

    /// 设置可选的 key（用于 diff / 状态保持）。
    pub fn key(mut self, k: impl Into<String>) -> Self {
        self.key = Some(k.into());
        self
    }
}

// ── StyleExt —— 统一链式样式设置 trait ──────────────────────────

/// 为所有实现了 `Into<ViewNode>` 的类型提供链式样式设置方法。
///
/// 通过 blanket impl 自动应用到 `ViewNode`（及其派生类型 `ButtonBuilder`、`InputBuilder` 等）。
///
/// # 示例
///
/// ```ignore
/// label("Hello")
///     .color("#333")
///     .font_size(16.0)
///     .padding(8.0)
///     .bg(Color::white());
/// ```
pub trait StyleExt: Into<ViewNode> + Sized {
    /// 设置文字颜色。
    fn color(self, color: impl Into<Color>) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.color = color.into();
        node
    }

    /// 设置字号。
    fn font_size(self, size: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.font_size = size;
        node
    }

    /// 设置背景色。
    fn bg(self, color: impl Into<Color>) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.background = Some(color.into());
        node
    }

    /// 设置内边距。
    fn padding(self, p: impl Into<EdgeInsets>) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.padding = p.into();
        node
    }

    /// 设置外边距。
    fn margin(self, m: impl Into<EdgeInsets>) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.margin = m.into();
        node
    }

    /// 设置固定宽度。
    fn width(self, w: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.width = Some(w);
        node
    }

    /// 设置固定高度。
    fn height(self, h: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.height = Some(h);
        node
    }

    /// 设置 Flex 扩展比例。
    fn flex_grow(self, g: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.flex_grow = g;
        node
    }

    /// 设置 Flex 收缩比例。
    fn flex_shrink(self, s: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.flex_shrink = s;
        node
    }

    /// 设置子项间距。
    fn gap(self, g: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.gap = g;
        node
    }

    /// 设置边框（宽度 + 颜色）。
    fn border(self, width: f32, color: impl Into<Color>) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.border_width = width;
        node.style.border_color = Some(color.into());
        node
    }

    /// 设置边框圆角。
    fn radius(self, r: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.border_radius = r;
        node
    }

    /// 设置透明度（0.0 ~ 1.0）。
    fn opacity(self, o: f32) -> ViewNode {
        let mut node: ViewNode = self.into();
        node.style.opacity = o;
        node
    }
}

// Blanket impl: 所有 Into<ViewNode> 的类型自动获得 StyleExt 方法
impl<T: Into<ViewNode>> StyleExt for T {}

// ── Ui —— 用户层简化渲染上下文 ─────────────────────────────────

/// 简化渲染上下文——用户层进行自定义绘制时使用的 API。
///
/// 提供 8 个高频绘制方法，内部委托给 `RenderContext`。用户不需要了解
/// `Canvas2D`、`SpatialContext`、`TextRenderService` 等底层概念。
///
/// # 获取方式
///
/// 在 `View::render()` 中通过参数获得：
/// ```ignore
/// impl View for MyWidget {
///     fn render(&self, ui: &mut Ui) {
///         ui.fill_rect(rect, Color::red());
///         ui.text("Hello", pos, Color::black(), 14.0);
///     }
/// }
/// ```
pub struct Ui<'a> {
    pub(crate) fill_rect_fn: Option<Box<dyn FnMut(Rect, Color, Option<f32>) + 'a>>,
    pub(crate) stroke_rect_fn: Option<Box<dyn FnMut(Rect, Color, f32) + 'a>>,
    pub(crate) text_fn: Option<Box<dyn FnMut(&str, Point, Color, f32) + 'a>>,
    pub(crate) text_center_fn: Option<Box<dyn FnMut(&str, Rect, Color, f32) + 'a>>,
    pub(crate) color_fn: Option<Box<dyn FnMut(&str) -> Color + 'a>>,
}

impl<'a> Ui<'a> {
    /// 填充矩形区域。
    pub fn fill_rect(&mut self, rect: Rect, color: impl Into<Color>, radius: Option<f32>) {
        if let Some(ref mut f) = self.fill_rect_fn {
            f(rect, color.into(), radius);
        }
    }

    /// 描边矩形。
    pub fn stroke_rect(&mut self, rect: Rect, color: impl Into<Color>, width: f32) {
        if let Some(ref mut f) = self.stroke_rect_fn {
            f(rect, color.into(), width);
        }
    }

    /// 绘制文本。
    pub fn text(&mut self, text: &str, pos: Point, color: impl Into<Color>, font_size: f32) {
        if let Some(ref mut f) = self.text_fn {
            f(text, pos, color.into(), font_size);
        }
    }

    /// 在矩形区域内居中绘制文本。
    pub fn text_center(&mut self, text: &str, rect: Rect, color: impl Into<Color>, font_size: f32) {
        if let Some(ref mut f) = self.text_center_fn {
            f(text, rect, color.into(), font_size);
        }
    }

    /// 通过名称获取主题色（如 "primary"、"bg"、"text"）。
    /// 返回 Color 或默认黑色。
    pub fn color(&mut self, name: &str) -> Color {
        if let Some(ref mut f) = self.color_fn {
            f(name)
        } else {
            Color::black()
        }
    }
}
