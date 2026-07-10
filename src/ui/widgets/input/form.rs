use std::collections::HashMap;

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

pub struct ValidationRule {
    pub required: bool,
    pub message: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub pattern: Option<String>,
    pub validator: Option<Box<dyn Fn(&str) -> Result<(), String> + 'static>>,
}

impl ValidationRule {
    pub fn required(msg: &str) -> Self {
        Self {
            required: true,
            message: msg.to_string(),
            min: None,
            max: None,
            pattern: None,
            validator: None,
        }
    }

    pub fn min(mut self, v: f64, msg: &str) -> Self {
        self.min = Some(v);
        self.message = msg.to_string();
        self
    }

    pub fn max(mut self, v: f64, msg: &str) -> Self {
        self.max = Some(v);
        self.message = msg.to_string();
        self
    }

    pub fn pattern(mut self, p: &str, msg: &str) -> Self {
        self.pattern = Some(p.to_string());
        self.message = msg.to_string();
        self
    }
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub status: ValidateStatus,
    pub message: String,
}

pub struct FieldDef {
    pub name: String,
    pub label: String,
    pub value: String,
    pub rules: Vec<ValidationRule>,
    pub status: ValidateStatus,
    pub message: String,
}

component! {
    pub struct FormItem {
        label: String,
        name: String,
        required: bool,
        status: ValidateStatus,
        help: String,
        label_width: f32,
        layout: FormLayout,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let error = ctx.tokens().color_error();
        let warning = ctx.tokens().color_warning();
        let success = ctx.tokens().color_success();

        match self.layout {
            FormLayout::Vertical => {
                if !self.label.is_empty() {
                    let lx = frame.x;
                    let ly = frame.y;
                    if self.required {
                        ctx.draw_text("*", Point::new(lx, ly), error, 14.0);
                        ctx.draw_text(&self.label, Point::new(lx + 10.0, ly), text, 14.0);
                    } else {
                        ctx.draw_text(&self.label, Point::new(lx, ly), text, 14.0);
                    }
                }
                self.render_status(ctx, frame, text_sec, error, warning, success);
            }
            FormLayout::Inline => {
                if !self.label.is_empty() {
                    let ly = ctx.visual_center_y(frame, 14.0);
                    if self.required {
                        ctx.draw_text("*", Point::new(frame.x, ly), error, 14.0);
                        ctx.draw_text(&self.label, Point::new(frame.x + 10.0, ly), text, 14.0);
                    } else {
                        ctx.draw_text(&self.label, Point::new(frame.x, ly), text, 14.0);
                    }
                }
            }
            FormLayout::Horizontal => {
                if !self.label.is_empty() {
                    let lx = frame.x + 8.0;
                    let ly = frame.y + 4.0;
                    if self.required {
                        ctx.draw_text("*", Point::new(lx, ly), error, 14.0);
                        ctx.draw_text(&self.label, Point::new(lx + 10.0, ly), text, 14.0);
                    } else {
                        ctx.draw_text(&self.label, Point::new(lx, ly), text, 14.0);
                    }
                }
                self.render_status(ctx, frame, text_sec, error, warning, success);
            }
        }
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
    fn content_rect(&self, frame: Rect) -> Rect {
        let frame_w = frame.w.max(0.0);
        let frame_h = frame.h.max(0.0);
        match self.layout {
            FormLayout::Vertical => {
                let label_h = 18.0f32.min(frame_h);
                Rect::new(
                    frame.x,
                    frame.y + label_h,
                    frame_w,
                    (frame_h - label_h).max(0.0),
                )
            }
            FormLayout::Inline => {
                let label_w = if self.label.is_empty() {
                    0.0
                } else {
                    60.0f32.min(frame_w)
                };
                let pad = 8.0f32.min((frame_w - label_w).max(0.0));
                Rect::new(
                    frame.x + label_w + pad,
                    frame.y + 2.0f32.min(frame_h),
                    (frame_w - label_w - pad).max(0.0),
                    (frame_h - 4.0).max(0.0),
                )
            }
            FormLayout::Horizontal => {
                let label_w = if self.label.is_empty() {
                    0.0
                } else {
                    self.label_width.max(60.0).min(frame_w)
                };
                let pad = 8.0f32.min((frame_w - label_w).max(0.0));
                Rect::new(
                    frame.x + label_w + pad,
                    frame.y + 2.0f32.min(frame_h),
                    (frame_w - label_w - pad).max(0.0),
                    (frame_h - 18.0).max(0.0),
                )
            }
        }
    }

    fn intrinsic_size(&self) -> Size {
        match self.layout {
            FormLayout::Vertical => Size::new(400.0, 56.0),
            FormLayout::Inline => Size::new(200.0, 44.0),
            FormLayout::Horizontal => Size::new(400.0, 44.0),
        }
    }

    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            name: String::new(),
            required: false,
            status: ValidateStatus::None,
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
                Rect::new(frame.x, frame.y, 2.0, frame.h),
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
            ctx.draw_text(
                &self.help,
                Point::new(frame.x + 8.0, frame.y + frame.h - 16.0),
                hc,
                11.0,
            );
        }
    }
}

