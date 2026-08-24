//! 骨架屏加载占位组件。

use crate::core::{Constraints, Rect, Size};
use crate::ui::SnapshotFields;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::widget;
use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq)]
/// 骨架占位内容的形状。
pub enum SkeletonShape {
    /// 圆角矩形占位。
    Rect,
    /// 圆形头像占位。
    Circle,
    /// 多行文本占位。
    Text,
}

widget! {
    /// 可选用流光动画的加载占位组件。
    pub struct Skeleton {
        shape: SkeletonShape,
        w: f32,
        h: f32,
        avatar_size: Option<Size>,
        paragraph_lines: usize,
        active: bool,
        phase: Cell<f32>,
        animation_dirty: Cell<bool>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let color = ctx.tokens().color_fill_tertiary();
        ctx.push_clip(frame);
        self.paint_placeholder(frame, color, ctx);
        if self.active {
            let phase = self.phase.get();
            let band_width = (frame.w * 0.35).max(8.0).min(frame.w);
            let shimmer_x = frame.x + (frame.w + band_width) * phase - band_width;
            ctx.push_clip(Rect::new(shimmer_x, frame.y, band_width, frame.h));
            self.paint_placeholder(
                frame,
                ctx.tokens().color_bg_container().with_alpha(96),
                ctx,
            );
            ctx.pop_clip();
        }
        ctx.pop_clip();
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.active { return false; }
        let delta = if dt.is_finite() {
            (dt.max(0.0) * 0.8).rem_euclid(1.0) as f32
        } else {
            0.0
        };
        let previous = self.phase.get();
        let next = (previous + delta).rem_euclid(1.0);
        if next != previous {
            self.phase.set(next);
            self.animation_dirty.set(true);
        }
        true
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.animation_dirty.replace(false) { frame } else { Rect::zero() }
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

// 把 Rust 动画与绘制内核融合为 UIX 声明的单一叶节点。
fn build_skeleton_view(kernel: Skeleton) -> ViewNode {
    ViewNode::leaf(kernel)
}

impl View for Skeleton {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根，Rust 继续拥有动画、几何与绘制机制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/skeleton/skeleton.uix")
    }
}

impl Skeleton {
    const MAX_PARAGRAPH_LINES: usize = 64;

    /// 创建默认宽 200、高 16 且未启用动画的矩形占位。
    pub fn new() -> Self {
        Self {
            shape: SkeletonShape::Rect,
            w: 200.0,
            h: 16.0,
            avatar_size: None,
            paragraph_lines: 0,
            active: false,
            phase: Cell::new(0.0),
            animation_dirty: Cell::new(false),
        }
    }

    /// 设置骨架占位形状。
    pub fn shape(mut self, s: SkeletonShape) -> Self {
        self.shape = s;
        self
    }

    /// 设置占位宽高；非有限值归零，负值截断为零。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.w = Self::normalized_dimension(w);
        self.h = Self::normalized_dimension(h);
        self
    }

    /// 设置占位宽度；非有限值归零，负值截断为零。
    pub fn width(mut self, width: f32) -> Self {
        self.w = Self::normalized_dimension(width);
        self
    }

    /// 设置占位高度；非有限值归零，负值截断为零。
    pub fn height(mut self, height: f32) -> Self {
        self.h = Self::normalized_dimension(height);
        self
    }

    /// 头像占位。`Size::Small/Default/Large` 分别为 32/40/56 logical。
    pub fn avatar(mut self, size: Size) -> Self {
        let side = Self::normalized_dimension(size.w.max(size.h));
        self.shape = SkeletonShape::Circle;
        self.avatar_size = Some(Size::new(side, side));
        self.w = side;
        self.h = side;
        self
    }

    /// 设置文本段落占位行数，并限制在一至六十四行。
    pub fn paragraph(mut self, lines: usize) -> Self {
        self.shape = SkeletonShape::Text;
        self.paragraph_lines = lines.clamp(1, Self::MAX_PARAGRAPH_LINES);
        self.h = (self.paragraph_lines as f32 * 16.0).max(16.0);
        self
    }

    /// 设置是否播放流光动画。
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.w, self.h)
    }

    fn normalized_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            Self::normalized_dimension(frame.w),
            Self::normalized_dimension(frame.h),
        )
    }

    fn avatar_rect(&self, frame: Rect) -> Rect {
        let requested = self
            .avatar_size
            .map(|size| Self::normalized_dimension(size.w.max(size.h)))
            .unwrap_or_else(|| frame.w.min(frame.h));
        let side = requested.min(frame.w).min(frame.h);
        Rect::new(
            frame.x + (frame.w - side) * 0.5,
            frame.y + (frame.h - side) * 0.5,
            side,
            side,
        )
    }

    fn for_each_paragraph_rect(&self, frame: Rect, mut visit: impl FnMut(Rect)) {
        let rows = if self.paragraph_lines == 0 {
            2
        } else {
            self.paragraph_lines
        };
        let desired_line_height = 12.0;
        let desired_gap = 4.0;
        let desired_height =
            desired_line_height * rows as f32 + desired_gap * rows.saturating_sub(1) as f32;
        let scale = if desired_height > 0.0 {
            (frame.h / desired_height).min(1.0)
        } else {
            0.0
        };
        let line_height = desired_line_height * scale;
        let gap = desired_gap * scale;
        let content_height = line_height * rows as f32 + gap * rows.saturating_sub(1) as f32;
        let start_y = frame.y + (frame.h - content_height) * 0.5;

        // 生产渲染逐行消费几何，不为每帧创建临时矩形列表。
        for row in 0..rows {
            let width_factor = if row + 1 == rows { 0.6 } else { 1.0 };
            visit(Rect::new(
                frame.x,
                start_y + row as f32 * (line_height + gap),
                frame.w * width_factor,
                line_height,
            ));
        }
    }

    fn paint_placeholder(&self, frame: Rect, color: crate::draw::Color, ctx: &mut PaintContext) {
        match self.shape {
            SkeletonShape::Rect => {
                let radius = 4.0_f32.min(frame.w.min(frame.h) * 0.5);
                ctx.fill_rect(frame, color, Some(crate::draw::Radius::uniform(radius)));
            }
            SkeletonShape::Circle => {
                let avatar = self.avatar_rect(frame);
                if avatar.w > 0.0 {
                    ctx.fill_circle(
                        avatar.x + avatar.w * 0.5,
                        avatar.y + avatar.h * 0.5,
                        avatar.w * 0.5,
                        color,
                    );
                }
            }
            SkeletonShape::Text => {
                self.for_each_paragraph_rect(frame, |line| {
                    ctx.fill_rect(
                        line,
                        color,
                        Some(crate::draw::Radius::uniform(2.0_f32.min(line.h * 0.5))),
                    );
                });
            }
        }
    }

    // 测试目标保留骨架头像区域观测入口，供占位布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn avatar_rect_for_test(&self, frame: Rect) -> Rect {
        self.avatar_rect(Self::normalized_frame(frame))
    }

    // 测试目标保留骨架段落区域观测入口，供占位布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn paragraph_rects_for_test(&self, frame: Rect) -> Vec<Rect> {
        let mut rects = Vec::with_capacity(self.paragraph_lines.max(2));
        self.for_each_paragraph_rect(Self::normalized_frame(frame), |rect| rects.push(rect));
        rects
    }

    // 测试目标保留骨架动画 phase 观测入口，供占位动画测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn phase_for_test(&self) -> f32 {
        self.phase.get()
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Skeleton {
            shape: self.shape,
            width: self.w,
            height: self.h,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let active_changed = self.active != next.active;
        self.shape = next.shape;
        self.w = Self::normalized_dimension(next.w);
        self.h = Self::normalized_dimension(next.h);
        self.avatar_size = next.avatar_size;
        self.paragraph_lines = next.paragraph_lines;
        self.active = next.active;
        if active_changed {
            self.phase.set(0.0);
            self.animation_dirty.set(true);
        }
    }
}

// 集中验证 UIX 声明壳与 Rust 内核的单节点契约。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/display/skeleton__tests.rs"]
mod tests;
