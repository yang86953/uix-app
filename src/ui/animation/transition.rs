//! widget 进出场动画配置与播放器。
//!
//! [`AnimationConfig`] 是应用侧公开配置；[`TransitionPlayer`] 只负责把配置
//! 展开为单次 opacity / offset / scale 插值。

use crate::core::Point;
use crate::ui::Placement;
use crate::ui::animation::core::Animation;

/// 浮层进入或离场时使用的动画配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationConfig {
    kind: AnimationKind,
    duration: f64,
    distance: f32,
    /// Stagger entry delay applied before the animation starts (0 = none).
    delay: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AnimationKind {
    FadeIn,
    FadeOut,
    SlideIn(Placement),
    SlideOut(Placement),
    ZoomIn,
    ZoomOut,
}

impl AnimationConfig {
    const DEFAULT_SLIDE_DISTANCE: f32 = 24.0;

    /// 创建渐入配置。
    pub fn fade_in(duration: f64) -> Self {
        Self {
            kind: AnimationKind::FadeIn,
            duration: duration.max(0.0),
            distance: 0.0,
            delay: 0.0,
        }
    }

    /// 创建渐出配置。
    pub fn fade_out(duration: f64) -> Self {
        Self {
            kind: AnimationKind::FadeOut,
            duration: duration.max(0.0),
            distance: 0.0,
            delay: 0.0,
        }
    }

    /// 创建从 `placement` 所在方向滑入的配置。
    pub fn slide_in(placement: Placement, duration: f64) -> Self {
        Self {
            kind: AnimationKind::SlideIn(placement),
            distance: Self::DEFAULT_SLIDE_DISTANCE,
            duration: duration.max(0.0),
            delay: 0.0,
        }
    }

    /// 创建向 `placement` 所在方向滑出的配置。
    pub fn slide_out(placement: Placement, duration: f64) -> Self {
        Self {
            kind: AnimationKind::SlideOut(placement),
            distance: Self::DEFAULT_SLIDE_DISTANCE,
            duration: duration.max(0.0),
            delay: 0.0,
        }
    }

    /// 创建缩放渐入配置。
    pub fn zoom_in(duration: f64) -> Self {
        Self {
            kind: AnimationKind::ZoomIn,
            duration: duration.max(0.0),
            distance: 0.0,
            delay: 0.0,
        }
    }

    /// 创建缩放渐出配置。
    pub fn zoom_out(duration: f64) -> Self {
        Self {
            kind: AnimationKind::ZoomOut,
            duration: duration.max(0.0),
            distance: 0.0,
            delay: 0.0,
        }
    }

    /// 附加延迟（stagger 交错入场）；`delay` 秒后开始播放。
    pub fn with_delay(mut self, delay: f64) -> Self {
        self.delay = delay.max(0.0);
        self
    }

    /// 获取附加延迟。
    pub fn delay(self) -> f64 {
        self.delay
    }

    /// 是否为进场动画。
    pub fn is_enter(self) -> bool {
        matches!(
            self.kind,
            AnimationKind::FadeIn | AnimationKind::SlideIn(_) | AnimationKind::ZoomIn
        )
    }

    /// 是否为退场动画。
    pub fn is_exit(self) -> bool {
        matches!(
            self.kind,
            AnimationKind::FadeOut | AnimationKind::SlideOut(_) | AnimationKind::ZoomOut
        )
    }

    /// 获取动画持续时间。
    pub fn duration(self) -> f64 {
        self.duration
    }

    // Drawer capability 启用时才保留其私有滑动距离配置入口。
    #[cfg(feature = "feedback")]
    pub(crate) fn with_distance(mut self, distance: f32) -> Self {
        if matches!(
            self.kind,
            AnimationKind::SlideIn(_) | AnimationKind::SlideOut(_)
        ) {
            self.distance = distance.max(0.0);
        }
        self
    }

    fn opacity_animation(self) -> Animation<f32> {
        match self.kind {
            AnimationKind::FadeIn | AnimationKind::SlideIn(_) | AnimationKind::ZoomIn => {
                Animation::<f32>::fade_in(self.duration())
            }
            AnimationKind::FadeOut | AnimationKind::SlideOut(_) | AnimationKind::ZoomOut => {
                Animation::<f32>::fade_out(self.duration())
            }
        }
    }

    fn offset_animation(self) -> Option<Animation<Point>> {
        match self.kind {
            AnimationKind::SlideIn(placement) => Some(
                Animation::new(
                    placement_offset(placement, self.distance),
                    Point::new(0.0, 0.0),
                    self.duration,
                )
                .easing(crate::ui::animation::easing::Easing::antd_out()),
            ),
            AnimationKind::SlideOut(placement) => Some(
                Animation::new(
                    Point::new(0.0, 0.0),
                    placement_offset(placement, self.distance),
                    self.duration,
                )
                .easing(crate::ui::animation::easing::Easing::antd_in()),
            ),
            _ => None,
        }
    }