component! {
    pub struct Form {
        label_width: f32,
        gap: f32,
        layout: FormLayout,
        fields: HashMap<String, FieldDef>,
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

impl Form {
    fn intrinsic_size(&self) -> Size {
        Size::new(400.0, 200.0)
    }

    pub fn new() -> Self {
        Self {
            label_width: 80.0,
            gap: 8.0,
            layout: FormLayout::Horizontal,
            fields: HashMap::new(),
        }
    }

    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = w;
        self
    }

    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }

    pub fn layout(mut self, l: FormLayout) -> Self {
        self.layout = l;
        self
    }

    pub fn set_field_value(&mut self, name: &str, value: &str) {
        if let Some(field) = self.fields.get_mut(name) {
            field.value = value.to_string();
        }
    }

    pub fn set_field_status(&mut self, name: &str, status: ValidateStatus, message: &str) {
        if let Some(field) = self.fields.get_mut(name) {
            field.status = status;
            field.message = message.to_string();
        }
    }

    pub fn validate(&mut self) -> bool {
        let mut all_valid = true;
        let field_results: Vec<(String, ValidationResult)> = self
            .fields
            .iter()
            .map(|(name, field)| (name.clone(), Self::validate_field(field)))
            .collect();

        for (name, result) in &field_results {
            if let Some(field) = self.fields.get_mut(name.as_str()) {
                field.status = result.status;
                field.message = result.message.clone();
            }
            if result.status == ValidateStatus::Error {
                all_valid = false;
            }
        }

        all_valid
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

    fn validate_field(field: &FieldDef) -> ValidationResult {
        for rule in &field.rules {
            if rule.required && field.value.is_empty() {
                return ValidationResult {
                    status: ValidateStatus::Error,
                    message: rule.message.clone(),
                };
            }
            if let Some(min) = rule.min {
                if let Ok(v) = field.value.parse::<f64>() {
                    if v < min {
                        return ValidationResult {
                            status: ValidateStatus::Error,
                            message: rule.message.clone(),
                        };
                    }
                }
            }
            if let Some(max) = rule.max {
                if let Ok(v) = field.value.parse::<f64>() {
                    if v > max {
                        return ValidationResult {
                            status: ValidateStatus::Error,
                            message: rule.message.clone(),
                        };
                    }
                }
            }
            if let Some(ref pattern) = rule.pattern {
                if !field.value.is_empty() && !simple_pattern_match(&field.value, pattern) {
                    return ValidationResult {
                        status: ValidateStatus::Error,
                        message: rule.message.clone(),
                    };
                }
            }
            if let Some(ref validator_fn) = rule.validator {
                if let Err(msg) = validator_fn(&field.value) {
                    return ValidationResult {
                        status: ValidateStatus::Error,
                        message: msg,
                    };
                }
            }
        }
        ValidationResult {
            status: ValidateStatus::None,
            message: String::new(),
        }
    }
}

fn simple_pattern_match(value: &str, pattern: &str) -> bool {
    if !pattern.contains('*') && !pattern.contains('?') {
        return value == pattern;
    }

    let v_chars: Vec<char> = value.chars().collect();
    let p_chars: Vec<char> = pattern.chars().collect();
    let mut vi = 0;
    let mut pi = 0;
    let mut star_vi = None;
    let mut star_pi = None;

    while vi < v_chars.len() {
        if pi < p_chars.len() && (p_chars[pi] == v_chars[vi] || p_chars[pi] == '?') {
            vi += 1;
            pi += 1;
        } else if pi < p_chars.len() && p_chars[pi] == '*' {
            star_vi = Some(vi);
            star_pi = Some(pi);
            pi += 1;
        } else if let (Some(sv), Some(sp)) = (star_vi, star_pi) {
            vi = sv + 1;
            pi = sp + 1;
            star_vi = Some(vi);
        } else {
            return false;
        }
    }

    while pi < p_chars.len() && p_chars[pi] == '*' {
        pi += 1;
    }

    pi >= p_chars.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::compositor::PicturePolicy;
    use crate::ui::core::widget::WidgetCore;
    use crate::ui::traits::{WidgetCapabilities, WidgetLayout};
    use crate::ui::{WidgetComponent, WidgetTree};
    use std::cell::Cell;
    use std::rc::Rc;

    struct FixedChild(Size);

    impl WidgetComponent for FixedChild {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
            self
        }

        fn capabilities(&self) -> WidgetCapabilities {
            WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT)
        }

        crate::wc_upcast!(FixedChild; WidgetLayout);
    }

    impl WidgetLayout for FixedChild {
        fn measure(&self, constraints: Constraints) -> Size {
            constraints.clamp(self.0)
        }
    }

    struct CountingChild {
        size: Size,
        measure_calls: Rc<Cell<usize>>,
    }

    impl WidgetComponent for CountingChild {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
            self
        }

        fn capabilities(&self) -> WidgetCapabilities {
            WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT)
        }

        crate::wc_upcast!(CountingChild; WidgetLayout);
    }

    impl WidgetLayout for CountingChild {
        fn measure(&self, constraints: Constraints) -> Size {
            self.measure_calls.set(self.measure_calls.get() + 1);
            constraints.clamp(self.size)
        }
    }

    fn assert_rect_within(parent: Rect, child: Rect) {
        assert!(child.w >= 0.0);
        assert!(child.h >= 0.0);
        assert!(child.x >= parent.x);
        assert!(child.y >= parent.y);
        assert!(child.x + child.w <= parent.x + parent.w);
        assert!(child.y + child.h <= parent.y + parent.h);
    }

    #[test]
    fn measure_clamps_form_item_size() {
        let measured = FormItem::new("Name").measure(Constraints::loose(Size::new(120.0, 20.0)));

        assert_eq!(measured, Size::new(120.0, 20.0));
    }

    #[test]
    fn measure_clamps_form_size() {
        let measured = Form::new().measure(Constraints::loose(Size::new(160.0, 80.0)));

        assert_eq!(measured, Size::new(160.0, 80.0));
    }

    #[test]
    fn form_shell_is_picture_eligible_but_form_item_is_not() {
        assert_eq!(Form::new().picture_policy(), PicturePolicy::Eligible);
        assert_eq!(FormItem::new("Name").picture_policy(), PicturePolicy::Never);
    }

    #[test]
    fn layout_children_measure_children_with_frame_constraints() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(Form::new().layout(FormLayout::Horizontal)));
        tree.add_child(root, Box::new(FixedChild(Size::new(240.0, 120.0))));
        tree.get_mut(root)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 80.0, 60.0));

        tree.layout();

        let child = tree.get(root).unwrap().children()[0];
        assert_eq!(
            tree.get(child).unwrap().frame(),
            Rect::new(0.0, 0.0, 80.0, 60.0)
        );
    }

    #[test]
    fn form_layout_children_keep_all_items_within_narrow_frame() {
        let frame = Rect::new(10.0, 20.0, 80.0, 20.0);
        for layout in [
            FormLayout::Vertical,
            FormLayout::Inline,
            FormLayout::Horizontal,
        ] {
            let mut tree = WidgetTree::new();
            let root = tree.set_root(Box::new(Form::new().layout(layout)));
            let first = tree.add_child(root, Box::new(FixedChild(Size::new(10.0, 10.0))));
            let second = tree.add_child(root, Box::new(FixedChild(Size::new(10.0, 10.0))));

            let widget = tree.get(root).unwrap().as_layout().unwrap();
            let measured = widget.measure_children(frame, &[first, second], &tree);
            let placements = widget.layout_children(frame, &measured, &tree);

            assert_eq!(placements.len(), 2);
            for &(_, rect) in &placements {
                assert_rect_within(frame, rect);
            }
        }
    }

    #[test]
    fn form_item_layout_children_clamp_label_padding_and_status_regions() {
        let frame = Rect::new(10.0, 20.0, 40.0, 10.0);
        for layout in [
            FormLayout::Vertical,
            FormLayout::Inline,
            FormLayout::Horizontal,
        ] {
            let mut tree = WidgetTree::new();
            let root = tree.set_root(Box::new(FormItem::new("Name").layout(layout)));
            let child = tree.add_child(root, Box::new(FixedChild(Size::new(20.0, 8.0))));

            let widget = tree.get(root).unwrap().as_layout().unwrap();
            let measured = widget.measure_children(frame, &[child], &tree);
            let placements = widget.layout_children(frame, &measured, &tree);

            assert_eq!(placements.len(), 1);
            assert_rect_within(frame, placements[0].1);
        }
    }

    #[test]
    fn form_arrange_uses_precomputed_measurements() {
        let calls = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(Form::new().layout(FormLayout::Inline)));
        let child = tree.add_child(
            root,
            Box::new(CountingChild {
                size: Size::new(160.0, 20.0),
                measure_calls: Rc::clone(&calls),
            }),
        );
        let frame = Rect::new(0.0, 0.0, 200.0, 40.0);
        let widget = tree.get(root).unwrap().as_layout().unwrap();
        let measured = widget.measure_children(frame, &[child], &tree);

        assert_eq!(calls.get(), 1);
        let placements = widget.layout_children(frame, &measured, &tree);

        assert_eq!(calls.get(), 1, "arrange must not measure children again");
        assert_eq!(placements[0].1, Rect::new(0.0, 0.0, 160.0, 40.0));
    }

    #[test]
    fn form_item_arrange_uses_precomputed_measurements() {
        let calls = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(FormItem::new("Name")));
        let child = tree.add_child(
            root,
            Box::new(CountingChild {
                size: Size::new(80.0, 32.0),
                measure_calls: Rc::clone(&calls),
            }),
        );
        let frame = Rect::new(0.0, 0.0, 100.0, 44.0);
        let widget = tree.get(root).unwrap().as_layout().unwrap();
        let measured = widget.measure_children(frame, &[child], &tree);

        assert_eq!(calls.get(), 1);
        let placements = widget.layout_children(frame, &measured, &tree);

        assert_eq!(calls.get(), 1, "arrange must not measure children again");
        assert_eq!(placements[0].1, Rect::new(88.0, 2.0, 12.0, 26.0));
    }
}
