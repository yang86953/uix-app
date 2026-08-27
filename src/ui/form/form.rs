use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::layout::LayoutChild;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_constraints;
use crate::ui::{SnapshotFields, WidgetId, WidgetTree};
use crate::widget;

/// 表单校验状态：未校验/成功/警告/错误/校验中。
///
/// 与 InputStatus/StepStatus/BadgeStatus/UploadStatus 共享「组件状态」命名模式，
/// 但各自语义与变体独立（本枚举含 None/Validating 校验专用态），勿强行合并。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValidateStatus {
    /// 尚未产生校验结果。
    None,
    /// 字段校验成功。
    Success,
    /// 字段存在不阻止提交的警告。
    Warning,
    /// 字段校验失败。
    Error,
    /// 字段校验仍在进行。
    Validating,
}

// 表单布局枚举与组件配置共享同一类型（config::FormLayout），
// 保持 `crate::ui::form::FormLayout` 公开路径兼容并消除手工映射。
pub use crate::ui::widget_runtime::config::FormLayout;

widget! {
    /// 表单项组件：标签、校验状态条、帮助文本与内容区布局。
    pub struct FormItem {
        label: String,
        name: String,
        required: bool,
        status: ValidateStatus,
        status_controlled: bool,
        help: String,
        label_width: f32,
        layout: FormLayout,
    }

    // 测量：直接返回固有尺寸。
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    // 渲染：按布局绘制标签与状态条/帮助文本。
    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        // 帧尺寸归一化，空帧直接返回。
        let frame = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let error = ctx.tokens().color_error();
        let warning = ctx.tokens().color_warning();
        let success = ctx.tokens().color_success();

        ctx.push_clip(frame);
        self.render_label(ctx, frame, text, error);
        self.render_status(ctx, frame, text_sec, error, warning, success);
        ctx.pop_clip();
    }

    // 测量子项：以内容区为约束逐一测量。
    measure_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let mut output = Vec::with_capacity(children.len());
        self.measure_item_children_into(frame, children, tree, &mut output);
        output
    }

    measure_children_into => (
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>
    ) {
        self.measure_item_children_into(frame, children, tree, output);
    }

    // 布局子项：全部铺满内容区。
    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut output = Vec::with_capacity(children.len());
        self.layout_item_children_into(frame, children, &mut output);
        output
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[LayoutChild],
        _tree: &WidgetTree,
        _scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>
    ) {
        self.layout_item_children_into(frame, children, output);
    }
}

impl FormItem {
    const LABEL_HEIGHT: f32 = 18.0;
    const HELP_HEIGHT: f32 = 16.0;
    const CONTENT_TOP_INSET: f32 = 2.0;
    const LABEL_GAP: f32 = 8.0;
    const INLINE_LABEL_WIDTH: f32 = 60.0;

    // 把内容子项测量结果写入布局树拥有的跨帧数组。
    fn measure_item_children_into(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>,
    ) {
        let content = self.content_rect(frame);
        let constraints = Constraints::loose(Size::new(content.w, content.h));
        output.clear();
        output.reserve(children.len());
        output.extend(children.iter().map(|&id| {
            let mut child = child_from_tree_with_constraints(id, tree, constraints);
            // 子项尺寸夹紧到内容区。
            child.measured_size = constraints.clamp(child.measured_size);
            child
        }));
    }

    // 把铺满内容区的子项位置写入布局树拥有的跨帧数组。
    fn layout_item_children_into(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        let content = self.content_rect(frame);
        output.clear();
        output.reserve(children.len());
        output.extend(children.iter().map(|child| (child.id, content)));
    }

    /// 帮助文本占用的高度（无帮助文本为 0）。
    fn help_height(&self, frame_h: f32) -> f32 {
        if self.help.is_empty() {
            0.0
        } else {
            Self::HELP_HEIGHT.min(frame_h.max(0.0))
        }
    }

