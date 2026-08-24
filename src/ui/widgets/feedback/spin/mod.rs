//! 提供动画加载指示器。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::draw::painting::PaintPass;
use crate::ui::SnapshotFields;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::widget;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
/// 加载指示器的预设尺寸。
pub enum SpinSize {
    /// 小尺寸加载指示器。
    Small,
    /// 默认尺寸加载指示器。
    Default,
    /// 大尺寸加载指示器。
    Large,
}

widget! {
    /// 显示可延迟启动的动画加载指示器。
    pub struct Spin {
        size: SpinSize,
        color: Option<Color>,
        spinning: bool,
        tip: String,
        wrapper_mode: bool,
        delay: Duration,
        delay_elapsed: f32,
        phase: f32,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::WidgetId, Rect)>
    {
        if !self.wrapper_mode {
            return Vec::new();
        }
        let frame = Self::normalize_frame(frame);
        children.iter().map(|child| (child.id, frame)).collect()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        self.wrapper_mode.then(|| Self::normalize_frame(frame))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let expected_pass = if self.wrapper_mode {
            PaintPass::AfterChildren
        } else {
            PaintPass::Content
        };
        if ctx.paint_pass() != expected_pass || !self.spinning || !self.delay_ready() {
            return;
        }

        let frame = Self::normalize_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let c = self.color.unwrap_or(ctx.tokens().color_primary());
        let (cx, cy, d, tip_frame) = self.content_geometry(frame);

        ctx.push_clip(frame);
        if self.wrapper_mode {
            ctx.fill_rect(frame, ctx.tokens().color_text().with_alpha(30), None);
        }

        if d > 0.0 {
            self.render_dots(ctx, cx, cy, d * 0.35, c);
        }

        if let Some(tip_frame) = tip_frame {
            self.render_tip(ctx, tip_frame);
        }
        ctx.pop_clip();
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.spinning {
            return false;
        }

        let dt = if dt.is_finite() && dt > 0.0 {
            dt.min(f32::MAX as f64) as f32
        } else {
            0.0
        };

        let delay = self.delay.as_secs_f32();
        if delay > 0.0 && self.delay_elapsed < delay {
            self.delay_elapsed = (self.delay_elapsed + dt).min(delay);
            return true;
        }

        self.phase = (self.phase + dt * Self::SPIN_ANGULAR_SPEED)
            .rem_euclid(std::f32::consts::TAU);
        true
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.spinning && self.delay_ready() {
            let frame = Self::normalize_frame(frame);
            self.spinner_bounds(frame)
                .intersect(&frame)
                .unwrap_or_default()
        } else {
            Rect::zero()
        }
    }
}

impl Spin {
    const SPIN_ANGULAR_SPEED: f32 = std::f32::consts::TAU;

    fn diameter(&self) -> f32 {
        match self.size {
            SpinSize::Small => 16.0,
            SpinSize::Default => 24.0,
            SpinSize::Large => 36.0,
        }
    }

    fn render_dots(&self, ctx: &mut PaintContext, cx: f32, cy: f32, r: f32, c: Color) {
        let dot_r = r * 0.18;
        for i in 0..8 {
            let angle = i as f32 * std::f32::consts::TAU / 8.0 + self.phase;
            let dx = angle.cos() * r;
            let dy = angle.sin() * r;
            let opacity = 0.25 + (i as f32 / 8.0) * 0.75;
            // 点渐隐：对主题色做预乘淡化（保持原算法，避免直接 alpha 在亮背景上过亮）。
            let dot_color = Color::from_rgba(
                (c.r as f32 * opacity) as u8,
                (c.g as f32 * opacity) as u8,
                (c.b as f32 * opacity) as u8,
                (c.a as f32 * opacity) as u8,
            );
            ctx.fill_circle(cx + dx, cy + dy, dot_r, dot_color);
        }
    }

    fn spinner_bounds(&self, frame: Rect) -> Rect {
        let (cx, cy, d, _) = self.content_geometry(frame);
        let orbit_r = d * 0.35;
        let dot_r = orbit_r * 0.18;
        let extent = orbit_r + dot_r + 1.0;
        Rect::new(cx - extent, cy - extent, extent * 2.0, extent * 2.0)
    }

    fn content_geometry(&self, frame: Rect) -> (f32, f32, f32, Option<Rect>) {
        const TIP_GAP: f32 = 8.0;
        const TIP_HEIGHT: f32 = 18.0;
        let has_tip = self.wrapper_mode && !self.tip.is_empty() && frame.h >= TIP_HEIGHT;
        let reserved_tip_height = if has_tip { TIP_GAP + TIP_HEIGHT } else { 0.0 };
        let d = self
            .diameter()
            .min(frame.w)
            .min((frame.h - reserved_tip_height).max(0.0));
        let content_height = d + reserved_tip_height;
        let top = frame.y + (frame.h - content_height) * 0.5;
        let cx = frame.x + frame.w * 0.5;
        let cy = top + d * 0.5;
        let tip_frame = has_tip.then(|| {
            Rect::new(
                frame.x,
                top + d + TIP_GAP,
                frame.w,
                TIP_HEIGHT.min(frame.y + frame.h - (top + d + TIP_GAP)),
            )
        });
        (cx, cy, d, tip_frame)
    }

