//! Alert widget — 警示条，支持类型、图标、关闭。

use std::cell::Cell;
use std::rc::Rc;

use crate::core::{Constraints, Rect, Size};
use crate::draw::Radius;
use crate::platform::capabilities::StatusLevel;
use crate::ui::SnapshotFields;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetId};
use crate::widget;

mod presentation;
use presentation::*;

// 记录会覆盖 UIX 默认值的 Rust 调用方声明。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AlertAuthored(u8);

impl AlertAuthored {
    const SHOW_ICON: u8 = 1 << 0;
    const BANNER: u8 = 1 << 1;

    fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertTarget {
    Action,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertPress {
    Pointer(AlertTarget),
    Key(KeyCode, AlertTarget),
}

#[derive(Debug, Clone, Copy)]
struct AlertLayout {
    frame: Rect,
    accent: Rect,
    icon: Rect,
    message: Rect,
    description: Rect,
    action: Rect,
    close: Rect,
}

widget! {
    /// Alert — 带类型颜色的警示条。
    pub struct Alert {
        message: String,
        description: String,
        type_: StatusLevel,
        closable: bool,
        show_icon: bool,
        visible: bool,
        action_label: String,
        action_callback: Option<Rc<dyn Fn()>>,
        banner: bool,
        focused: bool,
        action_hovered: Cell<bool>,
        close_hovered: Cell<bool>,
        pressed: Cell<Option<AlertPress>>,
        last_size: Cell<Size>,
        layout_requested: Cell<bool>,
        pending_close: Cell<bool>,
        pending_action: Cell<bool>,
        #[snapshot(skip)]
        visual: &'static AlertVisual,
        #[snapshot(skip)]
        authored: AlertAuthored,
    }

    measure => (&self, constraints: Constraints) -> Size {
        if self.visible {
            constraints.clamp(self.intrinsic_size())
        } else {
            Size::zero()
        }
    }

    visible => (&self) -> bool { self.visible }

    tab_index => (&self) -> i32 {
        i32::from(self.visible && (self.closable || !self.action_label.is_empty()))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.visible || (!self.closable && self.action_label.is_empty()) {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(target) = self.target_at(*pos) else {
                    return EventResult::NotHandled;
                };
                self.action_hovered.set(target == AlertTarget::Action);
                self.close_hovered.set(target == AlertTarget::Close);
                self.pressed.set(Some(AlertPress::Pointer(target)));
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(AlertPress::Pointer(target)) = self.pressed.replace(None) else {
                    return EventResult::NotHandled;
                };
                let released = self.target_at(*pos);
                self.action_hovered
                    .set(released == Some(AlertTarget::Action));
                self.close_hovered
                    .set(released == Some(AlertTarget::Close));
                let released_inside = released == Some(target);
                if released_inside {
                    self.activate_target(target);
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let target = self.target_at(*pos);
                let action_hovered = target == Some(AlertTarget::Action);
                let close_hovered = target == Some(AlertTarget::Close);
                let action_changed = self.action_hovered.replace(action_hovered) != action_hovered;
                let close_changed = self.close_hovered.replace(close_hovered) != close_hovered;
                if action_changed || close_changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let had_pointer_press = matches!(self.pressed.get(), Some(AlertPress::Pointer(_)));
                if had_pointer_press {
                    self.pressed.set(None);
                }
                let changed = self.action_hovered.replace(false)
                    | self.close_hovered.replace(false)
                    | had_pointer_press;
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.action_hovered.set(false);
                self.close_hovered.set(false);
                self.pressed.set(None);
                EventResult::Handled
            }
            SystemEvent::KeyDown {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                let Some(target) = self.keyboard_target() else {
                    return EventResult::NotHandled;
                };
                if self.pressed.get().is_none() {
                    self.pressed.set(Some(AlertPress::Key(*key, target)));
                }
                EventResult::Handled
            }
            SystemEvent::KeyUp {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                let pressed = self.pressed.replace(None);
                match pressed {
                    Some(AlertPress::Key(pressed_key, target)) if pressed_key == *key => {
                        self.activate_target(target);
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            SystemEvent::KeyDown {
                key: KeyCode::Escape,
                ..
            } if self.closable => {
                self.dismiss_from_input();
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        if self.pending_action.replace(false) {
            Some(SemanticEvent::submit(id, self.action_label.clone()))
        } else {
            self.pending_close
                .replace(false)
                .then(|| SemanticEvent::change(id, "closed"))
        }
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let layout = self.layout(frame);
        self.last_size.set(Size::new(layout.frame.w, layout.frame.h));
        if layout.frame.w <= 0.0 || layout.frame.h <= 0.0 {
            return;
        }
        let resolved = self.visual.resolve(self.type_, ctx.tokens());
        let chrome = self.visual.chrome;
        let typography = self.visual.typography;
        let r = (!self.banner).then(|| Radius::uniform(resolved.container_radius));
        ctx.push_clip(layout.frame);
        ctx.fill_rect(layout.frame, resolved.background, r);
        if layout.accent.w > 0.0 && layout.accent.h > 0.0 {
            ctx.fill_rect(
                layout.accent,
                resolved.status,
                Some(Radius::uniform(chrome.accent_radius)),
            );
        }

        if self.show_icon && layout.icon.w > 0.0 && layout.icon.h > 0.0 {
            let icon_name = match self.type_ {
                StatusLevel::Success => typography.success_icon,
                StatusLevel::Info => typography.info_icon,
                StatusLevel::Warning => typography.warning_icon,
                StatusLevel::Error => typography.error_icon,
            };
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                icon_name,
                layout.icon,
                resolved.status,
                typography.status_icon,
            );
        }
        Self::paint_elided_text(
            ctx,
            &self.message,
            layout.message,
            resolved.text,
            typography.message,
        );
        if !self.description.is_empty() {
            Self::paint_elided_text(
                ctx,
                &self.description,
                layout.description,
                resolved.text_secondary,
                typography.description,
            );
        }
        let pressed = self.pressed.get();
        if !self.action_label.is_empty() {
            if pressed.is_some_and(|pressed| {
                matches!(
                    pressed,
                    AlertPress::Pointer(AlertTarget::Action)
                        | AlertPress::Key(_, AlertTarget::Action)
                )
            }) {
                ctx.fill_rect(
                    Self::inset_rect(layout.action, chrome.action_inset),
                    resolved.fill_secondary,
                    Some(Radius::uniform(resolved.interaction_radius)),
                );
            } else if self.action_hovered.get() {
                ctx.fill_rect(
                    Self::inset_rect(layout.action, chrome.action_inset),
                    resolved.fill_tertiary,
                    Some(Radius::uniform(resolved.interaction_radius)),
                );
            }
            Self::paint_elided_text(
                ctx,
                &self.action_label,
                layout.action,
                resolved.status,
                typography.action,
            );
        }
        if self.closable && layout.close.w > 0.0 && layout.close.h > 0.0 {
            let close_button = Self::inset_rect(layout.close, chrome.close_inset);
            if pressed.is_some_and(|pressed| {
                matches!(
                    pressed,
                    AlertPress::Pointer(AlertTarget::Close)
                        | AlertPress::Key(_, AlertTarget::Close)
                )
            }) {
                ctx.fill_rect(
                    close_button,
                    resolved.fill_secondary,
                    Some(Radius::uniform(resolved.interaction_radius)),
                );
            } else if self.close_hovered.get() {
                ctx.fill_rect(
                    close_button,
                    resolved.fill_tertiary,
                    Some(Radius::uniform(resolved.interaction_radius)),
                );
            }
            if self.focused && tree.keyboard_focus_visible() {
                ctx.stroke_rect(
                    close_button,
                    resolved.primary,
                    chrome.focus_stroke,
                    Some(Radius::uniform(resolved.interaction_radius)),
                );
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                typography.close_icon_name,
                close_button,
                resolved.text_secondary,
                typography.close_icon,
            );
        }
        ctx.pop_clip();
    }
}

// 把内容/交互状态与 UIX 静态视觉融合为单一 Alert 根节点。
fn build_alert_view(mut kernel: Alert, visual: &'static AlertVisual) -> ViewNode {
    if !kernel.authored.contains(AlertAuthored::SHOW_ICON) {
        kernel.show_icon = visual.defaults.show_icon;
    }
    if !kernel.authored.contains(AlertAuthored::BANNER) {
        kernel.banner = visual.defaults.banner;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 让声明式 View 构建统一进入同目录 UIX 根。
fn build_alert_uix_root(kernel: Alert) -> ViewNode {
    crate::uix!("src/ui/widgets/feedback/alert/alert.uix")
}

impl View for Alert {
    fn build(self) -> ViewNode {
        build_alert_uix_root(self)
    }
}

impl Default for Alert {
    fn default() -> Self {
        Self::new("")
    }
}

impl Alert {
    /// 创建默认显示图标、状态为信息且当前可见的警示条。
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            description: String::new(),
            type_: StatusLevel::Info,
            closable: false,
            show_icon: ALERT_VISUAL.defaults.show_icon,
            visible: true,
            action_label: String::new(),
            action_callback: None,
            banner: ALERT_VISUAL.defaults.banner,
            focused: false,
            action_hovered: Cell::new(false),
            close_hovered: Cell::new(false),
            pressed: Cell::new(None),
            last_size: Cell::new(Size::new(
                ALERT_VISUAL.defaults.width,
                ALERT_VISUAL.defaults.base_height,
            )),
            layout_requested: Cell::new(false),
            pending_close: Cell::new(false),
            pending_action: Cell::new(false),
            visual: ALERT_VISUAL_REF,
            authored: AlertAuthored::default(),
        }
    }

    /// 创建成功状态的警示条。
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message).type_(StatusLevel::Success)
    }

    /// 创建信息状态的警示条。
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message).type_(StatusLevel::Info)
    }

    /// 创建警告状态的警示条。
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message).type_(StatusLevel::Warning)
    }

