//! 骨架屏加载占位组件。

use crate::core::{Constraints, Rect, Size};
use crate::ui::SnapshotFields;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::ColorValue;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::widget;
use std::cell::Cell;

// 保存由 UIX 声明、由 Rust 动画与几何内核消费的静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SkeletonVisual {
    default_width: f32,
    default_height: f32,
    default_paragraph_rows: usize,
    paragraph_line_height: f32,
    paragraph_gap: f32,
    paragraph_last_width: f32,
    rect_radius: f32,
    text_radius: f32,
    shimmer_width_ratio: f32,
    shimmer_min_width: f32,
    shimmer_speed: f64,
    shimmer_alpha: u8,
    base_color: ColorValue,
    shimmer_color: ColorValue,
}

// 同目录 UIX 生成唯一视觉值及静态借用。
crate::uix_items!("src/ui/widgets/display/skeleton/skeleton.uix");

impl SkeletonVisual {
    fn paragraph_extent(self) -> f32 {
        self.paragraph_line_height + self.paragraph_gap
    }
}

// 记录高度来自作者、默认视觉或段落视觉，供 UIX 融合时保留覆盖优先级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkeletonHeightSource {
    Default,
    Authored,
    Paragraph,
}

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
        #[snapshot(skip)]
        width_authored: bool,
        #[snapshot(skip)]
        height_source: SkeletonHeightSource,
        avatar_size: Option<Size>,
        paragraph_lines: usize,
        active: bool,
        phase: Cell<f32>,
        animation_dirty: Cell<bool>,
        #[snapshot(skip)]
        visual: &'static SkeletonVisual,
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
        let color = self.visual.base_color.resolve(ctx.tokens());
        ctx.push_clip(frame);
        self.paint_placeholder(frame, color, ctx);
        if self.active {
            let phase = self.phase.get();
            let band_width = (frame.w * self.visual.shimmer_width_ratio.max(0.0))
                .max(self.visual.shimmer_min_width.max(0.0))
                .min(frame.w);
            let shimmer_x = frame.x + (frame.w + band_width) * phase - band_width;
            ctx.push_clip(Rect::new(shimmer_x, frame.y, band_width, frame.h));
            self.paint_placeholder(
                frame,
                self.visual
                    .shimmer_color
                    .resolve(ctx.tokens())
                    .with_alpha(self.visual.shimmer_alpha),
                ctx,
            );
            ctx.pop_clip();
        }
        ctx.pop_clip();
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.active { return false; }
        let delta = if dt.is_finite() {
            (dt.max(0.0) * self.visual.shimmer_speed.max(0.0)).rem_euclid(1.0) as f32
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

// 向 UIX 提供骨架基础填充色角色。
const fn skeleton_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}

// 向 UIX 提供流光叠加层背景色角色。
const fn skeleton_bg_container() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}

// 把 UIX 声明的静态视觉融合进 Rust 动画与几何内核。
fn build_skeleton_view(mut kernel: Skeleton, visual: &'static SkeletonVisual) -> ViewNode {
    if !kernel.width_authored {
        kernel.w = visual.default_width;
    }
    kernel.h = match kernel.height_source {
        SkeletonHeightSource::Default => visual.default_height,
        SkeletonHeightSource::Authored => kernel.h,
        SkeletonHeightSource::Paragraph => {
            (kernel.paragraph_lines as f32 * visual.paragraph_extent()).max(visual.default_height)
        }
    };
    kernel.visual = visual;
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
        let visual = SKELETON_VISUAL_REF;
        Self {
            shape: SkeletonShape::Rect,
            w: visual.default_width,
            h: visual.default_height,
            width_authored: false,
            height_source: SkeletonHeightSource::Default,
            avatar_size: None,
            paragraph_lines: 0,
            active: false,
            phase: Cell::new(0.0),
            animation_dirty: Cell::new(false),
            visual,
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
        self.width_authored = true;
        self.height_source = SkeletonHeightSource::Authored;
        self
    }

    /// 设置占位宽度；非有限值归零，负值截断为零。
    pub fn width(mut self, width: f32) -> Self {
        self.w = Self::normalized_dimension(width);
        self.width_authored = true;
        self
    }

    /// 设置占位高度；非有限值归零，负值截断为零。
    pub fn height(mut self, height: f32) -> Self {
        self.h = Self::normalized_dimension(height);
        self.height_source = SkeletonHeightSource::Authored;
        self
    }

    /// 头像占位。`Size::Small/Default/Large` 分别为 32/40/56 logical。
    pub fn avatar(mut self, size: Size) -> Self {
        let side = Self::normalized_dimension(size.w.max(size.h));
        self.shape = SkeletonShape::Circle;
        self.avatar_size = Some(Size::new(side, side));
        self.w = side;
        self.h = side;
        self.width_authored = true;
        self.height_source = SkeletonHeightSource::Authored;
        self
    }

    /// 设置文本段落占位行数，并限制在一至六十四行。
    pub fn paragraph(mut self, lines: usize) -> Self {
        self.shape = SkeletonShape::Text;
        self.paragraph_lines = lines.clamp(1, Self::MAX_PARAGRAPH_LINES);
        self.height_source = SkeletonHeightSource::Paragraph;
        self.h = (self.paragraph_lines as f32 * self.visual.paragraph_extent())
            .max(self.visual.default_height);
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
            self.visual.default_paragraph_rows
        } else {
            self.paragraph_lines
        };
        let desired_line_height = self.visual.paragraph_line_height.max(0.0);
        let desired_gap = self.visual.paragraph_gap.max(0.0);
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
            let width_factor = if row + 1 == rows {
                self.visual.paragraph_last_width.clamp(0.0, 1.0)
            } else {
                1.0
            };
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
                let radius = self
                    .visual
                    .rect_radius
                    .max(0.0)
                    .min(frame.w.min(frame.h) * 0.5);
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
                        Some(crate::draw::Radius::uniform(
                            self.visual.text_radius.max(0.0).min(line.h * 0.5),
                        )),
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
        self.width_authored = next.width_authored;
        self.height_source = next.height_source;
        self.avatar_size = next.avatar_size;
        self.paragraph_lines = next.paragraph_lines;
        self.active = next.active;
        self.visual = next.visual;
        if active_changed {
            self.phase.set(0.0);
            self.animation_dirty.set(true);
        }
    }
}
