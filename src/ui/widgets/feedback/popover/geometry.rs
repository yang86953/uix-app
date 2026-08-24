//! 气泡卡片行为与几何。

use super::*;

impl Popover {
    /// 创建默认置于上方、点击触发且初始隐藏的气泡卡片。
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            title: String::new(),
            content: content.into(),
            visible: false,
            placement: POPOVER_VISUAL.defaults.placement,
            trigger: PopoverTrigger::Click,
            arrow: POPOVER_VISUAL.defaults.arrow,
            background: None,
            custom_trigger: false,
            custom_trigger_view: None,
            open_binding: None,
            timer: 0.0,
            enter_animation: AnimationConfig::fade_in(POPOVER_VISUAL.motion.enter_duration),
            leave_animation: AnimationConfig::fade_out(POPOVER_VISUAL.motion.exit_duration),
            transition: TransitionPlayer::new(AnimationConfig::fade_in(
                POPOVER_VISUAL.motion.enter_duration,
            )),
            closing: false,
            transition_dirty: false,
            focused: false,
            hovered: false,
            pressed_target: None,
            pressed_key: None,
            last_frame: Cell::new(Rect::zero()),
            popup_rect: Cell::new(Rect::new(
                0.0,
                -POPOVER_VISUAL.defaults.popup_height - POPOVER_VISUAL.layout.arrow_gap,
                POPOVER_VISUAL.defaults.popup_width,
                POPOVER_VISUAL.defaults.popup_height,
            )),
            surface_rect: Cell::new(Rect::zero()),
            visual: POPOVER_VISUAL_REF,
            authored: PopoverAuthored::default(),
        }
    }
    /// 设置气泡卡片标题。
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }
    /// 设置气泡卡片相对触发区域的放置方向。
    pub fn placement(mut self, p: PopoverPlacement) -> Self {
        self.placement = p;
        self.authored.set(PopoverAuthored::PLACEMENT);
        self
    }
    /// 设置打开和关闭气泡卡片的触发方式。
    pub fn trigger(mut self, t: PopoverTrigger) -> Self {
        self.trigger = t;
        self
    }
    /// 设置是否绘制指向触发区域的箭头。
    pub fn arrow(mut self, v: bool) -> Self {
        self.arrow = v;
        self.authored.set(PopoverAuthored::ARROW);
        self
    }

    /// 设置气泡卡片背景颜色。
    pub fn bg(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// 设置替代默认触发区域的自定义 View。
    pub fn trigger_view<V: crate::ui::view::View>(mut self, trigger: V) -> Self {
        self.custom_trigger = true;
        self.custom_trigger_view = Some(Rc::new(RefCell::new(Some(crate::ui::view::View::build(
            trigger,
        )))));
        self
    }

    /// 设置打开时播放的动画；已打开时从当前声明重新开始进场。
    pub fn enter_animation(mut self, animation: AnimationConfig) -> Self {
        self.enter_animation = animation;
        self.authored.set(PopoverAuthored::ENTER_ANIMATION);
        if self.visible && !self.closing {
            self.transition = TransitionPlayer::new(animation);
            self.transition_dirty = true;
        }
        self
    }

    /// 设置关闭时播放的动画。
    pub fn leave_animation(mut self, animation: AnimationConfig) -> Self {
        self.leave_animation = animation;
        self.authored.set(PopoverAuthored::LEAVE_ANIMATION);
        if self.closing {
            self.transition = TransitionPlayer::new(animation);
            self.transition_dirty = true;
        }
        self
    }

    /// 返回气泡卡片是否处于可见阶段。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 返回气泡卡片是否可见或仍在执行关闭过渡。
    pub fn is_present(&self) -> bool {
        self.visible || self.closing
    }

    /// 打开气泡卡片、启动进场动画并同步受控状态。
    pub fn open(&mut self) {
        self.open_now();
        self.write_bound_open(true);
    }

    /// 将打开状态双向绑定到外部 [`State<bool>`]。
    pub fn controlled_open(mut self, state: &State<bool>) -> Self {
        self.open_binding = Some(state.clone());
        self.apply_bound_open(state.get());
        self
    }

    /// 设置打开状态，并同步受控状态及相应过渡动画。
    pub fn set_open(&mut self, open: bool) {
        if open {
            self.open();
        } else {
            self.close();
        }
    }

    fn open_now(&mut self) {
        self.pressed_target = None;
        self.pressed_key = None;
        self.visible = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(self.enter_animation);
        self.transition_dirty = true;
    }

    /// 关闭气泡卡片、启动离场动画并同步受控状态。
    pub fn close(&mut self) {
        self.close_now();
        self.write_bound_open(false);
    }

    fn close_now(&mut self) {
        self.pressed_target = None;
        self.pressed_key = None;
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
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let geometry_changed = self.placement != next.placement || self.arrow != next.arrow;
        let trigger_changed = self.trigger != next.trigger;
        self.title = next.title;
        self.content = next.content;
        self.placement = next.placement;
        self.trigger = next.trigger;
        self.arrow = next.arrow;
        self.background = next.background;
        self.custom_trigger = next.custom_trigger;
        self.custom_trigger_view = next.custom_trigger_view;
        let controlled_open = next.open_binding.as_ref().map(|_| next.visible);
        self.open_binding = next.open_binding;
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
        self.visual = next.visual;
        self.authored = next.authored;
        if let Some(open) = controlled_open {
            self.apply_bound_open(open);
        }
        if geometry_changed || trigger_changed {
            self.cancel_pending_activation();
        }
    }

    pub(super) fn toggle(&mut self) {
        if self.is_present() && !self.closing {
            self.close();
        } else {
            self.open();
        }
    }

    pub(super) fn sync_bound_open(&mut self) {
        let open = self.open_binding.as_ref().map(State::get);
        if let Some(open) = open {
            self.apply_bound_open(open);
        }
    }

    fn apply_bound_open(&mut self, open: bool) {
        if open {
            if !self.visible || self.closing {
                self.open_now();
            }
        } else if self.visible && !self.closing {
            self.close_now();
        } else if !self.is_present() {
            self.visible = false;
            self.closing = false;
        }
    }

    fn write_bound_open(&self, open: bool) {
        if let Some(state) = self.open_binding.as_ref() {
            if state.get() != open {
                state.set(open);
            }
        }
    }

    pub(super) fn cancel_pending_activation(&mut self) {
        self.pressed_target = None;
        self.pressed_key = None;
    }

    pub(super) fn trigger_rect(&self) -> Rect {
        let frame = self.last_frame.get();
        let width = if frame.w > 0.0 {
            frame.w
        } else {
            self.visual.defaults.trigger_width
        };
        let height = if frame.h > 0.0 {
            frame.h
        } else {
            self.visual.defaults.trigger_height
        };
        Rect::new(0.0, 0.0, width, height)
    }

    pub(super) fn transitioned_rect(&self, rect: Rect) -> Rect {
        let scale = self.transition.scale.max(0.0);
        let width = rect.w * scale;
        let height = rect.h * scale;
        Rect::new(
            rect.x + (rect.w - width) * self.visual.layout.center_ratio + self.transition.offset.x,
            rect.y + (rect.h - height) * self.visual.layout.center_ratio + self.transition.offset.y,
            width,
            height,
        )
    }

    pub(super) fn transition_sweep_rect(&self, rect: Rect) -> Rect {
        let (from, to) = self.transition.offset_endpoints();
        rect.union(&translated_rect(rect, from))
            .union(&translated_rect(rect, to))
    }

    pub(super) fn transition_dirty_rect(&self, frame: Rect) -> Rect {
        let frame = Self::normalize_frame(frame);
        let popup = expand_popover_rect(
            self.absolute_popup_rect(frame),
            self.visual.layout.shadow_expand,
        );
        let bounds = frame.union(&self.transition_sweep_rect(popup));
        bounds
            .intersect(&self.surface_or_fallback(frame))
            .unwrap_or_default()
    }

    pub(super) fn intrinsic_size(&self) -> Size {
        Size::new(
            self.visual.defaults.trigger_width,
            self.visual.defaults.trigger_height,
        )
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Popover {
            title: self.title.clone(),
            content: self.content.clone(),
            placement: self.placement,
            trigger: self.trigger,
            arrow: self.arrow,
            visible: self.visible,
        }
    }

    pub(super) fn absolute_popup_rect(&self, frame: Rect) -> Rect {
        // 始终以当前表面重新解析，避免触发器不动时沿用旧窗口边界下的缓存。
        resolve_popover_geometry(
            // 传入当前触发器矩形。
            frame,
            // 传入布局阶段或绘制阶段记录的最新表面。
            self.surface_or_fallback(frame),
            // 保留作者指定位置。
            self.placement,
            // 保留箭头间距配置。
            self.arrow,
            // 使用 UIX 声明的完整视觉表。
            self.visual,
        )
        // 返回同一解析器生成的最终矩形。
        .popup
    }

    pub(super) fn surface_or_fallback(&self, frame: Rect) -> Rect {
        let surface = self.surface_rect.get();
        if surface.w > 0.0 && surface.h > 0.0 {
            surface
        } else {
            frame.union(&Rect::new(
                frame.x
                    - self.visual.defaults.popup_width * self.visual.layout.fallback_offset_popups,
                frame.y
                    - self.visual.defaults.popup_height * self.visual.layout.fallback_offset_popups,
                self.visual.defaults.popup_width * self.visual.layout.fallback_span_popups
                    + frame.w,
                self.visual.defaults.popup_height * self.visual.layout.fallback_span_popups
                    + frame.h,
            ))
        }
    }

    pub(super) fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    pub(super) fn paint_elided_text(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
        centered: bool,
    ) {
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(value) = ctx.elide_single_line_cow(value, font_size, frame.w) else {
            return;
        };
        if frame.h <= 0.0 {
            return;
        }
        ctx.push_clip(frame);
        if centered {
            ctx.text_center(value.as_ref(), frame, color, font_size);
        } else {
            let y = ctx.visual_center_y(frame, font_size);
            ctx.draw_text(value.as_ref(), Point::new(frame.x, y), color, font_size);
        }
        ctx.pop_clip();
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PopoverGeometry {
    pub(crate) popup: Rect,
    pub(crate) placement: PopoverPlacement,
}

pub(super) fn resolve_popover_geometry(
    trigger: Rect,
    surface: Rect,
    placement: PopoverPlacement,
    arrow: bool,
    visual: &PopoverVisual,
) -> PopoverGeometry {
    let width = visual.defaults.popup_width.min(surface.w).max(0.0);
    let height = visual.defaults.popup_height.min(surface.h).max(0.0);
    if width <= 0.0 || height <= 0.0 {
        return PopoverGeometry {
            popup: Rect::zero(),
            placement,
        };
    }

    let flipped = flip_popover_placement(placement);
    let authored =
        rect_for_popover_placement(trigger, placement, arrow, width, height, &visual.layout);
    let alternate =
        rect_for_popover_placement(trigger, flipped, arrow, width, height, &visual.layout);
    let (candidate, resolved) =
        if overflow_score(alternate, surface) < overflow_score(authored, surface) {
            (alternate, flipped)
        } else {
            (authored, placement)
        };
    let max_x = surface.x + surface.w - width;
    let max_y = surface.y + surface.h - height;
    PopoverGeometry {
        popup: Rect::new(
            candidate.x.clamp(surface.x, max_x),
            candidate.y.clamp(surface.y, max_y),
            width,
            height,
        ),
        placement: resolved,
    }
}

pub(super) fn rect_for_popover_placement(
    frame: Rect,
    placement: PopoverPlacement,
    arrow: bool,
    width: f32,
    height: f32,
    visual: &PopoverLayoutVisual,
) -> Rect {
    let (x, y) = popover_position(frame, placement, arrow, width, height, visual);
    Rect::new(x, y, width, height)
}

pub(super) fn overflow_score(rect: Rect, surface: Rect) -> f32 {
    (surface.x - rect.x).max(0.0)
        + (surface.y - rect.y).max(0.0)
        + (rect.x + rect.w - surface.x - surface.w).max(0.0)
        + (rect.y + rect.h - surface.y - surface.h).max(0.0)
}

pub(super) fn flip_popover_placement(placement: PopoverPlacement) -> PopoverPlacement {
    match placement {
        PopoverPlacement::Top => PopoverPlacement::Bottom,
        PopoverPlacement::TopLeft => PopoverPlacement::BottomLeft,
        PopoverPlacement::TopRight => PopoverPlacement::BottomRight,
        PopoverPlacement::Bottom => PopoverPlacement::Top,
        PopoverPlacement::BottomLeft => PopoverPlacement::TopLeft,
        PopoverPlacement::BottomRight => PopoverPlacement::TopRight,
        PopoverPlacement::Left => PopoverPlacement::Right,
        PopoverPlacement::LeftTop => PopoverPlacement::RightTop,
        PopoverPlacement::LeftBottom => PopoverPlacement::RightBottom,
        PopoverPlacement::Right => PopoverPlacement::Left,
        PopoverPlacement::RightTop => PopoverPlacement::LeftTop,
        PopoverPlacement::RightBottom => PopoverPlacement::LeftBottom,
    }
}

pub(super) fn popover_position(
    frame: Rect,
    placement: PopoverPlacement,
    arrow: bool,
    pw: f32,
    ph: f32,
    visual: &PopoverLayoutVisual,
) -> (f32, f32) {
    let gap = if arrow {
        visual.arrow_gap
    } else {
        visual.plain_gap
    };
    match placement {
        PopoverPlacement::Top | PopoverPlacement::TopLeft => (frame.x, frame.y - ph - gap),
        PopoverPlacement::TopRight => (frame.x + frame.w - pw, frame.y - ph - gap),
        PopoverPlacement::Bottom | PopoverPlacement::BottomLeft => {
            (frame.x, frame.y + frame.h + gap)
        }
        PopoverPlacement::BottomRight => (frame.x + frame.w - pw, frame.y + frame.h + gap),
        PopoverPlacement::Left => (
            frame.x - pw - gap,
            frame.y + frame.h * visual.center_ratio - ph * visual.center_ratio,
        ),
        PopoverPlacement::LeftTop => (frame.x - pw - gap, frame.y),
        PopoverPlacement::LeftBottom => (frame.x - pw - gap, frame.y + frame.h - ph),
        PopoverPlacement::Right => (
            frame.x + frame.w + gap,
            frame.y + frame.h * visual.center_ratio - ph * visual.center_ratio,
        ),
        PopoverPlacement::RightTop => (frame.x + frame.w + gap, frame.y),
        PopoverPlacement::RightBottom => (frame.x + frame.w + gap, frame.y + frame.h - ph),
    }
}

pub(super) fn translated_rect(rect: Rect, offset: Point) -> Rect {
    Rect::new(rect.x + offset.x, rect.y + offset.y, rect.w, rect.h)
}

pub(super) fn expand_popover_rect(rect: Rect, amount: f32) -> Rect {
    if rect.w <= 0.0 || rect.h <= 0.0 {
        Rect::zero()
    } else {
        Rect::new(
            rect.x - amount,
            rect.y - amount,
            rect.w + amount * 2.0,
            rect.h + amount * 2.0,
        )
    }
}

pub(super) fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

pub(super) fn draw_popover_arrow(
    ctx: &mut PaintContext,
    trigger: Rect,
    popup: Rect,
    placement: PopoverPlacement,
    color: Color,
    visual: &PopoverLayoutVisual,
) {
    let arrow_sz = visual.arrow_size;
    let (x1, y1, x2, y2, x3, y3) = match placement {
        PopoverPlacement::Top | PopoverPlacement::TopLeft | PopoverPlacement::TopRight => {
            let cx = arrow_anchor(
                trigger.x + trigger.w * visual.center_ratio,
                popup.x,
                popup.w,
                arrow_sz,
                visual.center_ratio,
            );
            (
                cx - arrow_sz,
                popup.y + popup.h,
                cx + arrow_sz,
                popup.y + popup.h,
                cx,
                popup.y + popup.h + arrow_sz,
            )
        }
        PopoverPlacement::Bottom | PopoverPlacement::BottomLeft | PopoverPlacement::BottomRight => {
            let cx = arrow_anchor(
                trigger.x + trigger.w * visual.center_ratio,
                popup.x,
                popup.w,
                arrow_sz,
                visual.center_ratio,
            );
            (
                cx - arrow_sz,
                popup.y,
                cx + arrow_sz,
                popup.y,
                cx,
                popup.y - arrow_sz,
            )
        }
        PopoverPlacement::Left | PopoverPlacement::LeftTop | PopoverPlacement::LeftBottom => {
            let cy = arrow_anchor(
                trigger.y + trigger.h * visual.center_ratio,
                popup.y,
                popup.h,
                arrow_sz,
                visual.center_ratio,
            );
            (
                popup.x + popup.w,
                cy - arrow_sz,
                popup.x + popup.w,
                cy + arrow_sz,
                popup.x + popup.w + arrow_sz,
                cy,
            )
        }
        PopoverPlacement::Right | PopoverPlacement::RightTop | PopoverPlacement::RightBottom => {
            let cy = arrow_anchor(
                trigger.y + trigger.h * visual.center_ratio,
                popup.y,
                popup.h,
                arrow_sz,
                visual.center_ratio,
            );
            (
                popup.x,
                cy - arrow_sz,
                popup.x,
                cy + arrow_sz,
                popup.x - arrow_sz,
                cy,
            )
        }
    };
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    pb.line_to(x3, y3);
    pb.close();
    ctx.fill_path(&pb.build(), color, FillRule::NonZero);
}

pub(super) fn arrow_anchor(
    desired: f32,
    start: f32,
    length: f32,
    inset: f32,
    center_ratio: f32,
) -> f32 {
    if length <= inset * 2.0 {
        start + length * center_ratio
    } else {
        desired.clamp(start + inset, start + length - inset)
    }
}

// 仅在测试构建中编译气泡卡片契约。
#[cfg(test)]
// 将测试放在同模块内以核验私有缓存状态。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/feedback/popover/geometry__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