    /// 创建错误状态的警示条。
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message).type_(StatusLevel::Error)
    }

    /// 设置操作按钮标签及其激活回调。
    pub fn action<F>(mut self, label: impl Into<String>, action: F) -> Self
    where
        F: Fn() + 'static,
    {
        self.action_label = label.into();
        self.action_callback = Some(Rc::new(action));
        self
    }

    /// 设置是否使用无圆角的横幅外观。
    pub fn banner(mut self, banner: bool) -> Self {
        self.banner = banner;
        self.authored.set(AlertAuthored::BANNER);
        self
    }
    /// 设置警示条的补充说明文本。
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }
    /// 设置警示条的状态级别。
    pub fn type_(mut self, t: StatusLevel) -> Self {
        self.type_ = t;
        self
    }
    /// 启用关闭控件。
    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }
    /// 设置是否显示状态图标。
    pub fn show_icon(mut self, show: bool) -> Self {
        self.show_icon = show;
        self.authored.set(AlertAuthored::SHOW_ICON);
        self
    }

    /// 返回警示条当前是否可见。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 显示警示条并请求重新布局。
    pub fn open(&mut self) {
        if !self.visible {
            self.visible = true;
            self.pending_close.set(false);
            self.pending_action.set(false);
            self.layout_requested.set(true);
        }
    }

    /// 隐藏警示条、清除交互状态并请求重新布局。
    pub fn close(&mut self) {
        if self.visible {
            self.visible = false;
            self.focused = false;
            self.action_hovered.set(false);
            self.close_hovered.set(false);
            self.pressed.set(None);
            self.layout_requested.set(true);
        }
    }

    fn dismiss_from_input(&mut self) {
        self.close();
        self.pending_close.set(true);
    }

    fn activate_target(&mut self, target: AlertTarget) {
        match target {
            AlertTarget::Action => {
                if let Some(callback) = self.action_callback.as_ref() {
                    callback();
                }
                self.pending_action.set(true);
            }
            AlertTarget::Close => self.dismiss_from_input(),
        }
    }

    fn keyboard_target(&self) -> Option<AlertTarget> {
        if self.closable {
            Some(AlertTarget::Close)
        } else if !self.action_label.is_empty() {
            Some(AlertTarget::Action)
        } else {
            None
        }
    }

    fn target_at(&self, point: crate::core::Point) -> Option<AlertTarget> {
        if self.closable && self.close_rect().contains(point) {
            Some(AlertTarget::Close)
        } else if !self.action_label.is_empty() && self.action_rect().contains(point) {
            Some(AlertTarget::Action)
        } else {
            None
        }
    }

    fn action_rect(&self) -> Rect {
        let size = self.last_size.get();
        self.layout(Rect::new(0.0, 0.0, size.w, size.h)).action
    }

    fn close_rect(&self) -> Rect {
        let size = self.last_size.get();
        self.layout(Rect::new(0.0, 0.0, size.w, size.h)).close
    }

    fn layout(&self, frame: Rect) -> AlertLayout {
        let frame = Self::normalize_frame(frame);
        let layout = self.visual.layout;
        let close_width = if self.closable {
            frame.w.min(layout.close_width)
        } else {
            0.0
        };
        let close_left = frame.x + frame.w - close_width;
        let action_right = (close_left
            - if self.closable {
                layout.close_action_gap
            } else {
                layout.content_right_gap
            })
        .max(frame.x);
        let action_width = if self.action_label.is_empty() {
            0.0
        } else {
            layout.action_width.min((action_right - frame.x).max(0.0))
        };
        let action_left = action_right - action_width;
        let content_right = if action_width > 0.0 {
            (action_left - layout.action_content_gap).max(frame.x)
        } else {
            (close_left - layout.content_right_gap).max(frame.x)
        };
        let icon_width = if self.show_icon {
            (content_right - frame.x).clamp(0.0, layout.icon_max_width)
        } else {
            0.0
        };
        let content_left = (frame.x
            + if self.show_icon {
                icon_width + layout.icon_gap
            } else {
                layout.no_icon_left
            })
        .min(content_right);
        let content = Rect::new(
            content_left,
            frame.y
                + layout
                    .content_vertical_inset
                    .min(frame.h * layout.content_vertical_ratio * 0.5),
            (content_right - content_left).max(0.0),
            (frame.h
                - (layout.content_vertical_inset * 2.0)
                    .min(frame.h * layout.content_vertical_ratio))
            .max(0.0),
        );
        let (message, description) = if self.description.is_empty() {
            (
                content,
                Rect::new(content.x, content.y + content.h, content.w, 0.0),
            )
        } else {
            let message_height = (content.h * layout.message_height_ratio).max(0.0);
            (
                Rect::new(content.x, content.y, content.w, message_height),
                Rect::new(
                    content.x,
                    content.y + message_height,
                    content.w,
                    (content.h - message_height).max(0.0),
                ),
            )
        };
        AlertLayout {
            frame,
            accent: Rect::new(
                frame.x + layout.accent_x.min(frame.w),
                frame.y + layout.accent_y.min(frame.h * 0.5),
                layout
                    .accent_width
                    .min((frame.w - layout.accent_x).max(0.0)),
                (frame.h - layout.accent_y * 2.0).max(0.0),
            ),
            icon: Rect::new(
                frame.x + layout.icon_x.min(frame.w),
                frame.y,
                icon_width,
                frame.h,
            ),
            message,
            description,
            action: Rect::new(
                action_left,
                frame.y,
                action_width,
                if action_width > 0.0 { frame.h } else { 0.0 },
            ),
            close: Rect::new(
                close_left,
                frame.y,
                close_width,
                if close_width > 0.0 { frame.h } else { 0.0 },
            ),
        }
    }

    fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            Self::normalize_dimension(frame.w),
            Self::normalize_dimension(frame.h),
        )
    }

    fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    fn inset_rect(frame: Rect, inset: f32) -> Rect {
        let inset_x = inset.min(frame.w * 0.5);
        let inset_y = inset.min(frame.h * 0.5);
        Rect::new(
            frame.x + inset_x,
            frame.y + inset_y,
            (frame.w - inset_x * 2.0).max(0.0),
            (frame.h - inset_y * 2.0).max(0.0),
        )
    }

    fn paint_elided_text(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: crate::draw::Color,
        font_size: f32,
    ) {
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(value) = ctx.elide_single_line_cow(value, font_size, frame.w) else {
            return;
        };
        if frame.h <= 0.0 {
            return;
        }
        ctx.push_clip(frame);
        let text_y = ctx.visual_center_y(frame, font_size);
        ctx.draw_text(
            value.as_ref(),
            crate::core::Point::new(frame.x, text_y),
            color,
            font_size,
        );
        ctx.pop_clip();
    }

    fn intrinsic_size(&self) -> Size {
        let h = self.visual.defaults.base_height
            + if self.description.is_empty() {
                0.0
            } else {
                self.visual.defaults.description_height
            };
        Size::new(self.visual.defaults.width, h)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Alert {
            message: self.message.clone(),
            description: self.description.clone(),
            type_: self.type_,
            closable: self.closable,
            show_icon: self.show_icon,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.message = next.message;
        self.description = next.description;
        self.type_ = next.type_;
        self.closable = next.closable;
        self.show_icon = next.show_icon;
        self.action_label = next.action_label;
        self.action_callback = next.action_callback;
        self.banner = next.banner;
        self.visual = next.visual;
        self.authored = next.authored;
        self.action_hovered.set(false);
        self.close_hovered.set(false);
        self.pressed.set(None);
        self.pending_close.set(false);
        self.pending_action.set(false);
        if (!self.closable && self.action_label.is_empty()) || !self.visible {
            self.focused = false;
        }
    }
}

// 集中验证 Alert 声明刷新与用户关闭状态的生命周期边界。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/feedback/alert__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
