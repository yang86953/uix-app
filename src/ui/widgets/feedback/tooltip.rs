use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::Color;
use crate::ui::animation::{TransitionPlayer, presets};
use crate::ui::widget_runtime::paint_context::PaintContext;
// 反馈组件复用基础层提示气泡原语。
use crate::ui::SnapshotFields;
use crate::ui::widgets::tooltip_primitives::{
    paint_tooltip_bubble, tooltip_bubble_rect, tooltip_dirty_rect, tooltip_fallback_surface,
};
use crate::ui::{EventResult, KeyCode, MouseButton, SystemEvent, WidgetTree};

// 复用基础层交互模型，并保持 feedback::tooltip 的既有公开路径。
pub use crate::ui::widgets::overlay_types::{TooltipPlacement, TriggerMode};

component! {
    /// 按指定触发方式显示说明文本的浮层提示。
    pub struct Tooltip {
        text: String,
        placement: TooltipPlacement,
        trigger: TriggerMode,
        bg_color: Option<Color>,
        text_color: Option<Color>,
        visible: bool,
        pending: bool,
        delay_ms: u32,
        timer_id: u32,
        arrow: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        pressed_button: Option<MouseButton>,
        pressed_key: Option<KeyCode>,
        last_frame: std::cell::Cell<Rect>,
        // 缓存当前逻辑表面，统一绘制、脏区与浮层登记的边界。
        surface_rect: std::cell::Cell<Option<Rect>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    hit_test_children => (&self) -> bool { false }

    hit_test_frame => (&self, actual_frame: Rect) -> Rect {
        let frame = Self::normalize_frame(actual_frame);
        self.last_frame.set(frame);
        frame
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        let frame = Self::normalize_frame(frame);
        self.last_frame.set(frame);
        children.iter().map(|child| (child.id, frame)).collect()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::FocusIn => return EventResult::Handled,
            SystemEvent::FocusOut => {
                self.cancel_pending_activation();
                if self.trigger == TriggerMode::Focus {
                    self.close();
                }
                return EventResult::Handled;
            }
            SystemEvent::WindowBlur => {
                let changed = self.pending
                    || self.pressed_button.is_some()
                    || self.pressed_key.is_some()
                    || self.is_present();
                self.cancel_pending_activation();
                self.close();
                return if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                };
            }
            SystemEvent::KeyDown { key: KeyCode::Escape, .. } if self.is_present() => {
                self.cancel_pending_activation();
                self.close();
                return EventResult::Handled;
            }
            SystemEvent::Timer { id } if self.pending && *id == self.timer_id => {
                self.open();
                return EventResult::Handled;
            }
            SystemEvent::PointerLeave => {
                let had_activation = self.pressed_button.take().is_some();
                if self.trigger == TriggerMode::Hover {
                    self.pending = false;
                    self.close();
                    return EventResult::Handled;
                }
                if had_activation {
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        match self.trigger {
            TriggerMode::Hover => {
                if let SystemEvent::PointerEnter = event {
                    if self.delay_ms == 0 {
                        self.open();
                    } else {
                        self.close();
                        self.pending = true;
                    }
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            TriggerMode::Focus => EventResult::NotHandled,
            TriggerMode::Click | TriggerMode::ContextMenu => {
                let expected_button = if self.trigger == TriggerMode::ContextMenu {
                    MouseButton::Right
                } else {
                    MouseButton::Left
                };
                match event {
                    SystemEvent::PointerDown { pos, button, .. }
                        if *button == expected_button =>
                    {
                        if self.trigger_rect().contains(*pos) {
                            self.pressed_button = Some(*button);
                            EventResult::Handled
                        } else if self.is_present() {
                            self.close();
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    SystemEvent::PointerUp { pos, button, .. }
                        if *button == expected_button =>
                    {
                        let armed = self.pressed_button.take() == Some(*button);
                        if armed && self.trigger_rect().contains(*pos) {
                            self.toggle();
                        }
                        if armed {
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    SystemEvent::KeyDown {
                        key: key @ (KeyCode::Enter | KeyCode::Space),
                        ..
                    } => {
                        self.pressed_key = Some(*key);
                        EventResult::Handled
                    }
                    SystemEvent::KeyUp {
                        key: key @ (KeyCode::Enter | KeyCode::Space),
                        ..
                    } => {
                        if self.pressed_key.take() == Some(*key) {
                            self.toggle();
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
        }
    }

    on_focus_within => (&mut self, focused: bool) -> EventResult {
        if self.trigger != TriggerMode::Focus {
            return EventResult::NotHandled;
        }
        if focused {
            if self.delay_ms == 0 {
                self.open();
                self.pending = false;
            } else {
                self.close();
                self.pending = true;
            }
        } else {
            self.close();
            self.pending = false;
        }
        EventResult::Handled
    }

    active_timer => (&self) -> Option<(u64, std::time::Duration)> {
        (self.pending && self.delay_ms > 0).then_some((
            u64::from(self.timer_id),
            std::time::Duration::from_millis(u64::from(self.delay_ms)),
        ))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        // 读取本次绘制使用的逻辑表面尺寸。
        let surface_size = ctx.logical_surface_size();
        // 归一化并缓存当前逻辑表面，供脏区与旧登记入口复用。
        self.surface_rect.set(Some(Self::normalize_frame(Rect::new(
            // 表面横坐标固定为窗口原点。
            0.0,
            // 表面纵坐标固定为窗口原点。
            0.0,
            // 使用绘制上下文的逻辑宽度。
            surface_size.w,
            // 使用绘制上下文的逻辑高度。
            surface_size.h,
        ))));
        // 缓存归一化触发器 frame 供交互复用。
        self.last_frame.set(Self::normalize_frame(frame));
        if !self.is_present() {
            return;
        }

        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        // 气泡底：原为 50,50,50 深灰，收敛为黑色 token（AntD 标准 tooltip 色，alpha 保持原值）。
        let bg = fade_color(
            self.bg_color
                .unwrap_or(ctx.tokens().color_black().with_alpha(230)),
            opacity,
        );
        let txt_color = fade_color(
            self.text_color
                .unwrap_or(ctx.tokens().color_white()),
            opacity,
        );
        paint_tooltip_bubble(
            ctx,
            &self.text,
            frame,
            self.placement,
            bg,
            txt_color,
            self.arrow,
        );
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 使用最近布局或绘制获得的表面解析当前脏区。
        tooltip_dirty_rect(
            // 传入提示文字。
            &self.text,
            // 传入箭头开关。
            self.arrow,
            // 传入作者指定方向。
            self.placement,
            // 传入触发器矩形。
            frame,
            // 传入当前表面或有限回退。
            self.surface_or_fallback(frame),
        )
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Tooltip)
                .bounds(tooltip_bubble_rect(
                    &self.text,
                    self.arrow,
                    self.placement,
                    frame,
                    self.surface_or_fallback(frame),
                ))
                .z_index(1100),
        )
    }

    // 使用组件树提供的同帧表面创建提示浮层登记。
    overlay_entry_for_surface => (&self, id: crate::ui::ComponentId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 缓存当前表面，使 dirty、旧入口和动画检查消费同一几何。
        self.surface_rect.set(Some(Self::normalize_frame(surface)));
        // 复用统一的浮层登记逻辑。
        self.overlay_entry(id, frame)
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
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            // 动画脏区也必须使用当前逻辑表面。
            tooltip_dirty_rect(
                // 传入提示文字。
                &self.text,
                // 传入箭头开关。
                self.arrow,
                // 传入作者指定方向。
                self.placement,
                // 传入触发器矩形。
                frame,
                // 传入当前表面或有限回退。
                self.surface_or_fallback(frame),
            )
        } else {
            Rect::zero()
        }
    }
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

impl Default for Tooltip {
    fn default() -> Self {
        Self::new("")
    }
}

impl Tooltip {
    /// 创建默认置于上方、悬停触发且初始隐藏的浮层提示。
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            placement: TooltipPlacement::Top,
            trigger: TriggerMode::Hover,
            bg_color: None,
            text_color: None,
            visible: false,
            pending: false,
            delay_ms: 0,
            timer_id: 1,
            arrow: true,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            pressed_button: None,
            pressed_key: None,
            last_frame: std::cell::Cell::new(Rect::new(0.0, 0.0, 80.0, 28.0)),
            // 新组件尚未接收布局或绘制表面。
            surface_rect: std::cell::Cell::new(None),
        }
    }

    /// 设置浮层相对触发区域的放置方向。
    pub fn placement(mut self, p: TooltipPlacement) -> Self {
        self.placement = p;
        self
    }

    /// 设置打开和关闭浮层的触发方式。
    pub fn trigger(mut self, t: TriggerMode) -> Self {
        self.trigger = t;
        self
    }

    /// 设置浮层背景颜色。
    pub fn bg_color(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }

    /// 设置浮层背景颜色，是 [`Self::bg_color`] 的简写。
    pub fn bg(self, c: Color) -> Self {
        self.bg_color(c)
    }

    /// 设置浮层文本颜色。
    pub fn text_color(mut self, c: Color) -> Self {
        self.text_color = Some(c);
        self
    }

    /// 设置浮层文本颜色，是 [`Self::text_color`] 的简写。
    pub fn color(self, c: Color) -> Self {
        self.text_color(c)
    }

    /// 设置是否绘制指向触发区域的箭头。
    pub fn arrow(mut self, v: bool) -> Self {
        self.arrow = v;
        self
    }

    /// 设置悬停触发后的打开延迟，单位为毫秒。
    pub fn delay_ms(mut self, ms: u32) -> Self {
        self.delay_ms = ms;
        self
    }

    /// 设置用于识别延迟打开事件的计时器标识。
    pub fn timer_id(mut self, id: u32) -> Self {
        self.timer_id = id;
        self
    }

    /// 返回浮层是否处于可见阶段。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 返回浮层是否可见或仍在执行关闭过渡。
    pub fn is_present(&self) -> bool {
        self.visible || self.closing
    }

    /// 取消待处理的激活并启动打开过渡。
    pub fn open(&mut self) {
        self.cancel_pending_activation();
        self.visible = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    /// 取消待处理的激活，并在浮层存在时启动关闭过渡。
    pub fn close(&mut self) {
        self.cancel_pending_activation();
        if !self.is_present() {
            self.visible = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }
        self.visible = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    fn toggle(&mut self) {
        if self.visible && !self.closing {
            self.close();
        } else {
            self.open();
        }
    }

    fn cancel_pending_activation(&mut self) {
        self.pending = false;
        self.pressed_button = None;
        self.pressed_key = None;
    }

    fn trigger_rect(&self) -> Rect {
        let frame = self.last_frame.get();
        let width = if frame.w > 0.0 { frame.w } else { 80.0 };
        let height = if frame.h > 0.0 { frame.h } else { 28.0 };
        Rect::new(0.0, 0.0, width, height)
    }

    fn normalize_frame(frame: Rect) -> Rect {
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

    // 返回布局或绘制记录的当前表面，尚未记录时使用有限回退。
    fn surface_or_fallback(&self, frame: Rect) -> Rect {
        // 优先使用组件树或绘制上下文提供的真实表面。
        self.surface_rect
            // 读取可复制的可选表面缓存。
            .get()
            // 首次登记前根据文字自然尺寸构造有限回退。
            .unwrap_or_else(|| tooltip_fallback_surface(&self.text, frame))
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(80.0, 28.0)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Tooltip {
            text: self.text.clone(),
            placement: self.placement,
            trigger: self.trigger,
            bg_color: self.bg_color,
            text_color: self.text_color,
            delay_ms: self.delay_ms,
            timer_id: self.timer_id,
            arrow: self.arrow,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let trigger_changed = self.trigger != next.trigger;
        self.text = next.text;
        self.placement = next.placement;
        self.trigger = next.trigger;
        self.bg_color = next.bg_color;
        self.text_color = next.text_color;
        self.delay_ms = next.delay_ms;
        self.timer_id = next.timer_id;
        self.arrow = next.arrow;
        if trigger_changed {
            self.pending = false;
            self.pressed_button = None;
            self.pressed_key = None;
        }
    }
}

// 验证反馈提示登记会消费组件树提供的同帧表面。
#[cfg(test)]
// 将表面约束契约限制在当前模块的内部测试中。
mod tests {
    // 复用被测组件与位置枚举。
    use super::{Tooltip, TooltipPlacement};
    // 引入几何基础类型。
    use crate::core::Rect;
    // 引入共享解析器以核对登记与绘制几何同源。
    use crate::ui::widgets::tooltip_primitives::resolve_tooltip_geometry;

    // 靠近表面上边缘时，登记必须使用翻转后的受限气泡。
    #[test]
    // 测试名称说明显式表面入口的职责。
    fn overlay_entry_uses_current_surface_geometry() {
        // 创建作者指定顶部方向的提示组件。
        let mut tooltip = Tooltip::new("tip").placement(TooltipPlacement::Top);
        // 打开提示，使其参与浮层登记。
        tooltip.open();
        // 将触发器放在表面上边缘附近。
        let frame = Rect::new(80.0, 0.0, 40.0, 20.0);
        // 使用足以容纳翻转后气泡的逻辑表面。
        let surface = Rect::new(0.0, 0.0, 200.0, 100.0);
        // 通过组件树使用的显式表面入口创建登记。
        let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
            // 传入被测提示组件。
            &tooltip,
            // 使用稳定的测试组件标识。
            crate::core::ComponentId::new(3),
            // 传入靠近上边缘的触发器。
            frame,
            // 传入当前帧的逻辑表面。
            surface,
        )
        // 打开状态必须生成提示登记。
        .expect("打开的提示应生成浮层登记");
        // 使用同一共享解析器计算预期几何。
        let expected = resolve_tooltip_geometry(
            // 保持提示文字一致。
            "tip",
            // 默认提示带箭头。
            true,
            // 保持作者指定的顶部方向。
            TooltipPlacement::Top,
            // 保持触发器不变。
            frame,
            // 使用当前逻辑表面。
            surface,
        );

        // 上方空间不足时应翻转到底部。
        assert_eq!(expected.placement, TooltipPlacement::Bottom);
        // 浮层登记必须与共享解析器的受限气泡完全一致。
        assert_eq!(overlay.bounds_rect(), Some(expected.bubble));
    }

    // 验证声明刷新只替换配置，不抹除运行时已经建立的可见状态。
    #[test]
    // 测试名称说明 Tooltip 生命周期状态的所有权边界。
    fn refresh_preserves_runtime_visibility() {
        // 创建旧声明对应的运行时组件。
        let mut current = Tooltip::new("旧提示");
        // 通过运行时入口建立可见状态与进入过渡。
        current.open();
        // 创建下一帧声明提供的新文字配置。
        let next = Tooltip::new("新提示");
        // 按组件树协调协议刷新声明字段。
        current.sync_from(next);
        // 声明刷新后运行时可见状态必须继续保留。
        assert!(current.is_visible());
        // 声明刷新应替换新的提示文字。
        assert_eq!(current.text, "新提示");
    }
}
