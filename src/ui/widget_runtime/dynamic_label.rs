//! 响应式文本标签 — 文本闭包绑定 State → Paint 失效的框架级组件。
//!
//! # SMC 边界（SMC-04）
//!
//! `DynamicLabel` 是框架级基础设施（原 `view::combinators::DynamicLabel`）：
//! 树（tree_dirty / 无障碍）需要在 build 期识别并绑定其 State 依赖，归
//! component Module 避免 component → view / widgets 依赖。

use std::any::Any;

use crate::core::{Constraints, Rect, Size};
use crate::draw::scene::PicturePolicy;
use crate::ui::theme::style::Style;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::traits::{
    WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetRender,
};
use crate::ui::widget_runtime::widget::WidgetTree;

/// 响应式标签的内部 Widget 实现。
pub(crate) struct DynamicLabel {
    text_fn: Box<dyn Fn() -> String>,
    style: Option<Style>,
}

impl DynamicLabel {
    pub(crate) fn new<F: Fn() -> String + 'static>(f: F) -> Self {
        Self {
            text_fn: Box::new(f),
            style: None,
        }
    }

    pub(crate) fn set_style(&mut self, style: Style) {
        self.style = Some(style);
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
    use crate::ui::reactive::state::StateBindCaptureGuard;
    // 引入实际组件树以验证移除与窗口关闭边界。
    use crate::ui::widget_runtime::widget::WidgetTree;
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
        // 开始捕获闭包内 State::get 依赖并建立异常清理边界。
        let capture = StateBindCaptureGuard::begin(component_id, queue.clone(), Some(paint_rect));
        // 执行一次与布局后绑定相同的依赖探测。
        label.probe_dependencies();
        // 将捕获依赖交给模拟实际节点租约集合。
        let leases = capture.finish();
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
        // 释放模拟实际节点拥有的全部绘制订阅。
        drop(leases);
        // 节点离开后 State 不得继续保留其绘制端点。
        assert_eq!(value.paint_site_count(), 0);
    }

    // 验证节点真实移除和窗口关闭都会释放 DynamicLabel 绘制订阅。
    #[test]
    fn tree_removal_and_shutdown_release_dynamic_label_paint_sites() {
        // 创建跨两棵临时窗口树存活的共享状态。
        let value = State::new(1_u32);

        // 克隆共享槽供第一棵树的动态文本闭包拥有。
        let removal_source = value.clone();
        // 创建模拟第一窗口的组件树。
        let mut removal_tree = WidgetTree::new();
        // 安装读取共享状态的动态标签并保存实际组件身份。
        let removal_id = removal_tree.set_root(Box::new(DynamicLabel::new(move || {
            // 每次探测或绘制都读取当前共享值。
            format!("remove: {}", removal_source.get())
        })));
        // 完成布局以触发框架级动态依赖探测。
        removal_tree.layout();
        // 第一棵树只能持有一个合并后的绘制端点。
        assert_eq!(value.paint_site_count(), 1);
        // 真实移除动态标签节点及其所有权资源。
        removal_tree.remove(removal_id);
        // 节点离开后共享 State 不得保留旧树端点。
        assert_eq!(value.paint_site_count(), 0);

        // 克隆共享槽供第二棵树的动态文本闭包拥有。
        let shutdown_source = value.clone();
        // 创建模拟第二窗口的组件树。
        let mut shutdown_tree = WidgetTree::new();
        // 安装读取同一共享槽的窗口根动态标签。
        shutdown_tree.set_root(Box::new(DynamicLabel::new(move || {
            // 每次探测或绘制都读取当前共享值。
            format!("shutdown: {}", shutdown_source.get())
        })));
        // 完成布局并建立由实际根节点持有的绘制租约。
        shutdown_tree.layout();
        // 第二棵树重新形成自己的唯一绘制端点。
        assert_eq!(value.paint_site_count(), 1);
        // 模拟所属窗口在保留树对象期间执行受控关闭。
        shutdown_tree.shutdown();
        // shutdown 必须主动清理仍在物理槽位中的节点租约。
        assert_eq!(value.paint_site_count(), 0);
    }
}
