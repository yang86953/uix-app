use super::{ControlledOpen, Modal};

use crate::platform::windowing::ControlSize;
use crate::ui::State;
use crate::ui::animation::AnimationConfig;
use std::rc::Rc;

/// Modal 内容回调上下文；关闭请求只作用于持有该上下文的 Modal。
pub struct ModalBuilder {
    pub(crate) modal: Modal,
    // 保存按声明顺序进入 Modal 内容区的全部 View。
    pub(crate) content: Vec<crate::ui::view::ViewNode>,
}

impl ModalBuilder {
    /// 设置受控打开状态（E-03）：Modal 可见性跟随 `State<bool>`；
    /// 初始为 `true` 时首帧直接呈现。
    pub fn open(mut self, state: &State<bool>) -> Self {
        self.modal.controlled = Some(ControlledOpen::new(state));
        if state.get() {
            self.modal.visible = true;
        }
        self
    }

    /// 注册可见性变化回调（E-03）：用户侧关闭（mask / close 按钮 / Escape）
    /// 与公开 `close()` 时触发；可与受控 State 双向回写。
    pub fn on_open_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(bool) + 'static,
    {
        if let Some(controlled) = &mut self.modal.controlled {
            controlled.on_change = Some(Rc::new(callback));
        }
        self
    }

    /// 设置 Modal 内容（E-03）：`FnOnce` 闭包构建内容 View。
    pub fn content<V>(mut self, content: impl FnOnce() -> V) -> Self
    where
        V: crate::ui::view::View,
    {
        // 单内容构建器替换现有内容集合，保持既有 API 的覆盖语义。
        self.content = vec![crate::ui::view::View::build(content())];
        // 返回更新后的构建器。
        self
    }

    // 设置已经物化的有序内容 View，供声明式生成器保留 If/For 展开结果。
    /// 替换已经物化、按声明顺序排列的全部内容节点。
    pub fn content_nodes(mut self, content: Vec<crate::ui::view::ViewNode>) -> Self {
        // 直接取得 View 集合所有权，避免复制运行节点或生命周期句柄。
        self.content = content;
        self
    }

    /// 设置对话框标题。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        // 标题配置完成后收紧为精确容量文本，避免每实例保留闲置 capacity。
        self.modal.title = title.into().into_boxed_str();
        self
    }

    /// 设置非负宽度；非有限数值会归零。
    pub fn width(mut self, width: f32) -> Self {
        self.modal.width = Modal::normalize_dimension(width);
        self
    }

    /// 设置非负高度；非有限数值会归零。
    pub fn height(mut self, height: f32) -> Self {
        self.modal.height = Modal::normalize_dimension(height);
        self
    }

    /// 同时设置非负宽高；非有限数值会归零。
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.modal.width = Modal::normalize_dimension(width);
        self.modal.height = Modal::normalize_dimension(height);
        self
    }

    /// 设置控件尺寸档位及其对应的预设宽高。
    pub fn modal_size(mut self, size: ControlSize) -> Self {
        self.modal = self.modal.modal_size(size);
        self
    }

    /// 设置是否显示并响应右上角关闭按钮。
    pub fn closable(mut self, closable: bool) -> Self {
        self.modal.closable = closable;
        self
    }

    /// 设置点击遮罩区域是否关闭对话框。
    pub fn mask_closable(mut self, mask_closable: bool) -> Self {
        self.modal.mask_closable = mask_closable;
        self
    }

    /// 设置默认底部确认和取消操作区是否可见。
    pub fn footer_visible(mut self, footer_visible: bool) -> Self {
        self.modal.footer_visible = footer_visible;
        self
    }

    // 注册确认操作的同步窄回调。
    /// 注册用户执行有效确认操作时同步调用的回调。
    pub fn on_ok<F>(mut self, callback: F) -> Self
    where
        // 回调由 Modal 实例持有并在有效确认输入中同步调用。
        F: Fn() + 'static,
    {
        // 使用共享所有权让声明式 reconcile 可替换回调而不借用调用方。
        self.modal.ok_callback = Some(Rc::new(callback));
        // 返回更新后的构建器。
        self
    }

    // 注册取消、遮罩、关闭槽与 Escape 共用的同步窄回调。
    /// 注册有效取消、遮罩、关闭按钮或 Escape 操作共用的同步回调。
    pub fn on_cancel<F>(mut self, callback: F) -> Self
    where
        // 回调由 Modal 实例持有并在有效取消输入中同步调用。
        F: Fn() + 'static,
    {
        // 使用共享所有权让声明式 reconcile 可替换回调而不借用调用方。
        self.modal.cancel_callback = Some(Rc::new(callback));
        // 返回更新后的构建器。
        self
    }

    /// 设置非浮层模式下是否在可用区域中居中对话框。
    pub fn centered(mut self, centered: bool) -> Self {
        self.modal.centered = centered;
        self
    }

    /// 设置是否把对话框登记为覆盖宿主表面的浮层。
    pub fn overlay(mut self, overlay: bool) -> Self {
        self.modal.overlay = overlay;
        self
    }

    /// 为受控 Modal 显式启用统一 backdrop blur 请求。
    pub fn backdrop_blur(mut self, blur: crate::ui::OverlayBackdropBlur) -> Self {
        // 复用 Modal 自身的便捷属性，不建立第二套效果状态。
        self.modal = self.modal.backdrop_blur(blur);
        // 返回更新后的声明式构建器。
        self
    }

    /// 设置退出动画完成后是否从组件树销毁内容子树。
    pub fn destroy_on_close(mut self, destroy_on_close: bool) -> Self {
        self.modal.destroy_on_close = destroy_on_close;
        self
    }

    /// 设置打开时播放的动画。
    pub fn enter_animation(mut self, animation: AnimationConfig) -> Self {
        self.modal = self.modal.enter_animation(animation);
        self
    }

    /// 设置关闭时播放的动画。
    pub fn leave_animation(mut self, animation: AnimationConfig) -> Self {
        self.modal = self.modal.leave_animation(animation);
        self
    }
}

impl crate::ui::view::View for ModalBuilder {
    fn build(self) -> crate::ui::view::ViewNode {
        // UIX 拥有视觉配置；Rust 内核继续独占状态、事件、布局与内容生命周期。
        self.modal.build_view_with_children(self.content)
    }
}

impl crate::ui::IntoWidgetNode for ModalBuilder {
    fn into_node(self) -> crate::ui::widget_runtime::widget::WidgetNode {
        crate::ui::adapter::ViewAdapter::expand(crate::ui::view::View::build(self))
    }
}

impl From<ModalBuilder> for crate::ui::view::ViewNode {
    fn from(builder: ModalBuilder) -> Self {
        crate::ui::view::View::build(builder)
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/feedback/modal/builder__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
