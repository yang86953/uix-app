//! widget 进出场动画配置与播放器。
//!
//! [`AnimationConfig`] 是应用侧公开配置；[`TransitionPlayer`] 只负责把配置
//! 展开为单次 opacity / offset / scale 插值。

use crate::core::Point;
use crate::ui::animation::core::Animation;
use crate::ui::Placement;

/// 浮层进入或离场时使用的动画配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationConfig {
    kind: AnimationKind,
    duration: f64,
    distance: f32,
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
        }
    }

    /// 创建渐出配置。
    pub fn fade_out(duration: f64) -> Self {
        Self {
            kind: AnimationKind::FadeOut,
            duration: duration.max(0.0),
            distance: 0.0,
        }
    }

    /// 创建从 `placement` 所在方向滑入的配置。
    pub fn slide_in(placement: Placement, duration: f64) -> Self {
        Self {
            kind: AnimationKind::SlideIn(placement),
            distance: Self::DEFAULT_SLIDE_DISTANCE,
            duration: duration.max(0.0),
        }
    }

    /// 创建向 `placement` 所在方向滑出的配置。
    pub fn slide_out(placement: Placement, duration: f64) -> Self {
        Self {
            kind: AnimationKind::SlideOut(placement),
            distance: Self::DEFAULT_SLIDE_DISTANCE,
            duration: duration.max(0.0),
        }
    }

    /// 创建缩放渐入配置。
    pub fn zoom_in(duration: f64) -> Self {
        Self {
            kind: AnimationKind::ZoomIn,
            duration: duration.max(0.0),
            distance: 0.0,
        }
    }

    /// 创建缩放渐出配置。
    pub fn zoom_out(duration: f64) -> Self {
        Self {
            kind: AnimationKind::ZoomOut,
            duration: duration.max(0.0),
            distance: 0.0,
        }
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
pub mod presets {
    use super::*;

    pub fn modal_enter() -> AnimationConfig {
        AnimationConfig::zoom_in(0.2)
    }

    pub fn modal_exit() -> AnimationConfig {
        AnimationConfig::zoom_out(0.2)
    }

    pub fn drawer_enter(placement: Placement) -> AnimationConfig {
        AnimationConfig::slide_in(placement, 0.25).with_distance(180.0)
    }

    pub fn drawer_exit(placement: Placement) -> AnimationConfig {
        AnimationConfig::slide_out(placement, 0.2).with_distance(180.0)
    }

    pub fn tooltip_enter() -> AnimationConfig {
        AnimationConfig::fade_in(0.15)
    }

    pub fn tooltip_exit() -> AnimationConfig {
        AnimationConfig::fade_out(0.1)
    }

    pub fn collapse_expand() -> AnimationConfig {
        AnimationConfig::fade_in(0.15)
    }

    pub fn collapse_collapse() -> AnimationConfig {
        AnimationConfig::fade_out(0.1)
    }
}

/// 管理单个 widget 的一次进场或离场动画状态。
#[derive(Debug, Clone)]
pub struct TransitionPlayer {
    config: AnimationConfig,
    /// 当前透明度进度 [0, 1]。
    pub opacity_progress: f32,
    /// 当前位移偏移量。
    pub offset: Point,
    /// 当前缩放比例。
    pub scale: f32,
    /// 是否已完成。
    pub finished: bool,
    opacity_anim: Animation<f32>,
    offset_anim: Option<Animation<Point>>,
    scale_anim: Option<Animation<f32>>,
}

impl TransitionPlayer {
    pub fn new(config: AnimationConfig) -> Self {
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
            config,
            opacity_anim,
            offset_anim,
            scale_anim,
        }
    }

    /// 重置动画以重新播放。
    pub fn reset(&mut self) {
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

    pub(crate) fn offset_endpoints(&self) -> (Point, Point) {
        self.offset_anim
            .as_ref()
            .map_or((Point::new(0.0, 0.0), Point::new(0.0, 0.0)), |animation| {
                (animation.from, animation.to)
            })
    }

    /// 按帧推进动画。
    pub fn update(&mut self, dt: f64) {
        if self.finished {
            return;
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
