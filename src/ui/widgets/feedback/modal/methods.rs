use super::{Modal, ModalBuilder, ModalContext, ModalPointerTarget};

use std::cell::{Cell, RefCell};

use crate::core::{Point, Rect, Size};
use crate::draw::Color;
use crate::native::windowing::input::ControlSize;
use crate::ui::animation::{presets, AnimationConfig, TransitionPlayer};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::SnapshotFields;

use std::rc::Rc;

/// Modal 内容回调上下文；关闭请求只作用于持有该上下文的 Modal。

impl Modal {

    pub fn new(title: &str) -> Self {
        let size = crate::ui::component::config::use_config().size;
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
            controlled: None,
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

    /// 构建受控声明式 Modal（E-03）：配合 `.open(&State<bool>)` 由业务状态
    /// 驱动可见性，`.on_open_change` 在用户侧关闭时回调；`closable(false)`
    /// 禁用外部关闭但保留 Escape。
    pub fn builder() -> ModalBuilder {
        let mut modal = Self::new("").overlay(true);
        modal.footer_visible = false;
        let content = crate::ui::render_empty_for::<Modal>()
            .unwrap_or_else(|| crate::ui::view::View::build(crate::ui::widgets::label("")));
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
            let cancel = crate::ui::widgets::button("取消").on_click_fn(move || {
                if cancel_completed.replace(true) {
                    return;
                }
                if let Some(callback) = cancel_callback.borrow_mut().take() {
                    callback();
                }
                cancel_context.close();
            });
            let confirm = crate::ui::widgets::button("确定")
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
            crate::ui::widgets::column((
                crate::ui::widgets::label(content),
                crate::ui::widgets::row((cancel, confirm)).gap(8.0),
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

    pub(crate) fn shortcut(
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
                crate::ui::widgets::embed(crate::ui::widgets::Icon::new(icon_name).size(24.0))
                    .color(crate::ui::ColorValue::Palette(status_color))
                    .role(crate::ui::AccessibilityRole::Image)
                    .accessible_name(status_name);
            let message =
                crate::ui::widgets::row((status_icon, crate::ui::widgets::label(content)))
                    .align(crate::ui::layout::AlignItems::Center)
                    .gap(12.0);
            let confirm = crate::ui::widgets::button("确定")
                .primary()
                .on_click_fn(move || context.close());
            crate::ui::widgets::column((message, confirm)).gap(16.0)
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
        self.do_open();
        if let Some(controlled) = &self.controlled {
            controlled.state.set(true);
            controlled.notify(true);
        }
    }

    pub fn close(&mut self) {
        self.do_close();
        if let Some(controlled) = &self.controlled {
            controlled.state.set(false);
            controlled.notify(false);
        }
    }

    pub(crate) fn do_open(&mut self) {
        self.cancel_interaction();
        self.visible = true;
        self.closing = false;
        self.restart_enter_transition();
        self.layout_requested.set(true);
    }

    pub(crate) fn do_close(&mut self) {
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

    /// 受控跟随（E-03）：每帧把受控 State 同步到 visible；外部 `set(true)`
    /// 在下一帧打开，`set(false)` 走正常离场动画。
    pub(crate) fn controlled_sync(&mut self) {
        if let Some(controlled) = &self.controlled {
            let want_open = controlled.want_open();
            let is_open = self.visible && !self.closing;
            if want_open != is_open {
                if want_open {
                    self.do_open();
                } else {
                    self.do_close();
                }
            }
        }
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
        if next.controlled.is_some() {
            self.controlled = next.controlled;
        }
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
        if interaction_geometry_changed {
            self.cancel_interaction();
        }
    }

    pub(crate) fn dialog_rect_for_surface(&self, surface: Rect) -> Rect {
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

    pub(crate) fn trigger_rect_for_size(frame_w: f32, frame_h: f32) -> Rect {
        let frame_w = Self::normalize_dimension(frame_w);
        let frame_h = Self::normalize_dimension(frame_h);
        let width = frame_w.min(96.0);
        Rect::new((frame_w - width) * 0.5, 0.0, width, frame_h.min(32.0))
    }

    pub(crate) fn trigger_rect_local(&self) -> Rect {
        let trigger = self.last_trigger_rect.get();
        if trigger.w > 0.0 && trigger.h > 0.0 {
            trigger
        } else {
            Rect::new(0.0, 0.0, 96.0, 32.0)
        }
    }

    pub(crate) fn dialog_rect_local(&self) -> Rect {
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

    pub(crate) fn close_rect_local(&self) -> Rect {
        let dialog = self.dialog_rect_local();
        let width = dialog.w.min(48.0);
        Rect::new(
            dialog.x + dialog.w - width,
            dialog.y,
            width,
            dialog.h.min(56.0),
        )
    }

    pub(crate) fn pointer_target_at(&self, pos: Point) -> Option<ModalPointerTarget> {
        if self.closable && self.close_rect_local().contains(pos) {
            Some(ModalPointerTarget::Close)
        } else if self.mask_closable && !self.dialog_rect_local().contains(pos) {
            Some(ModalPointerTarget::Mask)
        } else {
            None
        }
    }

    pub(crate) fn cancel_interaction(&self) {
        self.close_hovered.set(false);
        self.pressed_target.set(None);
        self.activation_key.set(None);
    }

    pub(crate) fn body_rect(&self, dialog: Rect) -> Rect {
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

    pub(crate) fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            Self::normalize_dimension(frame.w),
            Self::normalize_dimension(frame.h),
        )
    }

    pub(crate) fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    pub(crate) fn inset_rect(frame: Rect, inset: f32) -> Rect {
        let inset_x = inset.min(frame.w * 0.5);
        let inset_y = inset.min(frame.h * 0.5);
        Rect::new(
            frame.x + inset_x,
            frame.y + inset_y,
            (frame.w - inset_x * 2.0).max(0.0),
            (frame.h - inset_y * 2.0).max(0.0),
        )
    }

    pub(crate) fn paint_elided_text(
        ctx: &mut PaintContext,
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

    pub(crate) fn elide_single_line(
        ctx: &mut PaintContext,
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

    pub(crate) fn text_width(ctx: &mut PaintContext, value: &str, font_size: f32) -> f32 {
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

    pub(crate) fn restart_enter_transition(&mut self) {
        let mut transition = TransitionPlayer::new(self.enter_animation);
        // 与 View 进出场一致：零时长动画在创建后立即完成，避免登记多余帧。
        if self.enter_animation.duration() <= 0.0 {
            transition.update(0.0);
        }
        self.transition = transition;
        self.transition_dirty = true;
    }

    pub(crate) fn transition_opacity(&self) -> f32 {
        self.transition.opacity_progress.clamp(0.0, 1.0)
    }

    pub(crate) fn apply_transition_to_dialog(&self, rect: Rect) -> Rect {
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

    pub(crate) fn intrinsic_size(&self) -> Size {
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

