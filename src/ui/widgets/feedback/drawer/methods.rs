//! 抽屉行为实现。

use super::*;

impl Drawer {
    /// 创建默认关闭、从右侧进入且带遮罩和关闭按钮的抽屉。
    pub fn new(title: &str) -> Self {
        let size = crate::ui::widget_runtime::config::use_config().size;
        // 直接构造与 UIX View 构建共同读取同一视觉静态项。
        let visual = DRAWER_VISUAL_REF;
        // 三档尺寸只由 UIX 默认表决定。
        let (width, height) = visual.defaults.dimensions(size);
        // 保存默认方向，供初始动画使用同一事实。
        let placement = visual.defaults.placement;
        Self {
            title: title.into(),
            visible: false,
            width,
            height,
            drawer_size: size,
            placement,
            closable: visual.defaults.closable,
            mask_closable: visual.defaults.mask_closable,
            mask: visual.defaults.mask,
            // backdrop blur 默认关闭，遵守显式 opt-in 决策。
            backdrop_blur: None,
            footer_visible: visual.defaults.footer_visible,
            extra: String::new().into_boxed_str(),
            enter_animation: None,
            leave_animation: None,
            // 默认不绑定外部状态，保留现有非受控构造契约。
            controlled: None,
            transition: TransitionPlayer::new(
                AnimationConfig::slide_in(
                    Self::animation_placement_for(placement),
                    visual.motion.enter_duration,
                )
                .with_distance(visual.motion.distance),
            ),
            closing: false,
            transition_dirty: false,
            layout_requested: Cell::new(false),
            last_frame: Cell::new(Rect::zero()),
            last_surface_w: Cell::new(0.0),
            last_surface_h: Cell::new(0.0),
            last_trigger_rect: Cell::new(Rect::new(
                0.0,
                0.0,
                visual.layout.trigger_width,
                visual.layout.trigger_height,
            )),
            last_panel_rect: Cell::new(Rect::zero()),
            close_hovered: Cell::new(false),
            pressed_target: Cell::new(None),
            activation_key: Cell::new(None),
            visual,
        }
    }

    /// 设置初始可见性，并通过正式打开或关闭生命周期应用状态。
    pub fn visible(mut self, v: bool) -> Self {
        self.set_visible(v);
        self
    }

    /// 立即打开抽屉并返回可继续配置的实例。
    pub fn show(mut self) -> Self {
        self.open();
        self
    }

    // 绑定声明端唯一的打开状态事实源。
    /// 将打开状态双向绑定到外部响应式状态。
    pub fn controlled_open(mut self, state: &State<bool>) -> Self {
        // 保存共享状态句柄供每帧同步和用户关闭写回。
        self.controlled = Some(ControlledDrawerOpen::new(state));
        // 初始真值直接参与首帧呈现。
        if state.get() {
            // 只设置初始呈现事实，沿用构造时的进场动画。
            self.visible = true;
        }
        // 返回配置完成的 Drawer。
        self
    }

    // 设置横向 Drawer 的面板宽度。
    /// 设置左右侧抽屉使用的非负面板宽度。
    pub fn width(mut self, width: f32) -> Self {
        // 统一归一化非有限值和负值。
        self.width = Self::normalize_dimension(width);
        // 返回配置完成的 Drawer。
        self
    }

