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

use uix_platform::EdgeInsets;
use uix_graphics::Color;
use crate::style::Style;
use crate::api::traits::WidgetComponent;

// ── 子模块（由其他 Agent 实现）──
pub mod combinators;
pub mod adapter;
pub mod app;

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
    pub fn new(
        widget: impl WidgetComponent + 'static,
        children: Vec<ViewNode>,
    ) -> Self {
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