    /// 主区域高度 = 总高减去帮助文本高度。
    fn main_height(&self, frame_h: f32) -> f32 {
        (frame_h.max(0.0) - self.help_height(frame_h)).max(0.0)
    }

    /// 按布局计算标签宽度：纵向占满、行内固定 60、水平用配置宽度。
    fn label_width_for_frame(&self, frame_w: f32) -> f32 {
        if self.label.is_empty() {
            0.0
        } else {
            match self.layout {
                FormLayout::Vertical => frame_w.max(0.0),
                FormLayout::Inline => Self::INLINE_LABEL_WIDTH.min(frame_w.max(0.0)),
                FormLayout::Horizontal => self.label_width.max(60.0).min(frame_w.max(0.0)),
            }
        }
    }

    /// 标签矩形：纵向在上方一行，行内/水平在左侧。
    fn label_rect(&self, frame: Rect) -> Rect {
        let frame_w = frame.w.max(0.0);
        let frame_h = frame.h.max(0.0);
        let main_h = self.main_height(frame_h);
        match self.layout {
            FormLayout::Vertical => {
                Rect::new(frame.x, frame.y, frame_w, Self::LABEL_HEIGHT.min(main_h))
            }
            FormLayout::Inline | FormLayout::Horizontal => Rect::new(
                frame.x,
                frame.y,
                self.label_width_for_frame(frame_w),
                main_h,
            ),
        }
    }

    /// 标签文字矩形：水平布局额外留出与内容区的间隙。
    fn label_text_rect(&self, frame: Rect) -> Rect {
        let label = self.label_rect(frame);
        if self.layout == FormLayout::Horizontal {
            let inset = Self::LABEL_GAP.min(label.w.max(0.0));
            Rect::new(
                label.x + inset,
                label.y,
                (label.w - inset).max(0.0),
                label.h,
            )
        } else {
            label
        }
    }

    /// 帮助文本矩形：位于底部。
    fn help_rect(&self, frame: Rect) -> Rect {
        let help_h = self.help_height(frame.h);
        Rect::new(
            frame.x,
            frame.y + (frame.h.max(0.0) - help_h).max(0.0),
            frame.w.max(0.0),
            help_h,
        )
    }

    /// 内容区矩形：纵向在标签下方，行内/水平在标签右侧。
    fn content_rect(&self, frame: Rect) -> Rect {
        let frame_w = frame.w.max(0.0);
        let frame_h = frame.h.max(0.0);
        let main_h = self.main_height(frame_h);
        match self.layout {
            FormLayout::Vertical => {
                let label_h = Self::LABEL_HEIGHT.min(main_h);
                Rect::new(
                    frame.x,
                    frame.y + label_h,
                    frame_w,
                    (main_h - label_h).max(0.0),
                )
            }
            FormLayout::Inline => {
                let label_w = self.label_width_for_frame(frame_w);
                let pad = Self::LABEL_GAP.min((frame_w - label_w).max(0.0));
                let top = Self::CONTENT_TOP_INSET.min(main_h);
                Rect::new(
                    frame.x + label_w + pad,
                    frame.y + top,
                    (frame_w - label_w - pad).max(0.0),
                    (main_h - top).max(0.0),
                )
            }
            FormLayout::Horizontal => {
                let label_w = self.label_width_for_frame(frame_w);
                let pad = Self::LABEL_GAP.min((frame_w - label_w).max(0.0));
                let top = Self::CONTENT_TOP_INSET.min(main_h);
                Rect::new(
                    frame.x + label_w + pad,
                    frame.y + top,
                    (frame_w - label_w - pad).max(0.0),
                    (main_h - top).max(0.0),
                )
            }
        }
    }

