//! 抽屉行为实现。

use super::*;

impl Drawer {
    pub fn new(title: &str) -> Self {
        let size = crate::ui::component::config::use_config().size;
        Self {
            title: title.to_string(),
            visible: false,
            width: 378.0,
            height: 300.0,
            drawer_size: size,
            placement: DrawerPlacement::Right,
            closable: true,
            mask_closable: true,
            mask: true,
            footer_visible: false,
            extra: String::new(),
            enter_animation: None,
            leave_animation: None,
            transition: TransitionPlayer::new(presets::drawer_enter(
                Self::animation_placement_for(DrawerPlacement::Right),
            )),
            closing: false,
            transition_dirty: false,
            layout_requested: Cell::new(false),
            last_frame: Cell::new(Rect::zero()),
            last_surface_w: Cell::new(0.0),
            last_surface_h: Cell::new(0.0),
            last_trigger_rect: Cell::new(Rect::new(0.0, 0.0, 96.0, 32.0)),
            last_panel_rect: Cell::new(Rect::zero()),
            close_hovered: Cell::new(false),
            pressed_target: Cell::new(None),
            activation_key: Cell::new(None),
        }
        .drawer_size(size)
    }

    pub fn visible(mut self, v: bool) -> Self {
        self.set_visible(v);
        self
    }

    pub fn show(mut self) -> Self {
        self.open();
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = Self::normalize_dimension(w);
        self.height = Self::normalize_dimension(h);
        self
    }

    pub fn drawer_size(mut self, s: ControlSize) -> Self {
        self.drawer_size = s;
        match s {
            ControlSize::Small => {
                self.width = 300.0;
                self.height = 200.0;
            }
            ControlSize::Medium => {
                self.width = 378.0;
                self.height = 300.0;
            }
            ControlSize::Large => {
                self.width = 600.0;
                self.height = 450.0;
            }
        }
        self
    }

    pub fn placement(mut self, p: DrawerPlacement) -> Self {
        self.placement = p;
        if self.visible && !self.closing {
            self.restart_enter_transition();
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

    pub fn mask(mut self, v: bool) -> Self {
        self.mask = v;
        self
    }

    pub fn footer_visible(mut self, v: bool) -> Self {
        self.footer_visible = v;
        self
    }

    pub fn extra(mut self, t: impl Into<String>) -> Self {
        self.extra = t.into();
        self
    }

    /// 设置打开时播放的动画；已打开时从当前声明重新开始进场。
    pub fn enter_animation(mut self, animation: AnimationConfig) -> Self {
        self.enter_animation = Some(animation);
        if self.visible && !self.closing {
            self.restart_enter_transition();
        }
        self
    }

    /// 设置关闭时播放的动画。
    pub fn leave_animation(mut self, animation: AnimationConfig) -> Self {
        self.leave_animation = Some(animation);
        if self.closing {
            self.transition = TransitionPlayer::new(animation);
            self.transition_dirty = true;
        }
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visible
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
        self.transition = TransitionPlayer::new(self.resolved_leave_animation());
        self.transition_dirty = true;
        self.layout_requested.set(true);
    }

    pub fn set_visible(&mut self, v: bool) {
        if v {
            self.open();
        } else {
            self.close();
        }
    }

    pub(crate) fn is_present(&self) -> bool {
        self.visible || self.closing
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let interaction_geometry_changed = self.width != next.width
            || self.height != next.height
            || self.placement != next.placement
            || self.closable != next.closable
            || self.mask != next.mask;
        self.title = next.title;
        self.width = next.width;
        self.height = next.height;
        self.drawer_size = next.drawer_size;
        self.placement = next.placement;
        self.closable = next.closable;
        self.mask_closable = next.mask_closable;
        self.mask = next.mask;
        self.footer_visible = next.footer_visible;
        self.extra = next.extra;
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
        if interaction_geometry_changed {
            self.cancel_interaction();
        }
    }

    fn restart_enter_transition(&mut self) {
        let animation = self.resolved_enter_animation();
        let mut transition = TransitionPlayer::new(animation);
        // 与 View 进出场一致：零时长动画在创建后立即完成，避免登记多余帧。
        if animation.duration() <= 0.0 {
            transition.update(0.0);
        }
        self.transition = transition;
        self.transition_dirty = true;
    }

    fn panel_rect_local(&self) -> Rect {
        let frame = self.last_frame.get();
        let panel = self.last_panel_rect.get();
        let panel = if panel.w > 0.0 && panel.h > 0.0 {
            panel
        } else if self.mask {
            self.overlay_rect_for_surface(
                self.last_surface_w.get().max(1200.0),
                self.last_surface_h.get().max(760.0),
            )
        } else {
            match self.placement {
                DrawerPlacement::Right | DrawerPlacement::Left => {
                    Rect::new(frame.x, frame.y, self.width, frame.h)
                }
                DrawerPlacement::Top | DrawerPlacement::Bottom => {
                    Rect::new(frame.x, frame.y, frame.w, self.height)
                }
            }
        };
        Rect::new(panel.x - frame.x, panel.y - frame.y, panel.w, panel.h)
    }

    fn close_rect_local(&self) -> Rect {
        let panel = self.panel_rect_local();
        let close_width = panel.w.min(48.0);
        Rect::new(
            panel.x + panel.w - close_width,
            panel.y,
            close_width,
            panel.h.min(48.0),
        )
    }

    pub(super) fn pointer_target_at(&self, pos: Point) -> Option<DrawerPointerTarget> {
        if self.closable && self.close_rect_local().contains(pos) {
            Some(DrawerPointerTarget::Close)
        } else if self.mask_closable && !self.panel_rect_local().contains(pos) {
            Some(DrawerPointerTarget::Mask)
        } else {
            None
        }
    }

    pub(super) fn cancel_interaction(&self) {
        self.close_hovered.set(false);
        self.pressed_target.set(None);
        self.activation_key.set(None);
    }

    pub(super) fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            Self::normalize_dimension(frame.w),
            Self::normalize_dimension(frame.h),
        )
    }

