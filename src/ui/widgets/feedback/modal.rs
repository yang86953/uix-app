use crate::ui::core::widget::WidgetCore;
use std::cell::{Cell, RefCell};

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::{Color, Radius};
use crate::native::traits::input::ControlSize;
use crate::ui::animation::{presets, AnimationConfig, TransitionPlayer};
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, MouseButton, SystemEvent, WidgetTree};
use std::rc::Rc;

/// Modal 内容回调上下文；关闭请求只作用于持有该上下文的 Modal。
#[derive(Clone)]
pub struct ModalContext {
    close_requested: Rc<Cell<bool>>,
}

impl ModalContext {
    fn new() -> Self {
        Self {
            close_requested: Rc::new(Cell::new(false)),
        }
    }

    /// 在 Modal 子树的事件回调中请求关闭当前 Modal。
    pub fn close(&self) {
        self.close_requested.set(true);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModalPointerTarget {
    Trigger,
    Close,
    Mask,
}

component! {
    /// Modal dialog.
    pub struct Modal {
        title: String,
        visible: bool,
        width: f32,
        height: f32,
        modal_size: ControlSize,
        closable: bool,
        mask_closable: bool,
        footer_visible: bool,
        centered: bool,
        overlay: bool,
        destroy_on_close: bool,
        context_close_requested: Option<Rc<Cell<bool>>>,
        last_win_w: Cell<f32>,
        last_win_h: Cell<f32>,
        enter_animation: AnimationConfig,
        leave_animation: AnimationConfig,
        pub(crate) transition: TransitionPlayer,
        closing: bool,
        pub(crate) transition_dirty: bool,
        layout_requested: Cell<bool>,
        last_frame: Cell<Rect>,
        last_trigger_rect: Cell<Rect>,
        last_dialog_rect: Cell<Rect>,
        close_hovered: Cell<bool>,
        pressed_target: Cell<Option<ModalPointerTarget>>,
        activation_key: Cell<Option<crate::ui::KeyCode>>,
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

        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let target = self.pointer_target_at(*pos);
                self.close_hovered
                    .set(target == Some(ModalPointerTarget::Close));
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
                self.close_hovered
                    .set(target == Some(ModalPointerTarget::Close));
                if armed.is_some() && armed == target {
                    self.close();
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let hovered = self.pointer_target_at(*pos) == Some(ModalPointerTarget::Close);
                self.close_hovered.set(hovered);
                EventResult::Handled
            }
            SystemEvent::PointerLeave | SystemEvent::FocusOut => {
                self.cancel_interaction();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                if *key == crate::ui::KeyCode::Escape {
                    self.close();
                    return EventResult::Handled;
                }
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

impl Modal {
    pub fn new(title: &str) -> Self {
        let size = crate::ui::config::use_config().size;
        Self {
            title: title.to_string(),
            visible: false,
            width: 520.0,
            height: 300.0,
            modal_size: size,
            closable: true,
            mask_closable: true,
            footer_visible: true,
            centered: true,
            overlay: false,
            destroy_on_close: false,
            context_close_requested: None,
            last_win_w: Cell::new(0.0),
            last_win_h: Cell::new(0.0),
            enter_animation: presets::modal_enter(),
            leave_animation: presets::modal_exit(),
            transition: TransitionPlayer::new(presets::modal_enter()),
            closing: false,
            transition_dirty: false,
            layout_requested: Cell::new(false),
            last_frame: Cell::new(Rect::zero()),
            last_trigger_rect: Cell::new(Rect::new(0.0, 0.0, 96.0, 32.0)),
            last_dialog_rect: Cell::new(Rect::zero()),
            close_hovered: Cell::new(false),
            pressed_target: Cell::new(None),
            activation_key: Cell::new(None),
        }
        .modal_size(size)
    }

    pub fn visible(mut self, v: bool) -> Self {
        self.set_visible(v);
        self
    }

    /// 构建默认打开的声明式 Modal；内容可通过 [`ModalContext::close`] 关闭它。
    pub fn show<V>(content: impl FnOnce(ModalContext) -> V) -> ModalBuilder
    where
        V: crate::ui::view::View,
    {
        let context = ModalContext::new();
        let content = crate::ui::view::View::build(content(context.clone()));
        let mut modal = Self::new("").visible(true).overlay(true);
        modal.footer_visible = false;
        modal.context_close_requested = Some(context.close_requested);
        ModalBuilder { modal, content }
    }

    /// 快捷确认对话框；回调由应用在接入业务动作时持有。
    pub fn confirm<Ok, Cancel>(
        title: impl Into<String>,
        content: impl Into<String>,
        ok_callback: Ok,
        cancel_callback: Cancel,
    ) -> ModalBuilder
    where
        Ok: FnOnce() + 'static,
        Cancel: FnOnce() + 'static,
    {
        let ok_callback = Rc::new(RefCell::new(Some(ok_callback)));
        let cancel_callback = Rc::new(RefCell::new(Some(cancel_callback)));
        let completed = Rc::new(Cell::new(false));
        let content = content.into();
        Self::show(move |context| {
            let cancel_context = context.clone();
            let ok_context = context;
            let cancel_callback = cancel_callback.clone();
            let ok_callback = ok_callback.clone();
            let cancel_completed = completed.clone();
            let ok_completed = completed;
            let cancel = crate::ui::view::button("取消").on_click_fn(move || {
                if cancel_completed.replace(true) {
                    return;
                }
                if let Some(callback) = cancel_callback.borrow_mut().take() {
                    callback();
                }
                cancel_context.close();
            });
            let confirm = crate::ui::view::button("确定")
                .primary()
                .on_click_fn(move || {
                    if ok_completed.replace(true) {
                        return;
                    }
                    if let Some(callback) = ok_callback.borrow_mut().take() {
                        callback();
                    }
                    ok_context.close();
                });
            crate::ui::view::column((
                crate::ui::view::label(content),
                crate::ui::view::row((cancel, confirm)).gap(8.0),
            ))
            .gap(16.0)
        })
        .title(title)
    }

    pub fn info(title: impl Into<String>, content: impl Into<String>) -> ModalBuilder {
        Self::shortcut(
            title,
            content,
            "info",
            "信息",
            crate::ui::PaletteColor::Info,
        )
    }

    pub fn warning(title: impl Into<String>, content: impl Into<String>) -> ModalBuilder {
        Self::shortcut(
            title,
            content,
            "alert-triangle",
            "警告",
            crate::ui::PaletteColor::Warning,
        )
    }

    pub fn error(title: impl Into<String>, content: impl Into<String>) -> ModalBuilder {
        Self::shortcut(
            title,
            content,
            "x-circle",
            "错误",
            crate::ui::PaletteColor::Error,
        )
    }

    fn shortcut(
        title: impl Into<String>,
        content: impl Into<String>,
        icon_name: &'static str,
        status_name: &'static str,
        status_color: crate::ui::PaletteColor,
    ) -> ModalBuilder {
        let title = title.into();
        let content = content.into();
        Self::show(move |context| {
            let status_icon =
                crate::ui::view::embed(crate::ui::widgets::Icon::new(icon_name).size(24.0))
                    .color(crate::ui::ColorValue::Palette(status_color))
                    .role(crate::ui::AccessibilityRole::Image)
                    .accessible_name(status_name);
            let message = crate::ui::view::row((status_icon, crate::ui::view::label(content)))
                .align(crate::ui::layout::AlignItems::Center)
                .gap(12.0);
            let confirm = crate::ui::view::button("确定")
                .primary()
                .on_click_fn(move || context.close());
            crate::ui::view::column((message, confirm)).gap(16.0)
        })
        .title(title)
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = Self::normalize_dimension(w);
        self.height = Self::normalize_dimension(h);
        self
    }

    pub fn modal_size(mut self, s: ControlSize) -> Self {
        self.modal_size = s;
        match s {
            ControlSize::Small => {
                self.width = 400.0;
                self.height = 200.0;
            }
            ControlSize::Medium => {
                self.width = 520.0;
                self.height = 300.0;
            }
            ControlSize::Large => {
                self.width = 720.0;
                self.height = 400.0;
            }
        }
        self
    }

    pub fn closable(mut self, v: bool) -> Self {
        self.closable = v;
        self
    }

    pub fn mask_closable(mut self, v: bool) -> Self {
        self.mask_closable = v;
        self
    }

    pub fn footer_visible(mut self, v: bool) -> Self {
        self.footer_visible = v;
        self
    }

    pub fn centered(mut self, v: bool) -> Self {
        self.centered = v;
        self
    }

    pub fn overlay(mut self, v: bool) -> Self {
        self.overlay = v;
        self
    }

    pub fn destroy_on_close(mut self, v: bool) -> Self {
        self.destroy_on_close = v;
        self
    }

    /// 设置打开时播放的动画；已打开时从当前声明重新开始进场。
    pub fn enter_animation(mut self, animation: AnimationConfig) -> Self {
        self.enter_animation = animation;
        if self.visible && !self.closing {
            self.restart_enter_transition();
        }
        self
    }

    /// 设置关闭时播放的动画。
    pub fn leave_animation(mut self, animation: AnimationConfig) -> Self {
        self.leave_animation = animation;
        if self.closing {
            self.transition = TransitionPlayer::new(animation);
            self.transition_dirty = true;
        }
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, v: bool) {
        if v {
            self.open();
        } else {
            self.close();
        }
    }

    pub fn open(&mut self) {
        self.cancel_interaction();
        self.visible = true;
        self.closing = false;
        self.restart_enter_transition();
        self.layout_requested.set(true);
    }

    pub fn close(&mut self) {
        self.cancel_interaction();
        if !self.is_present() {
            self.visible = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }
        self.visible = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(self.leave_animation);
        self.transition_dirty = true;
        self.layout_requested.set(true);
    }

    pub fn confirm_close(&mut self) {
        self.close();
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let interaction_geometry_changed = self.width != next.width
            || self.height != next.height
            || self.closable != next.closable
            || self.mask_closable != next.mask_closable
            || self.centered != next.centered
            || self.overlay != next.overlay;
        self.title = next.title;
        self.width = next.width;
        self.height = next.height;
        self.modal_size = next.modal_size;
        self.closable = next.closable;
        self.mask_closable = next.mask_closable;
        self.footer_visible = next.footer_visible;
        self.centered = next.centered;
        self.overlay = next.overlay;
        self.destroy_on_close = next.destroy_on_close;
        self.context_close_requested = next.context_close_requested;
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
        if interaction_geometry_changed {
            self.cancel_interaction();
        }
    }

    fn dialog_rect_for_surface(&self, surface: Rect) -> Rect {
        let surface = Self::normalize_frame(surface);
        let width = Self::normalize_dimension(self.width).min(surface.w);
        let height = Self::normalize_dimension(self.height).min(surface.h);
        let (x, y) = if self.centered || self.overlay {
            (
                surface.x + (surface.w - width) * 0.5,
                surface.y + (surface.h - height) * 0.5,
            )
        } else {
            (surface.x, surface.y)
        };
        Rect::new(x, y, width, height)
    }

    fn trigger_rect_for_size(frame_w: f32, frame_h: f32) -> Rect {
        let frame_w = Self::normalize_dimension(frame_w);
        let frame_h = Self::normalize_dimension(frame_h);
        let width = frame_w.min(96.0);
        Rect::new((frame_w - width) * 0.5, 0.0, width, frame_h.min(32.0))
    }

    fn trigger_rect_local(&self) -> Rect {
        let trigger = self.last_trigger_rect.get();
        if trigger.w > 0.0 && trigger.h > 0.0 {
            trigger
        } else {
            Rect::new(0.0, 0.0, 96.0, 32.0)
        }
    }

    fn dialog_rect_local(&self) -> Rect {
        let frame = self.last_frame.get();
        let dialog = self.last_dialog_rect.get();
        let dialog = if dialog.w > 0.0 && dialog.h > 0.0 {
            dialog
        } else if self.overlay {
            self.dialog_rect_for_surface(Rect::new(
                0.0,
                0.0,
                self.last_win_w.get(),
                self.last_win_h.get(),
            ))
        } else {
            self.dialog_rect_for_surface(frame)
        };
        Rect::new(dialog.x - frame.x, dialog.y - frame.y, dialog.w, dialog.h)
    }

    fn close_rect_local(&self) -> Rect {
        let dialog = self.dialog_rect_local();
        let width = dialog.w.min(48.0);
        Rect::new(
            dialog.x + dialog.w - width,
            dialog.y,
            width,
            dialog.h.min(56.0),
        )
    }

    fn pointer_target_at(&self, pos: Point) -> Option<ModalPointerTarget> {
        if self.closable && self.close_rect_local().contains(pos) {
            Some(ModalPointerTarget::Close)
        } else if self.mask_closable && !self.dialog_rect_local().contains(pos) {
            Some(ModalPointerTarget::Mask)
        } else {
            None
        }
    }

    fn cancel_interaction(&self) {
        self.close_hovered.set(false);
        self.pressed_target.set(None);
        self.activation_key.set(None);
    }

    fn body_rect(&self, dialog: Rect) -> Rect {
        let header_h = dialog.h.min(56.0);
        let footer_h = if self.footer_visible {
            (dialog.h - header_h).clamp(0.0, 56.0)
        } else {
            0.0
        };
        Rect::new(
            dialog.x,
            dialog.y + header_h,
            dialog.w.max(0.0),
            (dialog.h - header_h - footer_h).max(0.0),
        )
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
        ctx: &mut PaintContext<'_>,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        let Some(value) = Self::elide_single_line(ctx, value, font_size, frame.w) else {
            return;
        };
        if frame.h <= 0.0 {
            return;
        }
        ctx.push_clip(frame);
        let y = ctx.visual_center_y(frame, font_size);
        ctx.draw_text(&value, Point::new(frame.x, y), color, font_size);
        ctx.pop_clip();
    }

    fn elide_single_line(
        ctx: &mut PaintContext<'_>,
        value: &str,
        font_size: f32,
        max_width: f32,
    ) -> Option<String> {
        if !max_width.is_finite() || max_width <= 0.0 {
            return None;
        }
        let value = value.replace(['\r', '\n'], " ");
        if Self::text_width(ctx, &value, font_size) <= max_width {
            return Some(value);
        }
        const ELLIPSIS: &str = "…";
        if Self::text_width(ctx, ELLIPSIS, font_size) > max_width {
            return None;
        }
        let mut visible = String::new();
        for ch in value.chars() {
            visible.push(ch);
            visible.push_str(ELLIPSIS);
            let fits = Self::text_width(ctx, &visible, font_size) <= max_width;
            visible.pop();
            if !fits {
                visible.pop();
                break;
            }
        }
        visible.push_str(ELLIPSIS);
        Some(visible)
    }

    fn text_width(ctx: &mut PaintContext<'_>, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::resources::font::text_backend::estimate_text_metrics(
                value,
                f32::INFINITY,
                font_size,
            )
            .max_line_width,
        )
    }

    pub(crate) fn is_present(&self) -> bool {
        self.visible || self.closing
    }

    pub(crate) fn should_destroy_on_close(&self) -> bool {
        self.destroy_on_close
    }

    pub(crate) fn take_context_close_request(&self) -> bool {
        self.context_close_requested
            .as_ref()
            .is_some_and(|requested| requested.replace(false))
    }

    fn restart_enter_transition(&mut self) {
        let mut transition = TransitionPlayer::new(self.enter_animation);
        // 与 View 进出场一致：零时长动画在创建后立即完成，避免登记多余帧。
        if self.enter_animation.duration() <= 0.0 {
            transition.update(0.0);
        }
        self.transition = transition;
        self.transition_dirty = true;
    }

    fn transition_opacity(&self) -> f32 {
        self.transition.opacity_progress.clamp(0.0, 1.0)
    }

    fn apply_transition_to_dialog(&self, rect: Rect) -> Rect {
        let scale = self.transition.scale.clamp(0.0, 1.0);
        let w = rect.w * scale;
        let h = rect.h * scale;
        Rect::new(
            rect.x + (rect.w - w) * 0.5 + self.transition.offset.x,
            rect.y + (rect.h - h) * 0.5 + self.transition.offset.y,
            w,
            h,
        )
    }

    fn intrinsic_size(&self) -> Size {
        if self.is_present() {
            if self.overlay {
                Size::zero()
            } else {
                Size::new(
                    Self::normalize_dimension(self.width),
                    Self::normalize_dimension(self.height),
                )
            }
        } else {
            // Closed: reserve a trigger slot for gallery / live demos.
            Size::new(96.0, 32.0)
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Modal {
            title: self.title.clone(),
            open: self.is_present(),
            width: self.width,
            height: self.height,
            modal_size: self.modal_size,
            closable: self.closable,
            mask_closable: self.mask_closable,
            footer_visible: self.footer_visible,
            centered: self.centered,
            overlay: self.overlay,
        }
    }
}

/// `Modal::show` 返回的声明式构建器。
pub struct ModalBuilder {
    modal: Modal,
    content: crate::ui::view::ViewNode,
}

impl ModalBuilder {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.modal.title = title.into();
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.modal.width = Modal::normalize_dimension(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.modal.height = Modal::normalize_dimension(height);
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.modal.width = Modal::normalize_dimension(width);
        self.modal.height = Modal::normalize_dimension(height);
        self
    }

    pub fn modal_size(mut self, size: ControlSize) -> Self {
        self.modal = self.modal.modal_size(size);
        self
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.modal.closable = closable;
        self
    }

    pub fn mask_closable(mut self, mask_closable: bool) -> Self {
        self.modal.mask_closable = mask_closable;
        self
    }

    pub fn footer_visible(mut self, footer_visible: bool) -> Self {
        self.modal.footer_visible = footer_visible;
        self
    }

    pub fn centered(mut self, centered: bool) -> Self {
        self.modal.centered = centered;
        self
    }

    pub fn overlay(mut self, overlay: bool) -> Self {
        self.modal.overlay = overlay;
        self
    }

    pub fn destroy_on_close(mut self, destroy_on_close: bool) -> Self {
        self.modal.destroy_on_close = destroy_on_close;
        self
    }

    pub fn enter_animation(mut self, animation: AnimationConfig) -> Self {
        self.modal = self.modal.enter_animation(animation);
        self
    }

    pub fn leave_animation(mut self, animation: AnimationConfig) -> Self {
        self.modal = self.modal.leave_animation(animation);
        self
    }
}

impl crate::ui::view::View for ModalBuilder {
    fn build(self) -> crate::ui::view::ViewNode {
        crate::ui::view::ViewNode::new(self.modal, vec![self.content])
    }
}

impl crate::ui::IntoWidgetNode for ModalBuilder {
    fn into_node(self) -> crate::ui::core::widget::WidgetNode {
        crate::ui::view::ViewAdapter::expand(crate::ui::view::View::build(self))
    }
}

impl From<ModalBuilder> for crate::ui::view::ViewNode {
    fn from(builder: ModalBuilder) -> Self {
        crate::ui::view::View::build(builder)
    }
}