    /// 固有尺寸：按布局给默认宽高，有帮助文本时增加高度。
    fn intrinsic_size(&self) -> Size {
        let mut size = match self.layout {
            FormLayout::Vertical => Size::new(400.0, 56.0),
            FormLayout::Inline => Size::new(200.0, 44.0),
            FormLayout::Horizontal => Size::new(400.0, 44.0),
        };
        if !self.help.is_empty() {
            size.h += Self::HELP_HEIGHT;
        }
        size
    }

    /// 创建表单项（水平布局、默认标签文本）。
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            name: String::new(),
            required: false,
            status: ValidateStatus::None,
            status_controlled: false,
            help: String::new(),
            label_width: 80.0,
            layout: FormLayout::Horizontal,
        }
    }

    /// 设置字段名（用于校验与表单取值）。
    pub fn name(mut self, n: impl Into<String>) -> Self {
        self.name = n.into();
        self
    }

    /// 标记必填（标签前显示星号）。
    pub fn required(mut self, v: bool) -> Self {
        self.required = v;
        self
    }

    /// 设置校验状态。
    pub fn status(mut self, s: ValidateStatus) -> Self {
        self.status = s;
        self
    }

    /// 设置受控校验状态：后续 sync 时不会被普通状态覆盖。
    pub(crate) fn controlled_status(mut self, status: ValidateStatus) -> Self {
        self.status = status;
        self.status_controlled = true;
        self
    }

    /// 设置帮助文本。
    pub fn help(mut self, h: &str) -> Self {
        self.help = h.to_string();
        self
    }

    /// 设置标签宽度（水平布局）。
    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = w;
        self
    }

    /// 设置布局（纵向/行内/水平）。
    pub fn layout(mut self, l: FormLayout) -> Self {
        self.layout = l;
        self
    }

    /// 获取当前校验状态。
    pub fn get_status(&self) -> ValidateStatus {
        self.status
    }

    /// 设置校验状态。
    pub fn set_status(&mut self, s: ValidateStatus) {
        self.status = s;
    }

    /// 导出快照字段。
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::FormItem {
            label: self.label.clone(),
            name: self.name.clone(),
            required: self.required,
            help: self.help.clone(),
            label_width: self.label_width,
            layout: self.layout,
        }
    }

    /// 同步快照：受控状态优先，非受控时采用新状态。
    pub(crate) fn sync_from(&mut self, next: Self) {
        self.label = next.label;
        self.name = next.name;
        self.required = next.required;
        // 仅当新状态为受控时覆盖状态，避免非受控更新冲掉校验结果。
        if next.status_controlled {
            self.status = next.status;
        }
        self.status_controlled = next.status_controlled;
        self.help = next.help;
        self.label_width = next.label_width;
        self.layout = next.layout;
    }

    /// 渲染状态条与帮助文本。
    fn render_status(
        &self,
        ctx: &mut PaintContext,
        frame: Rect,
        text_sec: Color,
        error: Color,
        warning: Color,
        success: Color,
    ) {
        // 状态色：错误/警告/成功对应主题色，否则透明（不画状态条）。
        let status_color = match self.status {
            ValidateStatus::Error => error,
            ValidateStatus::Warning => warning,
            ValidateStatus::Success => success,
            _ => Color::transparent(),
        };
        // 左侧 2px 状态条。
        if status_color.a > 0 {
            ctx.fill_rect(
                Rect::new(frame.x, frame.y, 2.0f32.min(frame.w.max(0.0)), frame.h),
                status_color,
                None,
            );
        }
        // 帮助文本：按状态着色，底部一行小字。
        if !self.help.is_empty() {
            let hc = match self.status {
                ValidateStatus::Error => error,
                ValidateStatus::Warning => warning,
                ValidateStatus::Success => success,
                _ => text_sec,
            };
            let help = self.help_rect(frame);
            let inset = Self::LABEL_GAP.min(help.w.max(0.0));
            let text_rect = Rect::new(help.x + inset, help.y, (help.w - inset).max(0.0), help.h);
            // 字号随可用高度收缩。
            let font_size = 11.0 * (text_rect.h / Self::HELP_HEIGHT).clamp(0.0, 1.0);
            if text_rect.w > 0.0 && font_size > 0.0 {
                let y = ctx.visual_center_y(text_rect, font_size);
                ctx.push_clip(text_rect);
                ctx.draw_text(&self.help, Point::new(text_rect.x, y), hc, font_size);
                ctx.pop_clip();
            }
        }
    }

    /// 渲染标签文本（必填项前加星号）。
    fn render_label(&self, ctx: &mut PaintContext, frame: Rect, text: Color, error: Color) {
        if self.label.is_empty() {
            return;
        }
        let label = self.label_text_rect(frame);
        // 字号随标签区高度收缩。
        let font_size = 14.0 * (label.h / Self::LABEL_HEIGHT).clamp(0.0, 1.0);
        if label.w <= 0.0 || font_size <= 0.0 {
            return;
        }
        let y = ctx.visual_center_y(label, font_size);
        ctx.push_clip(label);
        if self.required {
            // 必填星号 + 间隙 + 标签文本。
            ctx.draw_text("*", Point::new(label.x, y), error, font_size);
            let gap = (10.0 * (font_size / 14.0)).min(label.w);
            ctx.draw_text(&self.label, Point::new(label.x + gap, y), text, font_size);
        } else {
            ctx.draw_text(&self.label, Point::new(label.x, y), text, font_size);
        }
        ctx.pop_clip();
    }
}

