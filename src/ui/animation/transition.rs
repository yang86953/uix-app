//! Transition — widget 进出场过渡动画。
//!
//! 提供统一过渡动画定义，用于 Modal（弹出/关闭）、Drawer（滑入/滑出）、
//! Tooltip（渐隐渐现）、Collapse（展开/折叠）等场景。

use crate::ui::animation::core::Animation;
use crate::native::Point;

/// 过渡动画方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideDirection {
    Up,
    Down,
    Left,
    Right,
}

/// 过渡动画类型。
#[derive(Debug, Clone, Copy)]
pub enum Transition {
    /// 渐入（透明度 0→1）。
    FadeIn { duration: f64 },
    /// 渐出（透明度 1→0）。
    FadeOut { duration: f64 },
    /// 方向性滑入（透明度 + 位移）。
    SlideIn {
        direction: SlideDirection,
        distance: f32,
        duration: f64,
    },
    /// 方向性滑出（透明度 + 位移）。
    SlideOut {
        direction: SlideDirection,
        distance: f32,
        duration: f64,
    },
    /// 缩放弹出（透明度 + 缩放 0.8→1.0）。
    ZoomIn { duration: f64 },
    /// 缩放收起（透明度 + 缩放 1.0→0.8）。
    ZoomOut { duration: f64 },
}

impl Transition {
    /// 是否为进场动画。
    pub fn is_enter(&self) -> bool {
        matches!(
            self,
            Self::FadeIn { .. } | Self::SlideIn { .. } | Self::ZoomIn { .. }
        )
    }

    /// 是否为退场动画。
    pub fn is_exit(&self) -> bool {
        matches!(
            self,
            Self::FadeOut { .. } | Self::SlideOut { .. } | Self::ZoomOut { .. }
        )
    }

    /// 获取动画持续时间。
    pub fn duration(&self) -> f64 {
        match self {
            Self::FadeIn { duration }
            | Self::FadeOut { duration }
            | Self::SlideIn { duration, .. }
            | Self::SlideOut { duration, .. }
            | Self::ZoomIn { duration }
            | Self::ZoomOut { duration } => *duration,
        }
    }

    /// 创建对应的透明度动画（f32）。
    pub fn opacity_animation(&self) -> Animation<f32> {
        match self {
            Self::FadeIn { .. } | Self::SlideIn { .. } | Self::ZoomIn { .. } => {
                Animation::<f32>::fade_in(self.duration())
            }
            Self::FadeOut { .. } | Self::SlideOut { .. } | Self::ZoomOut { .. } => {
                Animation::<f32>::fade_out(self.duration())
            }
        }
    }

    /// 创建位移动画（用于 SlideIn/SlideOut）。
    pub fn offset_animation(&self) -> Option<Animation<Point>> {
        match self {
            Self::SlideIn {
                direction,
                distance,
                duration,
            } => {
                let offset = match direction {
                    SlideDirection::Up => Point::new(0.0, *distance),
                    SlideDirection::Down => Point::new(0.0, -*distance),
                    SlideDirection::Left => Point::new(*distance, 0.0),
                    SlideDirection::Right => Point::new(-*distance, 0.0),
                };
                Some(
                    Animation::new(offset, Point::new(0.0, 0.0), *duration)
                        .with_easing(crate::ui::animation::easing::Easing::antd_out()),
                )
            }
            Self::SlideOut {
                direction,
                distance,
                duration,
            } => {
                let offset = match direction {
                    SlideDirection::Up => Point::new(0.0, -*distance),
                    SlideDirection::Down => Point::new(0.0, *distance),
                    SlideDirection::Left => Point::new(-*distance, 0.0),
                    SlideDirection::Right => Point::new(*distance, 0.0),
                };
                Some(
                    Animation::new(Point::new(0.0, 0.0), offset, *duration)
                        .with_easing(crate::ui::animation::easing::Easing::antd_in()),
                )
            }
            _ => None,
        }
    }

    /// 创建缩放动画（用于 ZoomIn/ZoomOut）。
    pub fn scale_animation(&self) -> Option<Animation<f32>> {
        match self {
            Self::ZoomIn { duration } => Some(
                Animation::new(0.8, 1.0, *duration)
                    .with_easing(crate::ui::animation::easing::Easing::antd_default()),
            ),
            Self::ZoomOut { duration } => Some(
                Animation::new(1.0, 0.8, *duration)
                    .with_easing(crate::ui::animation::easing::Easing::antd_default()),
            ),
            _ => None,
        }
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self::FadeIn { duration: 0.2 }
    }
}

