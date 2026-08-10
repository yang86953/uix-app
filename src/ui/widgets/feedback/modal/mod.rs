use crate::ui::component::widget::WidgetCore;
use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::{Color, Radius};
use crate::native::windowing::input::ControlSize;
use crate::ui::animation::{AnimationConfig, TransitionPlayer};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::{EventResult, MouseButton, State, SystemEvent, WidgetTree};
use std::rc::Rc;

mod builder;
mod methods;

pub use builder::*;

/// Modal 内容回调上下文；关闭请求只作用于持有该上下文的 Modal。
#[derive(Clone)]
pub struct ModalContext {
    close_requested: Rc<Cell<bool>>,
}

impl ModalContext {
    pub(crate) fn new() -> Self {
        Self {
            close_requested: Rc::new(Cell::new(false)),
        }
    }

    /// 在 Modal 子树的事件回调中请求关闭当前 Modal。
    pub fn close(&self) {
        self.close_requested.set(true);
    }
}

/// 受控打开绑定（E-03）：visible 跟随 `State<bool>`，变化时通知回调。
///
/// 双向语义：外部 `State::set(true)` 在下一帧打开；用户侧关闭（mask /
/// close 按钮 / Escape / [`ModalContext::close`] / 公开 `close()`）写回
/// `false` 并触发 `on_open_change`。
pub(crate) struct ControlledOpen {
    state: State<bool>,
    on_change: Option<Rc<dyn Fn(bool)>>,
}

impl ControlledOpen {
    fn new(state: &State<bool>) -> Self {
        Self {
            state: state.clone(),
            on_change: None,
        }
    }

    fn want_open(&self) -> bool {
        self.state.get()
    }