    fn scale_animation(self) -> Option<Animation<f32>> {
        match self.kind {
            AnimationKind::ZoomIn => Some(
                Animation::new(0.8, 1.0, self.duration)
                    .easing(crate::ui::animation::easing::Easing::antd_default()),
            ),
            AnimationKind::ZoomOut => Some(
                Animation::new(1.0, 0.8, self.duration)
                    .easing(crate::ui::animation::easing::Easing::antd_default()),
            ),
            _ => None,
        }
    }
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self::fade_in(0.2)
    }
}

fn placement_offset(placement: Placement, distance: f32) -> Point {
    match placement {
        Placement::Top | Placement::TopLeft | Placement::TopRight => Point::new(0.0, -distance),
        Placement::Bottom | Placement::BottomLeft | Placement::BottomRight => {
            Point::new(0.0, distance)
        }
        Placement::Left => Point::new(-distance, 0.0),
        Placement::Right => Point::new(distance, 0.0),
    }
}

/// 内建组件使用的动画预设。
pub(crate) mod presets {
    use super::*;

    // Modal 组件属于 feedback capability。
    #[cfg(feature = "feedback")]
    pub(crate) fn modal_enter() -> AnimationConfig {
        // Modal 默认瞬时出现；需要缩放进场时由调用方显式配置 duration。
        AnimationConfig::zoom_in(0.0)
    }

    // Modal 退场预设只由 feedback capability 消费。
    #[cfg(feature = "feedback")]
    pub(crate) fn modal_exit() -> AnimationConfig {
        AnimationConfig::zoom_out(0.2)
    }

    // Drawer 组件属于 feedback capability。
    #[cfg(feature = "feedback")]
    pub(crate) fn drawer_enter(placement: Placement) -> AnimationConfig {
        // Drawer 默认瞬时滑入到终态；需要过渡时由调用方显式配置 duration。
        AnimationConfig::slide_in(placement, 0.0).with_distance(180.0)
    }

    // Drawer 退场预设只由 feedback capability 消费。
    #[cfg(feature = "feedback")]
    pub(crate) fn drawer_exit(placement: Placement) -> AnimationConfig {
        AnimationConfig::slide_out(placement, 0.2).with_distance(180.0)
    }

    pub(crate) fn tooltip_enter() -> AnimationConfig {
        AnimationConfig::fade_in(0.15)
    }

    pub(crate) fn tooltip_exit() -> AnimationConfig {
        AnimationConfig::fade_out(0.1)
    }

    pub(crate) fn collapse_expand() -> AnimationConfig {
        AnimationConfig::fade_in(0.15)
    }

    pub(crate) fn collapse_collapse() -> AnimationConfig {
        AnimationConfig::fade_out(0.1)
    }
}

/// 管理单个 widget 的一次进场或离场动画状态。
#[derive(Debug, Clone)]
pub(crate) struct TransitionPlayer {
    config: AnimationConfig,
    /// 当前透明度进度 [0, 1]。
    pub opacity_progress: f32,
    /// 当前位移偏移量。
    pub offset: Point,
    /// 当前缩放比例。
    pub scale: f32,
    /// 是否已完成。
    pub finished: bool,
    /// 已推进时间（含 stagger 延迟等待）。
    elapsed: f64,
    opacity_anim: Animation<f32>,
    offset_anim: Option<Animation<Point>>,
    scale_anim: Option<Animation<f32>>,
}

impl TransitionPlayer {
    pub(crate) fn new(config: AnimationConfig) -> Self {
        let opacity_anim = config.opacity_animation();
        let offset_anim = config.offset_animation();
        let scale_anim = config.scale_animation();
        let opacity_progress = opacity_anim.value();
        let offset = offset_anim
            .as_ref()
            .map_or_else(|| Point::new(0.0, 0.0), Animation::value);
        let scale = scale_anim.as_ref().map_or(1.0, Animation::value);
        Self {
            opacity_progress,
            offset,
            scale,
            finished: false,
            elapsed: 0.0,
            config,
            opacity_anim,
            offset_anim,
            scale_anim,
        }
    }

    pub(crate) fn new_from_current(
        config: AnimationConfig,
        opacity: f32,
        offset: Point,
        scale: f32,
    ) -> Self {
        let mut player = Self::new(config);
        player.opacity_anim.from = opacity;
        player.opacity_progress = opacity;
        if let Some(animation) = player.offset_anim.as_mut() {
            animation.from = offset;
        }
        player.offset = offset;
        if let Some(animation) = player.scale_anim.as_mut() {
            animation.from = scale;
        }
        player.scale = scale;
        player
    }