widget! {
    /// 表单容器组件：按布局排列表单项，支持行内/水平/垂直排布。
    pub struct Form {
        label_width: f32,
        gap: f32,
        layout: FormLayout,
    }

    // 测量：返回固有尺寸。
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    // 允许整体合成离屏缓冲。
    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    // 容器自身不渲染内容。
    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}

    // 测量子项：按布局方向逐个测量并累积位置。
    measure_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let mut output = Vec::with_capacity(children.len());
        self.measure_form_children_into(frame, children, tree, &mut output);
        output
    }

    measure_children_into => (
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>
    ) {
        self.measure_form_children_into(frame, children, tree, output);
    }

    // 布局子项：按测量尺寸顺序摆放。
    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut output = Vec::with_capacity(children.len());
        self.layout_form_children_into(frame, children, &mut output);
        output
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[LayoutChild],
        _tree: &WidgetTree,
        _scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>
    ) {
        self.layout_form_children_into(frame, children, output);
    }
}

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Form {
    fn clone(&self) -> Self {
        Self {
            label_width: self.label_width,
            gap: self.gap,
            layout: self.layout,
        }
    }
}

impl Form {
    /// 固有尺寸：400×200。
    fn intrinsic_size(&self) -> Size {
        Size::new(400.0, 200.0)
    }

    // 按布局方向测量子项，并把结果写入布局树拥有的跨帧数组。
    fn measure_form_children_into(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>,
    ) {
        output.clear();
        output.reserve(children.len());
        match self.layout {
            // 行内：从左向右排，剩余宽度递减，超界夹紧。
            FormLayout::Inline => {
                let mut x = frame.x;
                let right = frame.x + frame.w.max(0.0);
                for &cid in children {
                    let item_x = x.min(right);
                    let remaining_w = (right - item_x).max(0.0);
                    let max = Size::new(remaining_w, frame.h.max(0.0));
                    let child_constraints =
                        Constraints::new(Size::new(120.0f32.min(remaining_w), 0.0), max, None);
                    let mut child = child_from_tree_with_constraints(cid, tree, child_constraints);
                    // 无测量结果时用默认尺寸兜底。
                    child.measured_size = if tree.get(cid).is_some() {
                        child_constraints.clamp(child.measured_size)
                    } else {
                        child_constraints.clamp(Size::new(200.0, 44.0))
                    };
                    x = (item_x + child.measured_size.w + self.gap).min(right);
                    output.push(child);
                }
            }
            // 水平/垂直：从上向下排，剩余高度递减。
            FormLayout::Horizontal | FormLayout::Vertical => {
                let mut y = frame.y;
                let bottom = frame.y + frame.h.max(0.0);
                for &cid in children {
                    let item_y = y.min(bottom);
                    let remaining_h = (bottom - item_y).max(0.0);
                    let max = Size::new(frame.w.max(0.0), remaining_h);
                    let child_constraints =
                        Constraints::new(Size::new(0.0, 44.0f32.min(remaining_h)), max, None);
                    let mut child = child_from_tree_with_constraints(cid, tree, child_constraints);
                    // 无测量结果时用默认尺寸兜底。
                    child.measured_size = if tree.get(cid).is_some() {
                        child_constraints.clamp(child.measured_size)
                    } else {
                        child_constraints.clamp(Size::new(frame.w, 44.0))
                    };
                    y = (item_y + child.measured_size.h + self.gap).min(bottom);
                    output.push(child);
                }
            }
        }
    }