/// 常用过渡预设。
pub mod presets {
    use super::*;

    /// Modal 弹出过渡：zoom + fade。
    pub fn modal_enter() -> Transition {
        Transition::ZoomIn { duration: 0.2 }
    }

    /// Modal 关闭过渡：zoom + fade。
    pub fn modal_exit() -> Transition {
        Transition::ZoomOut { duration: 0.2 }
    }

    /// Drawer 滑入。
    pub fn drawer_enter(direction: SlideDirection) -> Transition {
        Transition::SlideIn {
            direction,
            distance: 180.0,
            duration: 0.25,
        }
    }

    /// Drawer 滑出。
    pub fn drawer_exit(direction: SlideDirection) -> Transition {
        Transition::SlideOut {
            direction,
            distance: 180.0,
            duration: 0.2,
        }
    }

    /// Tooltip 渐隐渐现。
    pub fn tooltip_enter() -> Transition {
        Transition::FadeIn { duration: 0.15 }
    }

    /// Tooltip 渐出。
    pub fn tooltip_exit() -> Transition {
        Transition::FadeOut { duration: 0.1 }
    }

    /// Collapse 展开。
    pub fn collapse_expand() -> Transition {
        Transition::FadeIn { duration: 0.15 }
    }

    /// Collapse 折叠。
    pub fn collapse_collapse() -> Transition {
        Transition::FadeOut { duration: 0.1 }
    }
}

/// TransitionPlayer — 管理单个 widget 的进出场动画状态。
#[derive(Debug, Clone)]
pub struct TransitionPlayer {
    /// 当前过渡动画类型。
    transition: Transition,
    /// 透明度动画进度 [0, 1]。
    pub opacity_progress: f32,
    /// 位移偏移量。
    pub offset: Point,
    /// 缩放比例。
    pub scale: f32,
    /// 是否已完成。
    pub finished: bool,
    /// 内部透明度动画。
    opacity_anim: Animation<f32>,
    /// 内部位移动画。
    offset_anim: Option<Animation<Point>>,
    /// 内部缩放动画。
    scale_anim: Option<Animation<f32>>,
}

impl TransitionPlayer {
    /// 创建一个新的过渡播放器。
    pub fn new(transition: Transition) -> Self {
        let opacity_anim = transition.opacity_animation();
        let offset_anim = transition.offset_animation();
        let scale_anim = transition.scale_animation();
        Self {
            opacity_progress: if transition.is_enter() { 0.0 } else { 1.0 },
            offset: Point::new(0.0, 0.0),
            scale: if transition.is_enter() { 0.8 } else { 1.0 },
            finished: false,
            transition,
            opacity_anim,
            offset_anim,
            scale_anim,
        }
    }

    /// 重置动画（用于重新播放）。
    pub fn reset(&mut self) {
        self.opacity_anim = self.transition.opacity_animation();
        self.offset_anim = self.transition.offset_animation();
        self.scale_anim = self.transition.scale_animation();
        self.opacity_progress = if self.transition.is_enter() { 0.0 } else { 1.0 };
        self.offset = Point::new(0.0, 0.0);
        self.scale = if self.transition.is_enter() { 0.8 } else { 1.0 };
        self.finished = false;
    }

    /// 按帧推进动画。
    pub fn update(&mut self, dt: f64) {
        if self.finished {
            return;
        }
        self.opacity_progress = self.opacity_anim.update(dt);
        if let Some(ref mut anim) = self.offset_anim {
            self.offset = anim.update(dt);
        }
        if let Some(ref mut anim) = self.scale_anim {
            self.scale = anim.update(dt);
        }
        let opacity_done = self.opacity_anim.is_finished();
        let offset_done = self.offset_anim.as_ref().map_or(true, |a| a.is_finished());
        let scale_done = self.scale_anim.as_ref().map_or(true, |a| a.is_finished());
        if opacity_done && offset_done && scale_done {
            self.finished = true;
            self.opacity_progress = if self.transition.is_enter() { 1.0 } else { 0.0 };
            self.offset = Point::new(0.0, 0.0);
            self.scale = 1.0;
        }
    }
}