    pub(crate) fn hold_at_start(&mut self) {
        self.opacity_progress = self.opacity_anim.from;
        self.offset = self
            .offset_anim
            .as_ref()
            .map_or_else(|| Point::new(0.0, 0.0), |animation| animation.from);
        self.scale = self
            .scale_anim
            .as_ref()
            .map_or(1.0, |animation| animation.from);
    }

    /// 重置动画以重新播放。
    /// 原公开面遗留（SMC-04 模块收口后无内部消费方；保留供外部集成）。
    #[allow(dead_code)]
    pub(crate) fn reset(&mut self) {
        self.opacity_anim = self.config.opacity_animation();
        self.offset_anim = self.config.offset_animation();
        self.scale_anim = self.config.scale_animation();
        self.opacity_progress = self.opacity_anim.value();
        self.offset = self
            .offset_anim
            .as_ref()
            .map_or_else(|| Point::new(0.0, 0.0), Animation::value);
        self.scale = self.scale_anim.as_ref().map_or(1.0, Animation::value);
        self.finished = false;
    }

    // 只有 feedback 浮层布局需要观察动画端点几何。
    #[cfg(feature = "feedback")]
    pub(crate) fn offset_endpoints(&self) -> (Point, Point) {
        self.offset_anim
            .as_ref()
            .map_or((Point::new(0.0, 0.0), Point::new(0.0, 0.0)), |animation| {
                (animation.from, animation.to)
            })
    }

    /// 按帧推进动画。
    pub(crate) fn update(&mut self, dt: f64) {
        if self.finished {
            return;
        }
        let mut dt = dt.max(0.0);
        let delay = self.config.delay();
        if delay > 0.0 && self.elapsed < delay {
            let remaining = delay - self.elapsed;
            if dt <= remaining {
                self.elapsed += dt;
                return;
            }
            self.elapsed = delay;
            dt -= remaining;
        }
        self.opacity_progress = self.opacity_anim.update(dt);
        if let Some(ref mut animation) = self.offset_anim {
            self.offset = animation.update(dt);
        }
        if let Some(ref mut animation) = self.scale_anim {
            self.scale = animation.update(dt);
        }
        let opacity_done = self.opacity_anim.is_finished();
        let offset_done = self.offset_anim.as_ref().is_none_or(Animation::is_finished);
        let scale_done = self.scale_anim.as_ref().is_none_or(Animation::is_finished);
        if opacity_done && offset_done && scale_done {
            self.finished = true;
            self.opacity_progress = if self.config.is_enter() { 1.0 } else { 0.0 };
            self.offset = Point::new(0.0, 0.0);
            self.scale = 1.0;
        }
    }
}

/// 声明式入场/离场动画的紧凑类型：文档「目标写法」的 `Transition::fade_in` /
/// `stagger` / `slide_up` 等直接绑定到 [`crate::ui::view::View`] 的 `enter` / `leave`。
///
/// `Transition` 是 [`AnimationConfig`] 的薄封装，`Into<AnimationConfig>` 后由
/// 既有播放管线消费，不改变任何现有 API。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    config: AnimationConfig,
}

impl Transition {
    /// 渐入。
    pub fn fade_in(duration: f64) -> Self {
        Self {
            config: AnimationConfig::fade_in(duration),
        }
    }

    /// 渐出。
    pub fn fade_out(duration: f64) -> Self {
        Self {
            config: AnimationConfig::fade_out(duration),
        }
    }

    /// 从下方滑入（`slide_up` 语义：内容向上进入视口）。
    pub fn slide_up(duration: f64) -> Self {
        Self {
            config: AnimationConfig::slide_in(Placement::Bottom, duration),
        }
    }

    /// 从上方滑入（`slide_down` 语义：内容向下进入视口）。
    pub fn slide_down(duration: f64) -> Self {
        Self {
            config: AnimationConfig::slide_in(Placement::Top, duration),
        }
    }

    /// 从 `placement` 所在方向滑入。
    pub fn slide_in(placement: Placement, duration: f64) -> Self {
        Self {
            config: AnimationConfig::slide_in(placement, duration),
        }
    }

    /// 向 `placement` 所在方向滑出。
    pub fn slide_out(placement: Placement, duration: f64) -> Self {
        Self {
            config: AnimationConfig::slide_out(placement, duration),
        }
    }

    /// 缩放渐入。
    pub fn zoom_in(duration: f64) -> Self {
        Self {
            config: AnimationConfig::zoom_in(duration),
        }
    }

    /// 缩放渐出。
    pub fn zoom_out(duration: f64) -> Self {
        Self {
            config: AnimationConfig::zoom_out(duration),
        }
    }

    /// 交错入场：`delay` 秒后开始播放 `inner`（stagger 语义）。
    pub fn stagger(delay: f64, inner: Transition) -> Self {
        Self {
            config: inner.config.with_delay(delay),
        }
    }
}

impl From<Transition> for AnimationConfig {
    fn from(transition: Transition) -> Self {
        transition.config
    }
}