    fn notify(&self, open: bool) {
        if let Some(callback) = &self.on_change {
            callback(open);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalPointerTarget {
    // 指向关闭状态下的内置打开入口。
    Trigger,
    // 指向标题栏关闭槽。
    Close,
    // 指向对话框外遮罩。
    Mask,
    // 指向底部取消操作。
    Cancel,
    // 指向底部确认操作。
    Ok,
}

component! {
    /// Modal dialog.
    pub struct Modal {
        pub(crate) title: String,
        pub(crate) visible: bool,
        pub(crate) width: f32,
        pub(crate) height: f32,
        pub(crate) modal_size: ControlSize,
        pub(crate) closable: bool,
        pub(crate) mask_closable: bool,
        pub(crate) footer_visible: bool,
        pub(crate) centered: bool,
        pub(crate) overlay: bool,
        pub(crate) destroy_on_close: bool,
        pub(crate) controlled: Option<ControlledOpen>,
        pub(crate) context_close_requested: Option<Rc<Cell<bool>>>,
        // 保存确认操作的同步窄回调。
        pub(crate) ok_callback: Option<Rc<dyn Fn()>>,
        // 保存取消、遮罩、关闭槽与 Escape 共用的同步窄回调。
        pub(crate) cancel_callback: Option<Rc<dyn Fn()>>,
        pub(crate) last_win_w: Cell<f32>,
        pub(crate) last_win_h: Cell<f32>,
        pub(crate) enter_animation: AnimationConfig,
        pub(crate) leave_animation: AnimationConfig,
        pub(crate) transition: TransitionPlayer,
        pub(crate) closing: bool,
        pub(crate) transition_dirty: bool,
        pub(crate) layout_requested: Cell<bool>,
        pub(crate) last_frame: Cell<Rect>,
        pub(crate) last_trigger_rect: Cell<Rect>,
        pub(crate) last_dialog_rect: Cell<Rect>,
        pub(crate) close_hovered: Cell<bool>,
        // 保存当前悬停的底部操作以驱动一致的交互绘制。
        pub(crate) footer_hovered: Cell<Option<ModalPointerTarget>>,
        pub(crate) pressed_target: Cell<Option<ModalPointerTarget>>,
        pub(crate) activation_key: Cell<Option<crate::ui::KeyCode>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    tab_index => (&self) -> i32 { i32::from(!self.is_present()) }

    hit_test_frame => (&self, actual_frame: Rect) -> Rect {
        self.last_frame.set(Self::normalize_frame(actual_frame));
        if self.overlay && self.is_present() {
            Rect::new(0.0, 0.0, self.last_win_w.get(), self.last_win_h.get())
        } else if !self.is_present() {
            let trigger = Self::trigger_rect_for_size(actual_frame.w, actual_frame.h);
            self.last_trigger_rect.set(trigger);
            Rect::new(
                actual_frame.x + trigger.x,
                actual_frame.y + trigger.y,
                trigger.w,
                trigger.h,
            )
        } else {
            actual_frame
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.is_present() {
            let trigger = self.trigger_rect_local();
            return match event {
                SystemEvent::PointerDown {
                    pos,
                    button: MouseButton::Left,
                    ..
                } if trigger.contains(*pos) => {
                    self.close_hovered.set(true);
                    self.pressed_target.set(Some(ModalPointerTarget::Trigger));
                    EventResult::Handled
                }
                SystemEvent::PointerUp {
                    pos,
                    button: MouseButton::Left,
                    ..
                } if self.pressed_target.replace(None) == Some(ModalPointerTarget::Trigger) => {
                    let released_inside = trigger.contains(*pos);
                    self.close_hovered.set(released_inside);
                    if released_inside {
                        self.open();
                    }
                    EventResult::Handled
                }
                SystemEvent::PointerMove { pos, .. } => {
                    let hovered = trigger.contains(*pos);
                    if self.close_hovered.replace(hovered) != hovered {
                        EventResult::Handled
                    } else {
                        EventResult::NotHandled
                    }
                }
                SystemEvent::PointerLeave | SystemEvent::FocusOut => {
                    self.cancel_interaction();
                    EventResult::Handled
                }
                SystemEvent::KeyDown {
                    key: key @ (crate::ui::KeyCode::Enter | crate::ui::KeyCode::Space),
                    ..
                } => {
                    self.activation_key.set(Some(*key));
                    EventResult::Handled
                }
                SystemEvent::KeyUp {
                    key: key @ (crate::ui::KeyCode::Enter | crate::ui::KeyCode::Space),
                    ..
                } if self.activation_key.replace(None) == Some(*key) => {
                    self.open();
                    EventResult::Handled
                }
                SystemEvent::FocusIn => EventResult::Handled,
                _ => EventResult::NotHandled,
            };
        }

        // 离场开始后停止接受新操作，避免重复回调或重新打开。
        if self.closing {
            // 清理上一输入序列的悬停和按压状态。
            self.cancel_interaction();
            // 遮罩仍吞掉输入，防止落到底层树。
            return EventResult::Handled;
        }

        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let target = self.pointer_target_at(*pos);
                // 让绘制与命中使用同一个目标解析结果。
                self.update_hover_target(target);
                self.pressed_target.set(target);
                if target.is_some() {
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let armed = self.pressed_target.replace(None);
                let target = self.pointer_target_at(*pos);
                // 更新释放位置对应的交互反馈。
                self.update_hover_target(target);
                if armed.is_some() && armed == target {
                    // 只有同一目标内的完整按下/释放序列才激活操作。
                    if let Some(target) = target {
                        // 委托 Modal 执行确认或取消的原子组件语义。
                        self.activate_target(target);
                    }
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                // 指针移动继续复用统一几何解析悬停目标。
                self.update_hover_target(self.pointer_target_at(*pos));
                EventResult::Handled
            }
            SystemEvent::PointerLeave | SystemEvent::FocusOut => {
                self.cancel_interaction();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                if *key == crate::ui::KeyCode::Escape {
                    // Escape 采用取消语义并写回受控 open 状态。
                    self.cancel();
                    return EventResult::Handled;
                }
                // 底部可见时 Enter/Space 预备默认确认操作。
                if self.footer_visible
                    && matches!(key, crate::ui::KeyCode::Enter | crate::ui::KeyCode::Space)
                {
                    // 保存配对释放所需的按键身份。
                    self.activation_key.set(Some(*key));
                    // Modal 已消费该确认按键。
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            // 配对的 Enter/Space 释放激活默认确认操作。
            SystemEvent::KeyUp { key, .. }
                if self.footer_visible && self.activation_key.replace(None) == Some(*key) =>
            {
                // 执行与确认按钮相同的窄回调和关闭语义。
                self.confirm_action();
                // Modal 已消费该确认按键。
                EventResult::Handled
            }
            _ => EventResult::Handled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Self::normalize_frame(frame);
        self.last_frame.set(frame);
        if !self.is_present() {
            let local_trigger = Self::trigger_rect_for_size(frame.w, frame.h);
            self.last_trigger_rect.set(local_trigger);
            let primary = if self.pressed_target.get() == Some(ModalPointerTarget::Trigger)
                || self.activation_key.get().is_some()
            {
                ctx.tokens().color_primary_active()
            } else if self.close_hovered.get() {
                ctx.tokens().color_primary_hover()
            } else {
                ctx.tokens().color_primary()
            };
            let trigger = Rect::new(
                frame.x + local_trigger.x,
                frame.y + local_trigger.y,
                local_trigger.w,
                local_trigger.h,
            );
            if trigger.w <= 0.0 || trigger.h <= 0.0 {
                return;
            }
            ctx.push_clip(trigger);
            ctx.fill_rect(trigger, primary, Some(Radius::uniform(ctx.tokens().border_radius())));
            ctx.text_center("打开 Modal", trigger, Color::white(), 13.0);
            ctx.pop_clip();
            return;
        }

        let border_radius_lg = ctx.tokens().border_radius_lg();
        let bg_container = ctx.tokens().color_bg_container();
        let border_secondary = ctx.tokens().color_border_secondary();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let surface_size = ctx.surface_size();
        let surface = Rect::new(0.0, 0.0, surface_size.w, surface_size.h);
        let dialog = if self.overlay {
            self.last_win_w.set(surface.w);
            self.last_win_h.set(surface.h);
            self.dialog_rect_for_surface(surface)
        } else {
            self.dialog_rect_for_surface(frame)
        };
        let dialog = self.apply_transition_to_dialog(dialog);
        self.last_dialog_rect.set(dialog);
        let overlay_alpha = (128.0 * self.transition_opacity()).round().clamp(0.0, 128.0) as u8;

        ctx.fill_rect(
            surface,
            Color::from_rgba(0, 0, 0, overlay_alpha),
            None,
        );

        if dialog.w <= 0.0 || dialog.h <= 0.0 {
            return;
        }
        ctx.push_clip(dialog);
        let radius = Some(Radius::uniform(border_radius_lg));
        ctx.fill_rect(dialog, bg_container, radius);
        ctx.stroke_rect(dialog, border_secondary, 1.0, radius);

        let title_h = dialog.h.min(56.0);
        let footer_h = if self.footer_visible {
            (dialog.h - title_h).clamp(0.0, 56.0)
        } else {
            0.0
        };
        let close_w = if self.closable { dialog.w.min(48.0) } else { 0.0 };
        let title_x = dialog.x + 24.0_f32.min(dialog.w);
        let title_content = Rect::new(
            title_x,
            dialog.y,
            (dialog.x + dialog.w - close_w - title_x).max(0.0),
            title_h,
        );
        Self::paint_elided_text(ctx, &self.title, title_content, text_color, 16.0);
        if title_h < dialog.h {
            ctx.fill_rect(
                Rect::new(dialog.x, dialog.y + title_h, dialog.w, 1.0),
                border_secondary,
                None,
            );
        }

        if self.closable {
            let close_rect = Rect::new(
                dialog.x + dialog.w - close_w,
                dialog.y,
                close_w,
                title_h,
            );
            let close_button = Self::inset_rect(close_rect, 8.0);
            if self.pressed_target.get() == Some(ModalPointerTarget::Close) {
                ctx.fill_rect(
                    close_button,
                    ctx.tokens().color_fill_secondary(),
                    Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                );
            } else if self.close_hovered.get() {
                ctx.fill_rect(
                    close_button,
                    ctx.tokens().color_fill_tertiary(),
                    Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                );
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                "x",
                close_button,
                text_secondary,
                16.0,
            );
        }
        if footer_h > 0.0 {
            ctx.fill_rect(
                Rect::new(dialog.x, dialog.y + dialog.h - footer_h, dialog.w, 1.0),
                border_secondary,
                None,
            );
            // 从与命中共用的几何函数取得取消和确认按钮区域。
            let (cancel_rect, ok_rect) = Self::footer_action_rects(dialog);
            // 使用统一的小圆角绘制底部操作。
            let action_radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
            // 按压与悬停状态只影响取消按钮的背景反馈。
            let cancel_fill = if self.pressed_target.get() == Some(ModalPointerTarget::Cancel) {
                // 按压取消使用更强的填充状态。
                ctx.tokens().color_fill_secondary()
            } else if self.footer_hovered.get() == Some(ModalPointerTarget::Cancel) {
                // 悬停取消使用较轻的填充状态。
                ctx.tokens().color_fill_tertiary()
            } else {
                // 默认取消按钮保持容器背景。
                bg_container
            };
            // 绘制取消按钮背景。
            ctx.fill_rect(cancel_rect, cancel_fill, action_radius);
            // 绘制取消按钮边框。
            ctx.stroke_rect(cancel_rect, border_secondary, 1.0, action_radius);
            // 使用当前语言环境绘制取消文案。
            ctx.text_center(
                crate::ui::component::locale::use_locale().cancel_text,
                cancel_rect,
                text_color,
                13.0,
            );
            // 确认按钮按交互状态选择主色。
            let ok_fill = if self.pressed_target.get() == Some(ModalPointerTarget::Ok) {
                // 按压确认使用主色激活态。
                ctx.tokens().color_primary_active()
            } else if self.footer_hovered.get() == Some(ModalPointerTarget::Ok) {
                // 悬停确认使用主色悬停态。
                ctx.tokens().color_primary_hover()
            } else {
                // 默认确认使用主色。
                ctx.tokens().color_primary()
            };
            // 绘制确认按钮背景。
            ctx.fill_rect(ok_rect, ok_fill, action_radius);
            // 使用当前语言环境绘制确认文案。
            ctx.text_center(
                crate::ui::component::locale::use_locale().ok_text,
                ok_rect,
                Color::white(),
                13.0,
            );
        }
        ctx.pop_clip();
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, _frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if self.is_present() && self.overlay {
            Some(
                crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Modal)
                    .bounds(Rect::new(-2000.0, -2000.0, 4000.0, 4000.0))
                    .z_index(1000),
            )
        } else {
            None
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        if !self.is_present() || children.is_empty() {
            return Vec::new();
        }
        let frame = Self::normalize_frame(frame);
        self.last_frame.set(frame);

        let dialog = if self.overlay {
            let (win_w, win_h) = tree
                .root_id()
                .and_then(|rid| tree.get(rid))
                .map(|root| (root.frame().w, root.frame().h))
                .map(|(w, h)| (Self::normalize_dimension(w), Self::normalize_dimension(h)))
                .filter(|(w, h)| *w > 0.0 && *h > 0.0)
                .unwrap_or((1200.0, 760.0));
            self.last_win_w.set(win_w);
            self.last_win_h.set(win_h);
            self.dialog_rect_for_surface(Rect::new(0.0, 0.0, win_w, win_h))
        } else {
            self.dialog_rect_for_surface(frame)
        };
        // Keep child layout on the final dialog geometry. The zoom transition
        // advances through paint-only invalidation, so baking its transient
        // scale into child frames would leave the subtree pinned to the first
        // animation sample without a matching layout pass.
        self.last_dialog_rect.set(dialog);

        let title_h = dialog.h.min(56.0);
        let footer_h = if self.footer_visible {
            (dialog.h - title_h).clamp(0.0, 56.0)
        } else {
            0.0
        };
        let body_y = dialog.y + title_h;
        let body_h = (dialog.h - title_h - footer_h).max(0.0);
        let padding = 24.0;
        children
            .iter()
            .map(|child| {
                (
                    child.id,
                    Rect::new(
                        dialog.x + padding,
                        body_y + padding,
                        (dialog.w - padding * 2.0).max(0.0),
                        (body_h - padding * 2.0).max(0.0),
                    ),
                )
            })
            .collect()
    }

    children_clip => (&self, _frame: Rect) -> Option<Rect> {
        if self.is_present() {
            Some(self.body_rect(self.last_dialog_rect.get()))
        } else {
            Some(Rect::zero())
        }
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        self.controlled_sync();
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.visible = false;
            self.closing = false;
            self.layout_requested.set(true);
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            let surface_w = self.last_win_w.get();
            let surface_h = self.last_win_h.get();
            if self.overlay && surface_w > 0.0 && surface_h > 0.0 {
                Rect::new(0.0, 0.0, surface_w, surface_h)
            } else {
                frame
            }
        } else {
            Rect::zero()
        }
    }
}
