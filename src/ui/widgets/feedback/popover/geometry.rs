//! 气泡卡片行为与几何。

use super::*;

// 复用浮层共享定位机制：方向候选、翻转、溢出评分与表面钳制的单一实现。
use crate::ui::widgets::binding::write_if_changed;
use crate::ui::widgets::overlay::{
    OverlayArrowVisual, OverlayBubbleGeometry, OverlayPlacement, draw_overlay_arrow,
    normalize_rect, resolve_overlay_bubble,
};

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
        write_if_changed(self.open_binding.as_ref(), open);
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
        let popup = expand_rect(
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
        // 返回同一解析器生成的最终气泡矩形。
        .bubble
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
        // 复用浮层共享归一化：非有限坐标回退原点、尺寸收敛为有限非负。
        normalize_rect(frame)
    }
}

// 保存气泡卡片经过翻转与表面约束后的最终几何。
// 复用共享气泡几何：最终矩形与实际方向绑定。
pub(crate) type PopoverGeometry = OverlayBubbleGeometry<PopoverPlacement>;

pub(super) fn resolve_popover_geometry(
    trigger: Rect,
    surface: Rect,
    placement: PopoverPlacement,
    arrow: bool,
    visual: &PopoverVisual,
) -> PopoverGeometry {
    // 箭头开启时使用箭头间距，否则使用无箭头间距。
    let gap = if arrow {
        visual.layout.arrow_gap
    } else {
        visual.layout.plain_gap
    };
    // 复用共享气泡定位解析：尺寸收敛、翻转与钳制在单一实现内完成。
    resolve_overlay_bubble(
        placement,
        trigger,
        surface,
        visual.defaults.popup_width,
        visual.defaults.popup_height,
        gap,
        visual.layout.center_ratio,
    )
}

pub(super) fn translated_rect(rect: Rect, offset: Point) -> Rect {
    Rect::new(rect.x + offset.x, rect.y + offset.y, rect.w, rect.h)
}

pub(super) fn draw_popover_arrow(
    ctx: &mut PaintContext,
    trigger: Rect,
    popup: Rect,
    placement: PopoverPlacement,
    color: Color,
    visual: &PopoverLayoutVisual,
) {
    // 复用共享箭头绘制：popover 箭头不嵌入气泡边缘、尖端恒居中。
    draw_overlay_arrow(
        ctx,
        trigger,
        popup,
        placement.decompose().0,
        color,
        OverlayArrowVisual {
            size: visual.arrow_size,
            edge_overlap: 0.0,
            tip_ratio: 0.5,
            center_ratio: visual.center_ratio,
        },
    );
}