    // 按测量尺寸排列子项，并把位置写入布局树拥有的跨帧数组。
    fn layout_form_children_into(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        output.clear();
        output.reserve(children.len());
        match self.layout {
            // 行内：水平逐个摆放。
            FormLayout::Inline => {
                let mut x = frame.x;
                let right = frame.x + frame.w.max(0.0);
                for child in children {
                    let item_x = x.min(right);
                    let remaining_w = (right - item_x).max(0.0);
                    let item_w = child.measured_size.w.max(0.0).min(remaining_w);
                    output.push((
                        child.id,
                        Rect::new(item_x, frame.y, item_w, frame.h.max(0.0)),
                    ));
                    x = (item_x + item_w + self.gap).min(right);
                }
            }
            // 水平/垂直：垂直逐个摆放。
            FormLayout::Horizontal | FormLayout::Vertical => {
                let mut y = frame.y;
                let bottom = frame.y + frame.h.max(0.0);
                for child in children {
                    let item_y = y.min(bottom);
                    let remaining_h = (bottom - item_y).max(0.0);
                    let item_h = child.measured_size.h.max(0.0).min(remaining_h);
                    output.push((
                        child.id,
                        Rect::new(frame.x, item_y, frame.w.max(0.0), item_h),
                    ));
                    y = (item_y + item_h + self.gap).min(bottom);
                }
            }
        }
    }

    /// 创建表单容器（默认水平布局、标签宽 80、间隙 8）。
    pub fn new() -> Self {
        // 缺省水平布局；与组件配置共享同一枚举，无需手工映射。
        let layout = crate::ui::widget_runtime::config::use_config()
            .overrides
            .form
            .layout
            .unwrap_or(FormLayout::Horizontal);
        Self {
            label_width: 80.0,
            gap: 8.0,
            layout,
        }
    }

    /// 设置全局标签宽度。
    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = w;
        self
    }

    /// 设置表单项间距（非负）。
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = if g.is_finite() { g.max(0.0) } else { 0.0 };
        self
    }

    /// 设置表单布局（行内/水平/垂直）。
    pub fn layout(mut self, l: FormLayout) -> Self {
        self.layout = l;
        self
    }

    /// 表单项布局（供子项快照/同步使用）。
    pub(crate) fn item_layout(&self) -> FormLayout {
        self.layout
    }

    /// 表单项标签宽度（供子项同步使用）。
    pub(crate) fn item_label_width(&self) -> f32 {
        self.label_width
    }

    /// 导出快照字段。
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Form {
            label_width: self.label_width,
            gap: self.gap,
            layout: self.layout,
        }
    }

    /// 同步快照配置。
    pub(crate) fn sync_from(&mut self, next: Self) {
        self.label_width = next.label_width;
        self.gap = next.gap;
        self.layout = next.layout;
    }
}