    /// 同时设置抽屉的非负面板宽度和高度。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = Self::normalize_dimension(w);
        self.height = Self::normalize_dimension(h);
        self
    }

    /// 设置控件尺寸档位及其对应的预设宽高。
    pub fn drawer_size(mut self, s: ControlSize) -> Self {
        self.drawer_size = s;
        // 预设尺寸与直接构造共同读取 UIX 唯一表。
        (self.width, self.height) = self.visual.defaults.dimensions(s);
        self
    }

    /// 设置抽屉进入的边缘；已打开时会重启进入动画。
    pub fn placement(mut self, p: DrawerPlacement) -> Self {
        self.placement = p;
        if self.visible && !self.closing {
            self.restart_enter_transition();
        }
        self
    }

    /// 设置是否显示并响应标题区关闭按钮。
    pub fn closable(mut self, v: bool) -> Self {
        self.closable = v;
        self
    }

    /// 设置点击遮罩区域是否关闭抽屉。
    pub fn mask_closable(mut self, v: bool) -> Self {
        self.mask_closable = v;
        self
    }

    /// 设置抽屉打开时是否绘制背景遮罩。
    pub fn mask(mut self, v: bool) -> Self {
        self.mask = v;
        self
    }

    /// 为 Drawer 显式启用统一 backdrop blur 请求。
    pub fn backdrop_blur(mut self, blur: crate::ui::OverlayBackdropBlur) -> Self {
        // 保存 typed 请求，Theme 与区域由当前表面统一解析。
        self.backdrop_blur = Some(blur);
        // 返回配置完成的 Drawer。
        self
    }

    /// 设置底部操作区域是否可见。
    pub fn footer_visible(mut self, v: bool) -> Self {
        self.footer_visible = v;
        self
    }

    /// 设置标题区末尾显示的附加文本。
    pub fn extra(mut self, t: impl Into<String>) -> Self {
        // 收缩为精确容量不可变文本，减少每实例闲置内存。
        self.extra = t.into().into_boxed_str();
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

    /// 返回抽屉是否处于稳定打开状态。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 打开抽屉、启动进入动画并同步受控状态。
    pub fn open(&mut self) {
        // 记录调用前是否已处于稳定打开态，保证状态写回幂等。
        let was_open = self.visible && !self.closing;
        // 稳定打开态不重复启动进场动画。
        if was_open {
            // 保持当前动画与交互状态。
            return;
        }
        // 进入 Drawer 自身拥有的唯一打开转换。
        self.do_open();
        // 受控实例把用户或公开入口确认的打开事实写回 State。
        if let Some(controlled) = &self.controlled {
            // 写回唯一业务事实源。
            controlled.state.set(true);
        }
    }

    // 请求关闭 Drawer，并在受控模式下幂等写回业务状态。
    /// 关闭抽屉、启动退出动画并同步受控状态。
    pub fn close(&mut self) {
        // 只有稳定打开态可以启动一次离场。
        let was_open = self.visible && !self.closing;
        // 已关闭或正在离场时不得重启关闭动画。
        if !was_open {
            // 完全关闭时清理可能残留的交互状态。
            if !self.is_present() {
                // 取消不可见触发器上的残留按压。
                self.cancel_interaction();
            }
            // 保持既有关闭或离场状态。
            return;
        }
        // 进入 Drawer 自身拥有的唯一关闭转换。
        self.do_close();
        // 受控实例把用户关闭事实写回 State。
        if let Some(controlled) = &self.controlled {
            // 写回唯一业务事实源，重复关闭不会再次执行。
            controlled.state.set(false);
        }
    }

    // 执行不写回外部状态的内部打开转换。
    fn do_open(&mut self) {
        // 清理上一生命周期遗留的输入状态。
        self.cancel_interaction();
        // 标记稳定打开。
        self.visible = true;
        // 取消可能进行中的离场。
        self.closing = false;
        // 从当前 placement 重启进场动画。
        self.restart_enter_transition();
        // 请求布局重新登记覆盖层与内容区域。
        self.layout_requested.set(true);
    }

    // 执行不写回外部状态的内部关闭转换。
    fn do_close(&mut self) {
        // 清理关闭前的输入状态。
        self.cancel_interaction();
        // 防御内部重复调用，不重启离场动画。
        if !self.is_present() {
            // 保持完全关闭。
            self.visible = false;
            // 清除离场标记。
            self.closing = false;
            // 清除动画脏标记。
            self.transition_dirty = false;
            // 结束内部关闭路径。
            return;
        }
        // 关闭稳定可见性，但在离场完成前保留呈现资格。
        self.visible = false;
        // 标记进入离场。
        self.closing = true;
        // 使用当前 placement 和声明动画建立唯一离场播放器。
        self.transition = TransitionPlayer::new(self.resolved_leave_animation());
        // 标记动画区域需要重绘。
        self.transition_dirty = true;
        // 请求布局继续保留覆盖层直到离场完成。
        self.layout_requested.set(true);
    }

    /// 通过正式生命周期打开或关闭抽屉。
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

    // 每帧把受控 State 同步到 Drawer 自身拥有的进退场生命周期。
    pub(crate) fn controlled_sync(&mut self) {
        // 只处理显式受控实例。
        if let Some(controlled) = &self.controlled {
            // 读取声明端当前期望值。
            let want_open = controlled.want_open();
            // 稳定打开态才算已经满足真值。
            let is_open = self.visible && !self.closing;
            // 仅在事实不一致时执行生命周期转换。
            if want_open != is_open {
                // 外部真值允许取消离场并重新打开。
                if want_open {
                    // 内部同步不反向写回同一 State。
                    self.do_open();
                } else {
                    // 外部假值走正常离场但不重复写回。
                    self.do_close();
                }
            }
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // 记录声明式可见性变化，避免关闭动画期间每次重建都重启离场。
        let visibility_changed = self.visible != next.visible;
        // 只有声明值真正变化时才同步运行态，保留进行中的离场动画。
        if visibility_changed {
            // 声明式打开需要重置关闭状态并启动进入动画。
            if next.visible {
                // 复用正式打开路径，确保 present、transition 和布局请求一致。
                self.do_open();
            } else {
                // 复用正式关闭路径，确保离场动画只启动一次。
                self.do_close();
            }
        }
        let interaction_geometry_changed = self.width != next.width
            || self.height != next.height
            || self.placement != next.placement
            || self.closable != next.closable
            || self.mask != next.mask
            || !std::ptr::eq(self.visual, next.visual);
        self.title = next.title;
        self.width = next.width;
        self.height = next.height;
        self.drawer_size = next.drawer_size;
        self.placement = next.placement;
        self.closable = next.closable;
        self.mask_closable = next.mask_closable;
        self.mask = next.mask;
        // 声明重建同步最新 backdrop blur 请求。
        self.backdrop_blur = next.backdrop_blur;
        self.footer_visible = next.footer_visible;
        self.extra = next.extra;
        // 声明重建提供新绑定时替换当前受控句柄。
        if next.controlled.is_some() {
            // 采用新声明指向的唯一 State 事实源。
            self.controlled = next.controlled;
        }
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
        // UIX 静态项按共享引用替换，不复制完整视觉表。
        self.visual = next.visual;
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
                self.last_surface_w
                    .get()
                    .max(self.visual.layout.surface_fallback_width),
                self.last_surface_h
                    .get()
                    .max(self.visual.layout.surface_fallback_height),
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
        let close_width = panel.w.min(self.visual.layout.close_width);
        Rect::new(
            panel.x + panel.w - close_width,
            panel.y,
            close_width,
            panel.h.min(self.visual.layout.header_height),
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

    pub(super) fn trigger_rect_for_size(&self, frame_w: f32, frame_h: f32) -> Rect {
        let frame_w = Self::normalize_dimension(frame_w);
        let frame_h = Self::normalize_dimension(frame_h);
        let width = frame_w.min(self.visual.layout.trigger_width);
        Rect::new(
            (frame_w - width) * 0.5,
            0.0,
            width,
            frame_h.min(self.visual.layout.trigger_height),
        )
    }

    pub(super) fn trigger_rect_local(&self) -> Rect {
        let trigger = self.last_trigger_rect.get();
        if trigger.w > 0.0 && trigger.h > 0.0 {
            trigger
        } else {
            Rect::new(
                0.0,
                0.0,
                self.visual.layout.trigger_width,
                self.visual.layout.trigger_height,
            )
        }
    }

    pub(super) fn body_rect(&self, panel: Rect) -> Rect {
        let header_h = panel.h.min(self.visual.layout.header_height);
        let footer_h = if self.footer_visible {
            (panel.h - header_h).clamp(0.0, self.visual.layout.footer_height)
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
        // 复用共享算法，并在极窄宽度连省略号也放不下时停止绘制。
        let Some(visible) = ctx.elide_single_line(value, font_size, frame.w) else {
            // 保持组件原有的无可见文本早退策略。
            return;
            // 结束极窄宽度分支。
        };
        ctx.push_clip(frame);
        let y = ctx.visual_center_y(frame, font_size);
        ctx.draw_text(&visible, Point::new(frame.x, y), color, font_size);
        ctx.pop_clip();
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
        self.enter_animation.unwrap_or_else(|| {
            AnimationConfig::slide_in(
                Self::animation_placement_for(self.placement),
                self.visual.motion.enter_duration,
            )
            .with_distance(self.visual.motion.distance)
        })
    }

    fn resolved_leave_animation(&self) -> AnimationConfig {
        self.leave_animation.unwrap_or_else(|| {
            AnimationConfig::slide_out(
                Self::animation_placement_for(self.placement),
                self.visual.motion.leave_duration,
            )
            .with_distance(self.visual.motion.distance)
        })
    }

    pub(super) fn intrinsic_size(&self) -> Size {
        if self.is_present() {
            if self.mask {
                // Masked drawer paints in overlay space; keep layout slot empty.
                Size::zero()
            } else {
                match self.placement {
                    DrawerPlacement::Right | DrawerPlacement::Left => Size::new(
                        Self::normalize_dimension(self.width),
                        self.visual.layout.open_vertical_extent,
                    ),
                    DrawerPlacement::Top | DrawerPlacement::Bottom => Size::new(
                        self.visual.layout.open_horizontal_extent,
                        Self::normalize_dimension(self.height),
                    ),
                }
            }
        } else {
            Size::new(
                self.visual.layout.trigger_width,
                self.visual.layout.trigger_height,
            )
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Drawer {
            title: self.title.to_string(),
            open: self.is_present(),
            width: self.width,
            height: self.height,
            drawer_size: self.drawer_size,
            placement: self.placement,
            closable: self.closable,
            mask_closable: self.mask_closable,
            mask: self.mask,
            // 快照纳入效果请求，确保声明式变更触发 reconcile。
            backdrop_blur: self.backdrop_blur,
            footer_visible: self.footer_visible,
            extra: self.extra.to_string(),
        }
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/feedback/drawer/methods__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
