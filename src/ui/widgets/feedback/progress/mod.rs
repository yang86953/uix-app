//! ProgressBar widget - deterministic and indeterminate progress indicators.

use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintPass;
use crate::draw::{Color, GradientDirection, Radius};
use crate::ui::SnapshotFields;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::widget;
use std::rc::Rc;
use std::sync::OnceLock;

/// Progress display type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressType {
    /// 使用水平条形轨道呈现进度。
    Line,
    /// 使用圆形轨道呈现进度。
    Circle,
}

/// Progress mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressMode {
    /// Fixed percentage (0.0 - 1.0).
    Determinate(f32),
    /// Indeterminate bar. Animation is driven outside the widget.
    Indeterminate,
}

/// 记录动态进度输入被安全归一化的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressNormalizationReason {
    /// 输入不是有限浮点数。
    NonFinite,
    /// 输入低于公开 fraction 下界。
    BelowRange,
    /// 输入高于公开 fraction 上界。
    AboveRange,
}

// 保存由 UIX 声明、由 Rust 几何与绘制算法消费的进度视觉常量。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ProgressLayoutVisual {
    default_width: f32,
    default_height: f32,
    default_round: bool,
    circle_radius_ratio: f32,
    circle_track_width_ratio: f32,
    circle_track_width_min: f32,
    circle_track_width_max: f32,
    circle_start_turns: f32,
    step_gap_ratio: f32,
    step_gap_max: f32,
    indeterminate_bar_width_ratio: f32,
    dashboard_center_y_ratio: f32,
    dashboard_radius_width_ratio: f32,
    dashboard_radius_height_ratio: f32,
    dashboard_track_width_ratio: f32,
    dashboard_track_width_min: f32,
    dashboard_track_width_max: f32,
    dashboard_start_turns: f32,
    dashboard_sweep_turns: f32,
    label_font_size: f32,
    line_radius_ratio: f32,
    dirty_padding: f32,
    gradient_direction: GradientDirection,
}

// 保存由 UIX 声明、由 Rust 动画状态机消费的运动参数。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ProgressMotionVisual {
    initial_phase: f32,
    phase_speed: f32,
    circle_indeterminate_sweep_turns: f32,
    dashboard_indeterminate_sweep_ratio: f32,
}

// 保存进度组件使用的主题语义色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProgressPaletteVisual {
    track: ColorValue,
    stroke: ColorValue,
    label: ColorValue,
    inner: ColorValue,
}

// 完整视觉配置由全部 ProgressBar 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ProgressVisual {
    layout: ProgressLayoutVisual,
    motion: ProgressMotionVisual,
    palette: ProgressPaletteVisual,
}

// 组合 UIX 声明的进度几何、排版与渐变方向。
#[allow(clippy::too_many_arguments)]
const fn progress_layout(
    default_width: f32,
    default_height: f32,
    default_round: bool,
    circle_radius_ratio: f32,
    circle_track_width_ratio: f32,
    circle_track_width_min: f32,
    circle_track_width_max: f32,
    circle_start_turns: f32,
    step_gap_ratio: f32,
    step_gap_max: f32,
    indeterminate_bar_width_ratio: f32,
    dashboard_center_y_ratio: f32,
    dashboard_radius_width_ratio: f32,
    dashboard_radius_height_ratio: f32,
    dashboard_track_width_ratio: f32,
    dashboard_track_width_min: f32,
    dashboard_track_width_max: f32,
    dashboard_start_turns: f32,
    dashboard_sweep_turns: f32,
    label_font_size: f32,
    line_radius_ratio: f32,
    dirty_padding: f32,
    gradient_direction: GradientDirection,
) -> ProgressLayoutVisual {
    ProgressLayoutVisual {
        default_width,
        default_height,
        default_round,
        circle_radius_ratio,
        circle_track_width_ratio,
        circle_track_width_min,
        circle_track_width_max,
        circle_start_turns,
        step_gap_ratio,
        step_gap_max,
        indeterminate_bar_width_ratio,
        dashboard_center_y_ratio,
        dashboard_radius_width_ratio,
        dashboard_radius_height_ratio,
        dashboard_track_width_ratio,
        dashboard_track_width_min,
        dashboard_track_width_max,
        dashboard_start_turns,
        dashboard_sweep_turns,
        label_font_size,
        line_radius_ratio,
        dirty_padding,
        gradient_direction,
    }
}

