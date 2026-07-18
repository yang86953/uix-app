use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::layout::{child_from_tree_with_constraints, LayoutChild};
use crate::ui::{ComponentId, SnapshotFields, WidgetTree};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValidateStatus {
    None,
    Success,
    Warning,
    Error,
    Validating,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormLayout {
    Horizontal,
    Vertical,
    Inline,
}

component! {
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

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
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

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let content = self.content_rect(frame);
        let constraints = Constraints::loose(Size::new(content.w, content.h));
        children
            .iter()
            .map(|&id| {
                let mut child = child_from_tree_with_constraints(id, tree, constraints);
                child.measured_size = constraints.clamp(child.measured_size);
                child
            })
            .collect()
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        let content = self.content_rect(frame);
        children.iter().map(|child| (child.id, content)).collect()
    }
}

impl FormItem {
    const LABEL_HEIGHT: f32 = 18.0;
    const HELP_HEIGHT: f32 = 16.0;
    const CONTENT_TOP_INSET: f32 = 2.0;
    const LABEL_GAP: f32 = 8.0;
    const INLINE_LABEL_WIDTH: f32 = 60.0;

    fn help_height(&self, frame_h: f32) -> f32 {
        if self.help.is_empty() {
            0.0
        } else {
            Self::HELP_HEIGHT.min(frame_h.max(0.0))
        }
    }

    fn main_height(&self, frame_h: f32) -> f32 {
        (frame_h.max(0.0) - self.help_height(frame_h)).max(0.0)
    }

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

    fn help_rect(&self, frame: Rect) -> Rect {
        let help_h = self.help_height(frame.h);
        Rect::new(
            frame.x,
            frame.y + (frame.h.max(0.0) - help_h).max(0.0),
            frame.w.max(0.0),
            help_h,
        )
    }

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

    pub fn name(mut self, n: impl Into<String>) -> Self {
        self.name = n.into();
        self
    }

    pub fn required(mut self, v: bool) -> Self {
        self.required = v;
        self
    }

    pub fn status(mut self, s: ValidateStatus) -> Self {
        self.status = s;
        self
    }

    pub(crate) fn controlled_status(mut self, status: ValidateStatus) -> Self {
        self.status = status;
        self.status_controlled = true;
        self
    }

    pub fn help(mut self, h: &str) -> Self {
        self.help = h.to_string();
        self
    }

    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = w;
        self
    }

    pub fn layout(mut self, l: FormLayout) -> Self {
        self.layout = l;
        self
    }

    pub fn get_status(&self) -> ValidateStatus {
        self.status
    }

    pub fn set_status(&mut self, s: ValidateStatus) {
        self.status = s;
    }

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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.label = next.label;
        self.name = next.name;
        self.required = next.required;
        if next.status_controlled {
            self.status = next.status;
        }
        self.status_controlled = next.status_controlled;
        self.help = next.help;
        self.label_width = next.label_width;
        self.layout = next.layout;
    }

    fn render_status(
        &self,
        ctx: &mut PaintContext,
        frame: Rect,
        text_sec: Color,
        error: Color,
        warning: Color,
        success: Color,
    ) {
        let status_color = match self.status {
            ValidateStatus::Error => error,
            ValidateStatus::Warning => warning,
            ValidateStatus::Success => success,
            _ => Color::transparent(),
        };
        if status_color.a > 0 {
            ctx.fill_rect(
                Rect::new(frame.x, frame.y, 2.0f32.min(frame.w.max(0.0)), frame.h),
                status_color,
                None,
            );
        }
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
            let font_size = 11.0 * (text_rect.h / Self::HELP_HEIGHT).clamp(0.0, 1.0);
            if text_rect.w > 0.0 && font_size > 0.0 {
                let y = ctx.visual_center_y(text_rect, font_size);
                ctx.push_clip(text_rect);
                ctx.draw_text(&self.help, Point::new(text_rect.x, y), hc, font_size);
                ctx.pop_clip();
            }
        }
    }

    fn render_label(&self, ctx: &mut PaintContext, frame: Rect, text: Color, error: Color) {
        if self.label.is_empty() {
            return;
        }
        let label = self.label_text_rect(frame);
        let font_size = 14.0 * (label.h / Self::LABEL_HEIGHT).clamp(0.0, 1.0);
        if label.w <= 0.0 || font_size <= 0.0 {
            return;
        }
        let y = ctx.visual_center_y(label, font_size);
        ctx.push_clip(label);
        if self.required {
            ctx.draw_text("*", Point::new(label.x, y), error, font_size);
            let gap = (10.0 * (font_size / 14.0)).min(label.w);
            ctx.draw_text(&self.label, Point::new(label.x + gap, y), text, font_size);
        } else {
            ctx.draw_text(&self.label, Point::new(label.x, y), text, font_size);
        }
        ctx.pop_clip();
    }
}

