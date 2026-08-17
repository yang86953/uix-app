//! 响应式文本标签 — 文本闭包绑定 State → Paint 失效的框架级组件。
//!
//! # SMC 边界（SMC-04）
//!
//! `DynamicLabel` 是框架级基础设施（原 `view::combinators::DynamicLabel`）：
//! 树（tree_dirty / 无障碍）需要在 build 期识别并绑定其 State 依赖，归
//! component Module 避免 component → view / widgets 依赖。

use std::any::Any;
use std::sync::Arc;

use crate::core::{ComponentId, Constraints, Rect, Size};
use crate::draw::renderer::InvalidationQueueHandle;
use crate::draw::scene::PicturePolicy;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::traits::{
    WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetRender,
};
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::reactive::state::StatePaintBind;
use crate::ui::theme::style::Style;

/// 响应式标签的内部 Widget 实现。
pub(crate) struct DynamicLabel {
    text_fn: Box<dyn Fn() -> String>,
    state_sources: Vec<Arc<dyn StatePaintBind>>,
    style: Option<Style>,
}

impl DynamicLabel {
    pub(crate) fn new<F: Fn() -> String + 'static>(f: F) -> Self {
        Self {
            text_fn: Box::new(f),
            state_sources: Vec::new(),
            style: None,
        }
    }

    pub(crate) fn set_style(&mut self, style: Style) {
        self.style = Some(style);
    }

    /// 将关联 State 绑定到 widget 的 Paint 失效。
    pub(crate) fn bind_state_invalidation(
        &self,
        component_id: ComponentId,
        queue: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        for source in &self.state_sources {
            // 仅 Paint：文本闭包在 render 时取值；绑 reconcile 会让 timer State
            // 每秒触发整树 reconcile（叠加 layout 振荡即周期性卡顿）。
            source.bind_paint(component_id, queue.clone(), rect);
        }
    }

    /// layout 后探测闭包依赖：执行一次文本闭包以捕获 `State::get()`。
    pub(crate) fn probe_dependencies(&self) {
        let _ = (self.text_fn)();
    }

    pub(crate) fn semantic_text(&self) -> String {
        (self.text_fn)()
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

    fn layout_margin(&self) -> crate::core::EdgeInsets {
        self.style.as_ref().map(|s| s.margin).unwrap_or_default()
    }

    fn align_self(&self) -> Option<crate::ui::layout::AlignItems> {
        self.style.as_ref().and_then(|s| s.align_self)
    }

    fn flex_grow(&self) -> f32 {
        self.style.as_ref().map(|s| s.flex_grow).unwrap_or(0.0)
    }

    fn flex_shrink(&self) -> f32 {
        self.style.as_ref().map(|s| s.flex_shrink).unwrap_or(0.0)
    }
}

impl DynamicLabel {
    fn intrinsic_size(&self) -> Size {
        let pad = self.style.as_ref().map(|s| s.padding).unwrap_or_default();
        if let Some(style) = &self.style {
            if let (Some(w), Some(h)) = (style.width, style.height) {
                return Size::new(w, h);
            }
        }
        let text = (self.text_fn)();
        let fs = self
            .style
            .as_ref()
            .map(|s| s.font_size.default_size())
            .unwrap_or(14.0);
        let h = self
            .style
            .as_ref()
            .and_then(|s| s.height)
            .unwrap_or(fs * 1.5 + pad.vertical());
        let w = self
            .style
            .as_ref()
            .and_then(|s| s.width)
            .unwrap_or(text.len() as f32 * 7.0 + pad.horizontal());
        Size::new(w, h)
    }
}

impl WidgetRender for DynamicLabel {
    fn render(&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let text = (self.text_fn)();
        if text.is_empty() {
            return;
        }
        let style = self.style.as_ref();
        let color = style
            .map(|s| s.resolve_color(ctx.tokens()))
            .unwrap_or_else(|| ctx.tokens().color_text());
        let font_size = style
            .map(|s| s.resolve_font_size(ctx.tokens()))
            .unwrap_or(14.0);
        let padding = style.map(|s| s.padding).unwrap_or_default();
        ctx.draw_text(
            &text,
            crate::core::Point::new(frame.x + padding.left, frame.y + padding.top),
            color,
            font_size,
        );
    }
}

// 仅在测试构建中验证动态文本的响应式生命周期契约。
#[cfg(test)]
mod tests {
    // 引入被测 DynamicLabel 私有实现。
    use super::DynamicLabel;
    // 引入精确组件身份与绘制矩形。
    use crate::core::{ComponentId, Rect};
    // 引入共享失效队列。
    use crate::draw::renderer::InvalidationQueue;
    // 引入动态闭包依赖探测入口。
    use crate::ui::reactive::state::{begin_state_bind_capture, end_state_bind_capture};
    // 引入公开响应式状态。
    use crate::ui::State;

    // 验证动态文本读取最新语义并只推送自身矩形的 Paint 失效。
    #[test]
    fn state_change_invalidates_only_dynamic_label_paint() {
        // 创建由测试调用方拥有的响应式值。
        let value = State::new(1_u32);
        // 克隆同一状态槽供动态文本闭包拥有。
        let source = value.clone();
        // 创建每次读取当前值的动态标签。
        let label = DynamicLabel::new(move || format!("tick: {}", source.get()));
        // 创建独立树失效队列。
        let queue = InvalidationQueue::shared();
        // 使用非根组件身份验证失效范围。
        let component_id = ComponentId::new(7);
        // 指定动态文本自身的已布局矩形。
        let paint_rect = Rect::new(12.0, 20.0, 80.0, 24.0);
        // 开始捕获闭包内 State::get 依赖。
        begin_state_bind_capture(component_id, queue.clone(), Some(paint_rect));
        // 执行一次与布局后绑定相同的依赖探测。
        label.probe_dependencies();
        // 将捕获依赖绑定为精确 Paint 失效。
        end_state_bind_capture(component_id);
        // 初始无障碍语义必须读取当前文本。
        assert_eq!(label.semantic_text(), "tick: 1");
        // 更新 State 应触发已绑定动态文本节点。
        value.set(2);
        // 无障碍语义必须立即反映最新状态。
        assert_eq!(label.semantic_text(), "tick: 2");
        // 读取状态更新产生的失效证据。
        let queue = queue.lock().expect("动态文本失效队列应可读取");
        // 动态文本值变化不得请求结构布局。
        assert!(!queue.has_layout());
        // 只有绑定的动态文本组件需要绘制。
        assert!(queue.node_needs_paint(component_id));
        // 已知组件矩形不得退化为全帧绘制。
        assert!(!queue.needs_full_frame());
    }
}