// 组合 UIX 声明的不确定动画参数。
const fn progress_motion(
    initial_phase: f32,
    phase_speed: f32,
    circle_indeterminate_sweep_turns: f32,
    dashboard_indeterminate_sweep_ratio: f32,
) -> ProgressMotionVisual {
    ProgressMotionVisual {
        initial_phase,
        phase_speed,
        circle_indeterminate_sweep_turns,
        dashboard_indeterminate_sweep_ratio,
    }
}

// 组合 UIX 声明的进度主题色角色。
const fn progress_palette(
    track: ColorValue,
    stroke: ColorValue,
    label: ColorValue,
    inner: ColorValue,
) -> ProgressPaletteVisual {
    ProgressPaletteVisual {
        track,
        stroke,
        label,
        inner,
    }
}

// 组合 UIX 声明的完整进度视觉配置。
const fn progress_visual(
    layout: ProgressLayoutVisual,
    motion: ProgressMotionVisual,
    palette: ProgressPaletteVisual,
) -> ProgressVisual {
    ProgressVisual {
        layout,
        motion,
        palette,
    }
}

// 向 UIX 提供线形进度默认圆角状态。
const fn progress_round_default() -> bool {
    true
}

// 向 UIX 提供进度轨道主题角色。
const fn progress_track() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}

// 向 UIX 提供进度主色主题角色。
const fn progress_stroke() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

// 向 UIX 提供进度文字主题角色。
const fn progress_label() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

// 向 UIX 提供圆形进度内层背景主题角色。
const fn progress_inner() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}

// 向 UIX 提供线形渐变方向。
const fn progress_horizontal_gradient() -> GradientDirection {
    GradientDirection::Horizontal
}

// Rust 直接构造或绕过 View 声明根时保持既有视觉；正常 View 构建会改用 UIX 静态配置。
static DEFAULT_PROGRESS_VISUAL: ProgressVisual = progress_visual(
    progress_layout(
        200.0,
        8.0,
        progress_round_default(),
        0.4,
        0.25,
        1.0,
        8.0,
        -0.25,
        0.08,
        2.0,
        0.3,
        0.72,
        0.42,
        0.65,
        0.2,
        1.0,
        8.0,
        0.5,
        0.5,
        11.0,
        0.5,
        1.0,
        progress_horizontal_gradient(),
    ),
    progress_motion(0.0, 0.75, 0.25, 0.25),
    progress_palette(
        progress_track(),
        progress_stroke(),
        progress_label(),
        progress_inner(),
    ),
);

// 正常 UIX 构建首次写入声明配置，后续 ProgressBar 实例只共享该静态对象。
static UIX_PROGRESS_VISUAL: OnceLock<ProgressVisual> = OnceLock::new();

// 为诊断日志提供稳定、无本地化依赖的原因标识。
impl ProgressNormalizationReason {
    /// 返回适合日志与自动化断言的稳定名称。
    pub const fn as_str(self) -> &'static str {
        // 按公开原因枚举返回稳定文本。
        match self {
            // 非有限输入使用明确分类。
            Self::NonFinite => "non_finite",
            // 下界越界使用明确分类。
            Self::BelowRange => "below_range",
            // 上界越界使用明确分类。
            Self::AboveRange => "above_range",
        }
    }
}