component! {
    pub struct Form {
        label_width: f32,
        gap: f32,
        layout: FormLayout,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let mut measured = Vec::with_capacity(children.len());
        match self.layout {
            FormLayout::Inline => {
                let mut x = frame.x;
                let right = frame.x + frame.w.max(0.0);
                for &cid in children {
                    let item_x = x.min(right);
                    let remaining_w = (right - item_x).max(0.0);
                    let max = Size::new(remaining_w, frame.h.max(0.0));
                    let child_constraints = Constraints::new(
                        Size::new(120.0f32.min(remaining_w), 0.0),
                        max,
                        None,
                    );
                    let mut child = child_from_tree_with_constraints(cid, tree, child_constraints);
                    child.measured_size = if tree.get(cid).is_some() {
                        child_constraints.clamp(child.measured_size)
                    } else {
                        child_constraints.clamp(Size::new(200.0, 44.0))
                    };
                    x = (item_x + child.measured_size.w + self.gap).min(right);
                    measured.push(child);
                }
            }
            FormLayout::Horizontal | FormLayout::Vertical => {
                let mut y = frame.y;
                let bottom = frame.y + frame.h.max(0.0);
                for &cid in children {
                    let item_y = y.min(bottom);
                    let remaining_h = (bottom - item_y).max(0.0);
                    let max = Size::new(frame.w.max(0.0), remaining_h);
                    let child_constraints = Constraints::new(
                        Size::new(0.0, 44.0f32.min(remaining_h)),
                        max,
                        None,
                    );
                    let mut child = child_from_tree_with_constraints(cid, tree, child_constraints);
                    child.measured_size = if tree.get(cid).is_some() {
                        child_constraints.clamp(child.measured_size)
                    } else {
                        child_constraints.clamp(Size::new(frame.w, 44.0))
                    };
                    y = (item_y + child.measured_size.h + self.gap).min(bottom);
                    measured.push(child);
                }
            }
        }
        measured
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        let mut result = Vec::with_capacity(children.len());
        match self.layout {
            FormLayout::Inline => {
                let mut x = frame.x;
                let right = frame.x + frame.w.max(0.0);
                for child in children {
                    let item_x = x.min(right);
                    let remaining_w = (right - item_x).max(0.0);
                    let item_w = child.measured_size.w.max(0.0).min(remaining_w);
                    result.push((
                        child.id,
                        Rect::new(item_x, frame.y, item_w, frame.h.max(0.0)),
                    ));
                    x = (item_x + item_w + self.gap).min(right);
                }
            }
            FormLayout::Horizontal | FormLayout::Vertical => {
                let mut y = frame.y;
                let bottom = frame.y + frame.h.max(0.0);
                for child in children {
                    let item_y = y.min(bottom);
                    let remaining_h = (bottom - item_y).max(0.0);
                    let item_h = child.measured_size.h.max(0.0).min(remaining_h);
                    result.push((
                        child.id,
                        Rect::new(frame.x, item_y, frame.w.max(0.0), item_h),
                    ));
                    y = (item_y + item_h + self.gap).min(bottom);
                }
            }
        }
        result
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
    fn intrinsic_size(&self) -> Size {
        Size::new(400.0, 200.0)
    }

    pub fn new() -> Self {
        let layout = match crate::ui::config::use_config().overrides.form.layout {
            Some(crate::ui::config::FormLayout::Horizontal) | None => FormLayout::Horizontal,
            Some(crate::ui::config::FormLayout::Vertical) => FormLayout::Vertical,
            Some(crate::ui::config::FormLayout::Inline) => FormLayout::Inline,
        };
        Self {
            label_width: 80.0,
            gap: 8.0,
            layout,
        }
    }

    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = w;
        self
    }

    pub fn gap(mut self, g: f32) -> Self {
        self.gap = if g.is_finite() { g.max(0.0) } else { 0.0 };
        self
    }

    pub fn layout(mut self, l: FormLayout) -> Self {
        self.layout = l;
        self
    }

    pub(crate) fn item_layout(&self) -> FormLayout {
        self.layout
    }

    pub(crate) fn item_label_width(&self) -> f32 {
        self.label_width
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Form {
            label_width: self.label_width,
            gap: self.gap,
            layout: self.layout,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.label_width = next.label_width;
        self.gap = next.gap;
        self.layout = next.layout;
    }
}