    fn render_tip(&self, ctx: &mut PaintContext, frame: Rect) {
        const FONT_SIZE: f32 = 13.0;
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(text) = ctx.elide_single_line(&self.tip, FONT_SIZE, frame.w) else {
            return;
        };
        // 使用同一保守宽度契约居中截断后的提示文本。
        let text_width = ctx.conservative_text_width(&text, FONT_SIZE);
        let y = ctx.visual_center_y(frame, FONT_SIZE);
        ctx.draw_text(
            &text,
            Point::new(frame.x + (frame.w - text_width) * 0.5, y),
            ctx.tokens().color_text_secondary(),
            FONT_SIZE,
        );
    }

    fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            Self::normalize_dimension(frame.w),
            Self::normalize_dimension(frame.h),
        )
    }

    fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }
}

impl Default for Spin {
    fn default() -> Self {
        Self::new()
    }
}

// 把加载指示器 Rust 内核与已有拥有型子树融合为 UIX 声明的单一根节点。
fn build_spin_view(kernel: Spin, children: Vec<ViewNode>) -> ViewNode {
    ViewNode::new(kernel, children)
}

impl View for Spin {
    fn build(self) -> ViewNode {
        // 独立指示器同样经由组件自己的 UIX 根声明构建。
        self.build_view_with_children(Vec::new())
    }
}

impl Spin {
    /// 创建默认尺寸且立即开始旋转的加载指示器。
    pub fn new() -> Self {
        Self {
            size: SpinSize::Default,
            color: None,
            spinning: true,
            tip: String::new(),
            wrapper_mode: false,
            delay: Duration::ZERO,
            delay_elapsed: 0.0,
            phase: 0.0,
        }
    }

    /// 将加载指示器设为小尺寸。
    pub fn small(mut self) -> Self {
        self.size = SpinSize::Small;
        self
    }

    /// 将加载指示器设为大尺寸。
    pub fn large(mut self) -> Self {
        self.size = SpinSize::Large;
        self
    }

    /// 设置加载指示器的预设尺寸。
    pub fn size(mut self, size: SpinSize) -> Self {
        self.size = size;
        self
    }

    /// 设置开始显示加载动画前的延迟时长，并重置延迟计时。
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self.delay_elapsed = 0.0;
        self
    }

    /// 设置加载指示器颜色。
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    /// 设置是否显示并推进加载动画。
    pub fn spinning(mut self, v: bool) -> Self {
        self.spinning = v;
        self
    }

    /// 设置加载指示器下方的提示文本。
    pub fn tip(mut self, t: impl Into<String>) -> Self {
        self.tip = t.into();
        self
    }

    /// 启用包裹模式，在子组件上方绘制加载遮罩。
    pub fn wrapper_mode(mut self) -> Self {
        self.wrapper_mode = true;
        self
    }

    /// 经由同目录 UIX 根声明构建加载指示器及其遮罩子树。
    #[doc(hidden)]
    pub fn build_view_with_children(self, children: Vec<ViewNode>) -> ViewNode {
        // UIX 只拥有公开根；Rust 内核继续独占动画、布局、几何与绘制机制。
        let kernel = self;
        crate::uix!("src/ui/widgets/feedback/spin/spin.uix")
    }

    /// 返回当前动画相位。
    pub fn phase(&self) -> f32 {
        self.phase
    }

    fn intrinsic_size(&self) -> Size {
        if self.wrapper_mode {
            Size::new(0.0, 0.0)
        } else {
            let d = self.diameter();
            Size::new(d, d)
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Spin {
            size: self.size,
            color: self.color,
            spinning: self.spinning,
            tip: self.tip.clone(),
            wrapper_mode: self.wrapper_mode,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let restarting = !self.spinning && next.spinning;
        self.size = next.size;
        self.color = next.color;
        self.spinning = next.spinning;
        self.tip = next.tip;
        self.wrapper_mode = next.wrapper_mode;
        let delay_changed = self.delay != next.delay;
        self.delay = next.delay;
        if delay_changed || restarting || !self.spinning {
            self.delay_elapsed = 0.0;
        }
    }

    fn delay_ready(&self) -> bool {
        self.delay.is_zero() || self.delay_elapsed >= self.delay.as_secs_f32()
    }
}

// 验证声明刷新不会夺走 Spin 的运行时动画生命周期状态。
#[cfg(test)]
// 将生命周期契约限制在当前组件模块的内部测试中。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/feedback/spin__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