    pub(super) fn transition_opacity(&self) -> f32 {
        self.transition.opacity_progress.clamp(0.0, 1.0)
    }

    pub(super) fn apply_transition_to_rect(&self, rect: Rect) -> Rect {
        Rect::new(
            rect.x + self.transition.offset.x,
            rect.y + self.transition.offset.y,
            rect.w,
            rect.h,
        )
    }

    pub(super) fn overlay_rect_for_surface(&self, surface_w: f32, surface_h: f32) -> Rect {
        let surface_w = Self::normalize_dimension(surface_w);
        let surface_h = Self::normalize_dimension(surface_h);
        let width = Self::normalize_dimension(self.width).min(surface_w);
        let height = Self::normalize_dimension(self.height).min(surface_h);
        match self.placement {
            DrawerPlacement::Right => Rect::new(surface_w - width, 0.0, width, surface_h),
            DrawerPlacement::Left => Rect::new(0.0, 0.0, width, surface_h),
            DrawerPlacement::Top => Rect::new(0.0, 0.0, surface_w, height),
            DrawerPlacement::Bottom => Rect::new(0.0, surface_h - height, surface_w, height),
        }
    }

    pub(super) fn trigger_rect_for_size(frame_w: f32, frame_h: f32) -> Rect {
        let frame_w = Self::normalize_dimension(frame_w);
        let frame_h = Self::normalize_dimension(frame_h);
        let width = frame_w.min(96.0);
        Rect::new((frame_w - width) * 0.5, 0.0, width, frame_h.min(32.0))
    }

    pub(super) fn trigger_rect_local(&self) -> Rect {
        let trigger = self.last_trigger_rect.get();
        if trigger.w > 0.0 && trigger.h > 0.0 {
            trigger
        } else {
            Rect::new(0.0, 0.0, 96.0, 32.0)
        }
    }

    pub(super) fn body_rect(&self, panel: Rect) -> Rect {
        let header_h = panel.h.min(48.0);
        let footer_h = if self.footer_visible {
            (panel.h - header_h).clamp(0.0, 56.0)
        } else {
            0.0
        };
        Rect::new(
            panel.x,
            panel.y + header_h,
            panel.w.max(0.0),
            (panel.h - header_h - footer_h).max(0.0),
        )
    }

    pub(super) fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    pub(super) fn paint_elided_text(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        if value.is_empty() || frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let value = value.replace(['\r', '\n'], " ");
        let visible = if Self::text_width(ctx, &value, font_size) <= frame.w {
            value
        } else {
            const ELLIPSIS: char = '…';
            let mut visible = String::new();
            for ch in value.chars() {
                visible.push(ch);
                visible.push(ELLIPSIS);
                let fits = Self::text_width(ctx, &visible, font_size) <= frame.w;
                visible.pop();
                if !fits {
                    visible.pop();
                    break;
                }
            }
            visible.push(ELLIPSIS);
            visible
        };
        ctx.push_clip(frame);
        let y = ctx.visual_center_y(frame, font_size);
        ctx.draw_text(&visible, Point::new(frame.x, y), color, font_size);
        ctx.pop_clip();
    }

    fn text_width(ctx: &mut PaintContext, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::resources::font::text_backend::estimate_text_metrics(
                value,
                f32::INFINITY,
                font_size,
            )
            .max_line_width,
        )
    }

    fn animation_placement_for(placement: DrawerPlacement) -> crate::ui::Placement {
        match placement {
            DrawerPlacement::Right => crate::ui::Placement::Right,
            DrawerPlacement::Left => crate::ui::Placement::Left,
            DrawerPlacement::Top => crate::ui::Placement::Top,
            DrawerPlacement::Bottom => crate::ui::Placement::Bottom,
        }
    }

    fn resolved_enter_animation(&self) -> AnimationConfig {
        self.enter_animation
            .unwrap_or_else(|| presets::drawer_enter(Self::animation_placement_for(self.placement)))
    }

    fn resolved_leave_animation(&self) -> AnimationConfig {
        self.leave_animation
            .unwrap_or_else(|| presets::drawer_exit(Self::animation_placement_for(self.placement)))
    }

    pub(super) fn intrinsic_size(&self) -> Size {
        if self.is_present() {
            if self.mask {
                // Masked drawer paints in overlay space; keep layout slot empty.
                Size::zero()
            } else {
                match self.placement {
                    DrawerPlacement::Right | DrawerPlacement::Left => {
                        Size::new(Self::normalize_dimension(self.width), 600.0)
                    }
                    DrawerPlacement::Top | DrawerPlacement::Bottom => {
                        Size::new(400.0, Self::normalize_dimension(self.height))
                    }
                }
            }
        } else {
            Size::new(96.0, 32.0)
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Drawer {
            title: self.title.clone(),
            open: self.is_present(),
            width: self.width,
            height: self.height,
            drawer_size: self.drawer_size,
            placement: self.placement,
            closable: self.closable,
            mask_closable: self.mask_closable,
            mask: self.mask,
            footer_visible: self.footer_visible,
            extra: self.extra.clone(),
        }
    }
}