widget! {
    /// ProgressBar widget.
    pub struct ProgressBar {
        progress: f32,
        mode: ProgressMode,
        // 记录最近输入是否经过安全归一化。
        input_normalized: bool,
        // 记录最近输入的稳定归一化原因。
        normalization_reason: Option<ProgressNormalizationReason>,
        // 把日志去重门闩排除在公开快照之外。
        #[snapshot(skip)]
        normalization_reported: bool,
        stroke_color: Option<Color>,
        track_color: Option<Color>,
        height: f32,
        #[snapshot(skip)]
        height_authored: bool,
        width: f32,
        #[snapshot(skip)]
        width_authored: bool,
        round: bool,
        #[snapshot(skip)]
        round_authored: bool,
        progress_type: ProgressType,
        #[snapshot(skip)]
        progress_type_authored: bool,
        gradient_start: Option<Color>,
        gradient_end: Option<Color>,
        steps: usize,
        dashboard: bool,
        format_text: Option<Rc<dyn Fn(f32) -> String>>,
        indeterminate_phase: f32,
        previous_indeterminate_phase: f32,
        #[snapshot(skip)]
        visual: &'static ProgressVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }



    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if ctx.paint_pass() != PaintPass::Content {
            return;
        }
        let frame = Self::normalize_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let palette = &self.visual.palette;
        let layout = &self.visual.layout;
        let track_c = self
            .track_color
            .unwrap_or_else(|| palette.track.resolve(ctx.tokens()));
        let stroke_c = self
            .stroke_color
            .unwrap_or_else(|| palette.stroke.resolve(ctx.tokens()));
        let label_color = palette.label.resolve(ctx.tokens());

        ctx.push_clip(frame);
        if self.dashboard {
            self.render_dashboard(frame, track_c, stroke_c, label_color, ctx);
            ctx.pop_clip();
            return;
        }
        if self.progress_type == ProgressType::Circle {
            let cx = frame.x + frame.w * 0.5;
            let cy = frame.y + frame.h * 0.5;
            let r = frame.w.min(frame.h) * layout.circle_radius_ratio;
            if r <= 0.0 {
                ctx.pop_clip();
                return;
            }
            let track_width = (r * layout.circle_track_width_ratio)
                .clamp(
                    layout.circle_track_width_min,
                    layout.circle_track_width_max,
                )
                .min(r);

            ctx.fill_circle(cx, cy, r, track_c);
            let inner_radius = r - track_width;
            if inner_radius > 0.0 {
                ctx.fill_circle(cx, cy, inner_radius, palette.inner.resolve(ctx.tokens()));
            }

            let (start_angle, end_angle) = match self.mode {
                ProgressMode::Determinate(p) => {
                    let start_angle = std::f32::consts::TAU * layout.circle_start_turns;
                    (start_angle, start_angle + std::f32::consts::TAU * p)
                }
                ProgressMode::Indeterminate => {
                    let start_angle = self.indeterminate_phase * std::f32::consts::TAU;
                    (
                        start_angle,
                        start_angle
                            + std::f32::consts::TAU
                                * self.visual.motion.circle_indeterminate_sweep_turns,
                    )
                }
            };
            if end_angle > start_angle {
                ctx.stroke_arc(
                    cx,
                    cy,
                    r - track_width * 0.5,
                    start_angle,
                    end_angle,
                    stroke_c,
                    track_width,
                );
            }
            self.paint_progress_label(frame, label_color, ctx);
            ctx.pop_clip();
            return;
        }

        let radius = self.line_radius(frame);

        // Track (background)
        ctx.fill_rect(frame, track_c, radius);

        match self.mode {
            ProgressMode::Determinate(p) => {
                let fill_w = frame.w * p;
                if fill_w > 0.0 {
                    let fill_rect = Rect::new(frame.x, frame.y, fill_w, frame.h);
                    if self.steps > 1 {
                        let gap = (frame.w / self.steps as f32 * layout.step_gap_ratio)
                            .min(layout.step_gap_max);
                        for index in 0..self.steps {
                            let start = frame.x + index as f32 * frame.w / self.steps as f32;
                            let end = frame.x + (index + 1) as f32 * frame.w / self.steps as f32;
                            let visible_end = (end - gap).min(frame.x + fill_w);
                            if visible_end > start {
                                self.paint_line_fill(
                                    ctx,
                                    Rect::new(start, frame.y, visible_end - start, frame.h),
                                    frame,
                                    stroke_c,
                                );
                            }
                        }
                    } else {
                        self.paint_line_fill(ctx, fill_rect, frame, stroke_c);
                    }
                }
                self.paint_progress_label(frame, label_color, ctx);
            }
            ProgressMode::Indeterminate => {
                let bar_w = frame.w * layout.indeterminate_bar_width_ratio;
                let bar_x = frame.x + (frame.w - bar_w) * self.indeterminate_phase;
                let bar_rect = Rect::new(bar_x, frame.y, bar_w, frame.h);
                ctx.fill_rect(bar_rect, stroke_c, radius);
            }
        }
        ctx.pop_clip();
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !matches!(self.mode, ProgressMode::Indeterminate) {
            return false;
        }

        self.previous_indeterminate_phase = self.indeterminate_phase;
        self.indeterminate_phase = (self.indeterminate_phase
            + dt as f32 * self.visual.motion.phase_speed)
            .rem_euclid(1.0);
        true
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        let frame = Self::normalize_frame(frame);
        match self.mode {
            ProgressMode::Indeterminate if self.progress_type == ProgressType::Circle => {
                self.circle_indeterminate_bounds(frame)
            }
            ProgressMode::Indeterminate => self.line_indeterminate_bounds(frame),
            ProgressMode::Determinate(_) => Rect::zero(),
        }
    }

    // 初次挂载时只为非法动态输入记录一次可观察诊断。
    on_mount => (&mut self) {
        // 把首次归一化事实报告给现有 tracing 诊断边界。
        self.report_normalization_if_needed();
    }
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

// 把 UIX 声明的共享视觉配置融合进进度条 Rust 状态与绘制内核。
fn build_progress_view(mut kernel: ProgressBar, declared_visual: ProgressVisual) -> ViewNode {
    let visual = UIX_PROGRESS_VISUAL.get_or_init(|| declared_visual);
    // 单一同目录 UIX 源在同一程序中必须保持一份确定配置。
    debug_assert_eq!(*visual, declared_visual);
    if !kernel.width_authored {
        kernel.width = visual.layout.default_width;
    }
    if !kernel.height_authored {
        kernel.height = visual.layout.default_height;
    }
    if !kernel.round_authored {
        kernel.round = visual.layout.default_round;
    }
    if !kernel.progress_type_authored {
        kernel.progress_type = ProgressType::Line;
    }
    if kernel.indeterminate_phase == DEFAULT_PROGRESS_VISUAL.motion.initial_phase {
        kernel.indeterminate_phase = visual.motion.initial_phase;
        kernel.previous_indeterminate_phase = visual.motion.initial_phase;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for ProgressBar {
    fn build(self) -> ViewNode {
        // UIX 拥有视觉配置，Rust 保留归一化、动画状态、几何执行和底层绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/feedback/progress/progress.uix")
    }
}

impl ProgressBar {
    /// 创建零进度、确定模式、圆角线形且尺寸为 200×8 的进度条。
    pub fn new() -> Self {
        Self {
            progress: 0.0,
            mode: ProgressMode::Determinate(0.0),
            // 默认 fraction 合法，无需标记归一化。
            input_normalized: false,
            // 默认状态没有归一化原因。
            normalization_reason: None,
            // 初始状态尚未报告任何归一化原因。
            normalization_reported: false,
            stroke_color: None,
            track_color: None,
            height: DEFAULT_PROGRESS_VISUAL.layout.default_height,
            height_authored: false,
            width: DEFAULT_PROGRESS_VISUAL.layout.default_width,
            width_authored: false,
            round: DEFAULT_PROGRESS_VISUAL.layout.default_round,
            round_authored: false,
            progress_type: ProgressType::Line,
            progress_type_authored: false,
            gradient_start: None,
            gradient_end: None,
            steps: 0,
            dashboard: false,
            format_text: None,
            indeterminate_phase: DEFAULT_PROGRESS_VISUAL.motion.initial_phase,
            previous_indeterminate_phase: DEFAULT_PROGRESS_VISUAL.motion.initial_phase,
            visual: &DEFAULT_PROGRESS_VISUAL,
        }
    }

    /// 设置确定模式的进度比例，并安全归一化到 `0.0..=1.0`。
    pub fn progress(mut self, p: f32) -> Self {
        // 一次计算归一值与原因，供绘制、快照和无障碍共享。
        let (progress, reason) = Self::normalize_progress_with_reason(p);
        // 保存安全 fraction 作为唯一呈现值。
        self.progress = progress;
        // 暴露本次输入是否发生归一化。
        self.input_normalized = reason.is_some();
        // 保存稳定原因供快照与去重日志使用。
        self.normalization_reason = reason;
        // 新声明尚未通过挂载或 reconcile 报告该原因。
        self.normalization_reported = false;
        // 兼容既有 Rust builder：显式 progress 选择确定模式。
        self.mode = ProgressMode::Determinate(self.progress);
        self
    }

    /// `.percent()` 是 `.progress()` 的语义别名，参数仍为 0.0～1.0。
    pub fn percent(self, p: f32) -> Self {
        self.progress(p)
    }

    /// 设置进度部分从起始色到结束色的渐变。
    pub fn gradient(mut self, start: Color, end: Color) -> Self {
        self.gradient_start = Some(start);
        self.gradient_end = Some(end);
        self
    }

    /// 启用分段进度并设置至少为一的分段数量。
    pub fn steps(mut self, count: usize) -> Self {
        self.steps = count.max(1);
        self
    }

    /// 切换为带缺口的圆形仪表盘样式。
    pub fn dashboard(mut self) -> Self {
        self.dashboard = true;
        self.progress_type = ProgressType::Circle;
        self.progress_type_authored = true;
        self
    }

    /// 设置根据当前进度比例生成显示文本的格式化器。
    pub fn format<F>(mut self, formatter: F) -> Self
    where
        F: Fn(f32) -> String + 'static,
    {
        self.format_text = Some(Rc::new(formatter));
        self
    }

    /// 切换为由组件动画相位驱动的不确定进度模式。
    pub fn indeterminate(mut self) -> Self {
        self.mode = ProgressMode::Indeterminate;
        self
    }

    /// 按动态布尔值显式选择模式，并保留确定模式的 fraction。
    pub fn indeterminate_when(mut self, indeterminate: bool) -> Self {
        // 生成器不依赖 builder 调用顺序决定最终模式。
        self.mode = if indeterminate {
            // 不确定模式只使用组件拥有的动画相位。
            ProgressMode::Indeterminate
        } else {
            // 恢复确定模式时复用已归一化的保留 fraction。
            ProgressMode::Determinate(self.progress)
        };
        // 返回完成模式选择的组件。
        self
    }

    /// 设置已完成进度部分的颜色。
    pub fn stroke_color(mut self, c: Color) -> Self {
        self.stroke_color = Some(c);
        self
    }

    /// 设置未完成轨道的颜色。
    pub fn track_color(mut self, c: Color) -> Self {
        self.track_color = Some(c);
        self
    }

    /// 设置非负高度；非有限数值会归零。
    pub fn height(mut self, h: f32) -> Self {
        self.height = Self::normalize_dimension(h);
        self.height_authored = true;
        self
    }

    /// 设置非负宽度；非有限数值会归零。
    pub fn width(mut self, w: f32) -> Self {
        self.width = Self::normalize_dimension(w);
        self.width_authored = true;
        self
    }

    /// 同时设置非负宽高；非有限数值会归零。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = Self::normalize_dimension(w);
        self.height = Self::normalize_dimension(h);
        self.width_authored = true;
        self.height_authored = true;
        self
    }

    /// 设置线形进度条两端是否使用圆角。
    pub fn round(mut self, round: bool) -> Self {
        self.round = round;
        self.round_authored = true;
        self
    }

    /// 切换为完整圆形轨道样式。
    pub fn circle(mut self) -> Self {
        self.progress_type = ProgressType::Circle;
        self.progress_type_authored = true;
        self
    }

    /// 返回不确定模式当前的归一化动画相位。
    pub fn animation_phase(&self) -> f32 {
        self.indeterminate_phase
    }

    /// 返回输入是否经过安全归一化。
    pub fn input_normalized(&self) -> bool {
        // 只读暴露快照同源事实。
        self.input_normalized
    }

    /// 返回最近动态输入的归一化原因。
    pub fn normalization_reason(&self) -> Option<ProgressNormalizationReason> {
        // 只读暴露稳定原因枚举。
        self.normalization_reason
    }

    // 测试目标只读暴露 UIX 视觉默认值，不把私有配置类型扩展到生产 API。
    #[cfg(test)]
    pub(crate) fn visual_contract_for_test(&self) -> (f32, f32, f32) {
        (
            self.visual.layout.default_width,
            self.visual.layout.default_height,
            self.visual.motion.phase_speed,
        )
    }

    // 测试目标验证实例只共享视觉配置引用，不复制完整参数表。
    #[cfg(test)]
    pub(crate) fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }

    fn render_dashboard(
        &self,
        frame: Rect,
        track_color: Color,
        stroke_color: Color,
        label_color: Color,
        ctx: &mut PaintContext,
    ) {
        let layout = &self.visual.layout;
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * layout.dashboard_center_y_ratio;
        let radius = (frame.w * layout.dashboard_radius_width_ratio)
            .min(frame.h * layout.dashboard_radius_height_ratio);
        if radius <= 0.0 {
            return;
        }
        let width = (radius * layout.dashboard_track_width_ratio)
            .clamp(
                layout.dashboard_track_width_min,
                layout.dashboard_track_width_max,
            )
            .min(radius);
        let start = std::f32::consts::TAU * layout.dashboard_start_turns;
        let end = start + std::f32::consts::TAU * layout.dashboard_sweep_turns;
        ctx.stroke_arc(cx, cy, radius, start, end, track_color, width);

        let (progress_start, progress_end) = match self.mode {
            ProgressMode::Determinate(progress) => (start, start + (end - start) * progress),
            ProgressMode::Indeterminate => {
                let sweep = (end - start) * self.visual.motion.dashboard_indeterminate_sweep_ratio;
                let travel = (end - start) - sweep;
                let progress_start = start + travel * self.indeterminate_phase;
                (progress_start, progress_start + sweep)
            }
        };
        if progress_end > progress_start {
            ctx.stroke_arc(
                cx,
                cy,
                radius,
                progress_start,
                progress_end,
                stroke_color,
                width,
            );
        }
        self.paint_progress_label(frame, label_color, ctx);
    }

    fn paint_progress_label(&self, frame: Rect, color: Color, ctx: &mut PaintContext) {
        if let (ProgressMode::Determinate(progress), Some(format)) =
            (self.mode, self.format_text.as_ref())
        {
            ctx.text_center(
                &format(progress),
                frame,
                color,
                self.visual.layout.label_font_size,
            );
        }
    }

    fn paint_line_fill(
        &self,
        ctx: &mut PaintContext,
        rect: Rect,
        gradient_domain: Rect,
        fallback: Color,
    ) {
        let Some((start, end)) = self.gradient_start.zip(self.gradient_end) else {
            ctx.fill_rect(rect, fallback, self.line_radius(rect));
            return;
        };
        if !self.round {
            let color_a = gradient_color_at(start, end, gradient_domain, rect.x);
            let color_b = gradient_color_at(start, end, gradient_domain, rect.x + rect.w);
            ctx.fill_linear_gradient(
                rect,
                color_a,
                color_b,
                self.visual.layout.gradient_direction,
            );
            return;
        }

        let cap_radius = (rect.h * 0.5).min(rect.w * 0.5);
        if cap_radius <= 0.0 {
            return;
        }
        let left_center = rect.x + cap_radius;
        let right_center = rect.x + rect.w - cap_radius;
        if right_center > left_center {
            let center = Rect::new(left_center, rect.y, right_center - left_center, rect.h);
            ctx.fill_linear_gradient(
                center,
                gradient_color_at(start, end, gradient_domain, left_center),
                gradient_color_at(start, end, gradient_domain, right_center),
                self.visual.layout.gradient_direction,
            );
        }
        let cy = rect.y + rect.h * 0.5;
        ctx.fill_circle(
            left_center,
            cy,
            cap_radius,
            gradient_color_at(start, end, gradient_domain, left_center),
        );
        if right_center > left_center {
            ctx.fill_circle(
                right_center,
                cy,
                cap_radius,
                gradient_color_at(start, end, gradient_domain, right_center),
            );
        }
    }

    fn line_indeterminate_bounds(&self, frame: Rect) -> Rect {
        let previous = self.indeterminate_bar_rect(frame, self.previous_indeterminate_phase);
        let current = self.indeterminate_bar_rect(frame, self.indeterminate_phase);
        previous
            .union(&current)
            .intersect(&frame)
            .unwrap_or_default()
    }

    fn indeterminate_bar_rect(&self, frame: Rect, phase: f32) -> Rect {
        let bar_w = frame.w * self.visual.layout.indeterminate_bar_width_ratio;
        let bar_x = frame.x + (frame.w - bar_w) * phase;
        Rect::new(bar_x, frame.y, bar_w, frame.h)
    }

    fn circle_indeterminate_bounds(&self, frame: Rect) -> Rect {
        let r = frame.w.min(frame.h) * self.visual.layout.circle_radius_ratio
            + self.visual.layout.dirty_padding;
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        Rect::new(cx - r, cy - r, r * 2.0, r * 2.0)
            .intersect(&frame)
            .unwrap_or_default()
    }

    fn line_radius(&self, rect: Rect) -> Option<Radius> {
        self.round
            .then(|| Radius::uniform(rect.w.min(rect.h) * self.visual.layout.line_radius_ratio))
    }

    fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            Self::normalize_dimension(frame.w),
            Self::normalize_dimension(frame.h),
        )
    }

    fn intrinsic_size(&self) -> Size {
        if self.progress_type == ProgressType::Circle {
            let d = self.width.max(self.height);
            return Size::new(d, d);
        }
        Size::new(self.width, self.height)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ProgressBar {
            progress: self.progress,
            mode: self.mode,
            // 快照暴露最近输入是否经过归一化。
            input_normalized: self.input_normalized,
            // 快照暴露稳定归一化原因。
            normalization_reason: self.normalization_reason,
            stroke_color: self.stroke_color,
            track_color: self.track_color,
            height: self.height,
            width: self.width,
            round: self.round,
            progress_type: self.progress_type,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // 对任何内部构造路径再次执行安全归一化。
        let (progress, fallback_reason) = Self::normalize_progress_with_reason(next.progress);
        // 优先保留声明构建期记录的原始归一化原因。
        let next_reason = next.normalization_reason.or(fallback_reason);
        // 记录原因类别是否发生状态转换。
        let normalization_changed = self.normalization_reason != next_reason;
        // 同步唯一安全 fraction。
        self.progress = progress;
        self.mode = match next.mode {
            // 确定模式始终消费与快照同源的唯一 fraction。
            ProgressMode::Determinate(_) => ProgressMode::Determinate(self.progress),
            // 不确定模式继续由组件动画相位驱动。
            ProgressMode::Indeterminate => ProgressMode::Indeterminate,
        };
        // 同步可观察归一化标记。
        self.input_normalized = next_reason.is_some();
        // 同步稳定原因分类。
        self.normalization_reason = next_reason;
        // 原因变化后允许报告一次新状态。
        if normalization_changed {
            // 新的合法状态清除报告，新的非法类别等待本次报告。
            self.normalization_reported = false;
        }
        // reconcile 只在合法转非法或非法类别变化时记录一次。
        self.report_normalization_if_needed();
        self.stroke_color = next.stroke_color;
        self.track_color = next.track_color;
        self.height = Self::normalize_dimension(next.height);
        self.height_authored = next.height_authored;
        self.width = Self::normalize_dimension(next.width);
        self.width_authored = next.width_authored;
        self.round = next.round;
        self.round_authored = next.round_authored;
        self.progress_type = next.progress_type;
        self.progress_type_authored = next.progress_type_authored;
        self.gradient_start = next.gradient_start;
        self.gradient_end = next.gradient_end;
        self.steps = next.steps;
        self.dashboard = next.dashboard;
        self.format_text = next.format_text;
        self.visual = next.visual;
    }

    /// 同时返回安全 fraction 与可观察归一化原因。
    fn normalize_progress_with_reason(value: f32) -> (f32, Option<ProgressNormalizationReason>) {
        // 非有限值统一回退公开默认值。
        if !value.is_finite() {
            // 返回零值与非有限原因。
            return (0.0, Some(ProgressNormalizationReason::NonFinite));
        }
        // 低于 fraction 下界时安全截断。
        if value < 0.0 {
            // 返回下界与越界原因。
            return (0.0, Some(ProgressNormalizationReason::BelowRange));
        }
        // 高于 fraction 上界时安全截断。
        if value > 1.0 {
            // 返回上界与越界原因。
            return (1.0, Some(ProgressNormalizationReason::AboveRange));
        }
        // 合法 fraction 原样进入唯一呈现状态。
        (value, None)
    }

    /// 按原因状态转换至多记录一次归一化警告。
    fn report_normalization_if_needed(&mut self) {
        // 合法输入清除旧报告门闩。
        let Some(reason) = self.normalization_reason else {
            // 允许未来再次进入非法状态时报告。
            self.normalization_reported = false;
            // 合法状态不产生警告。
            return;
        };
        // 同一非法原因在当前连续状态中不重复记录。
        if self.normalization_reported {
            // 保持每个原因状态只报告一次。
            return;
        }
        // 使用稳定原因字段发布现有 tracing 警告。
        tracing::warn!(
            // 原因字段供日志消费者稳定筛选。
            normalization_reason = reason.as_str(),
            // 文本说明运行时已安全归一化。
            "ProgressBar 动态 progress 输入已归一化到 0.0..=1.0"
        );
        // 标记当前原因已经报告。
        self.normalization_reported = true;
    }

    fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }
}

fn interpolate_color(start: Color, end: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);
    Color::from_rgba(
        (start.r as f32 + (end.r as f32 - start.r as f32) * t).round() as u8,
        (start.g as f32 + (end.g as f32 - start.g as f32) * t).round() as u8,
        (start.b as f32 + (end.b as f32 - start.b as f32) * t).round() as u8,
        (start.a as f32 + (end.a as f32 - start.a as f32) * t).round() as u8,
    )
}

fn gradient_color_at(start: Color, end: Color, domain: Rect, x: f32) -> Color {
    let amount = if domain.w > 0.0 {
        (x - domain.x) / domain.w
    } else {
        0.0
    };
    interpolate_color(start, end, amount)
}
