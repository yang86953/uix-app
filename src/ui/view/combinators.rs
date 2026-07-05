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

use crate::ui::layout::FlexDirection;
use crate::ui::view::{View, ViewNode};

use crate::ui::traits::{WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetRender};
use crate::draw::painting::RenderContext;
use crate::ui::state::{drain_pending_state_binds, StatePaintBind};
use crate::ui::{WidgetId, WidgetTree};
use std::any::Any;
use std::sync::Arc;
use crate::draw::pipeline::InvalidationQueueHandle;
use crate::native::{Rect, Size};

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
    ) {
        for source in &self.state_sources {
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
    fn capabilities(&self) -> WidgetCapabilities {
        let mut c = WidgetCapabilities::new();
        c.insert(WidgetCapabilities::LAYOUT);
        c.insert(WidgetCapabilities::RENDER);
        c
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
    fn preferred_size(&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        let text = (self.text_fn)();
        let len = text.len() as f32;
        Size::new(len * 7.0, 18.0)
    }
}

impl WidgetRender for DynamicLabel {
    fn render(&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let text = (self.text_fn)();
        if !text.is_empty() {
            ctx.draw_text(
                &text,
                crate::native::Point::new(frame.x, frame.y),
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

// ── 按钮构建器 ──────────────────────────────────────────────

/// 按钮构建器。通过 `button("text").primary().on_click(fn)` 链式调用。
///
/// # 示例
///
/// ```ignore
/// button("提交").primary().on_click(|| println!("提交成功"))
/// ```
pub struct ButtonBuilder {
    text: String,
    variant: crate::ui::widgets::ButtonVariant,
    danger: bool,
    on_click: Option<Box<dyn FnMut() + 'static>>,
}

impl ButtonBuilder {
    /// 设置为主要按钮样式。
    pub fn primary(mut self) -> Self {
        self.variant = crate::ui::widgets::ButtonVariant::Primary;
        self
    }

    /// 设置为危险按钮样式（红色提示）。
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    /// 绑定点击回调。
    pub fn on_click<F: FnMut() + 'static>(mut self, f: F) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl View for ButtonBuilder {
    fn build(self) -> ViewNode {
        let mut btn = crate::ui::widgets::Button::new(self.text).variant(self.variant);
        if self.danger {
            btn = btn.danger(true);
        }
        if let Some(f) = self.on_click {
            btn = btn.on_click(f);
        }
        ViewNode::leaf(btn)
    }
}

/// 创建按钮。返回 `ButtonBuilder` 以链式设置属性。
///
/// # 示例
///
/// ```ignore
/// button("保存").primary().on_click(|| save_data())
/// ```
pub fn button(text: impl Into<String>) -> ButtonBuilder {
    ButtonBuilder {
        text: text.into(),
        variant: crate::ui::widgets::ButtonVariant::Default,
        danger: false,
        on_click: None,
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
    on_change: Option<Box<dyn FnMut(&str) + 'static>>,
}

impl InputBuilder {
    /// 设置占位文本。
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// 绑定值变更回调（输入内容变化时触发）。
    pub fn on_change<F: FnMut(&str) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}

impl View for InputBuilder {
    fn build(self) -> ViewNode {
        let mut input = crate::ui::widgets::Input::new(self.placeholder);
        if let Some(f) = self.on_change {
            input = input.on_change(f);
        }
        ViewNode::leaf(input)
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
        on_change: None,
    }
}
