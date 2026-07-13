use std::collections::HashMap;
use std::fmt;

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
    pub validator_key: Option<FormValidatorKey>,
}

impl ValidationRule {
    pub fn custom(key: impl Into<FormValidatorKey>) -> Self {
        Self {
            required: false,
            message: String::new(),
            min: None,
            max: None,
            pattern: None,
            validator_key: Some(key.into()),
        }
    }

    pub fn required(msg: &str) -> Self {
        Self {
            required: true,
            message: msg.to_string(),
            min: None,
            max: None,
            pattern: None,
            validator_key: None,
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

    /// Associates this rule with a validator held in [`FormValidatorTable`].
    pub fn validator(mut self, key: impl Into<FormValidatorKey>) -> Self {
        self.validator_key = Some(key.into());
        self
    }
}

/// Stable name used to associate a validation rule with an external validator.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FormValidatorKey(String);

impl FormValidatorKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FormValidatorKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<&str> for FormValidatorKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for FormValidatorKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

type FormValidator = Box<dyn Fn(&str) -> Result<(), String> + 'static>;

/// Application-owned custom validators, separate from the `Form` component.
#[derive(Default)]
pub struct FormValidatorTable {
    validators: HashMap<FormValidatorKey, FormValidator>,
}

impl FormValidatorTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F>(&mut self, key: impl Into<FormValidatorKey>, validator: F) -> &mut Self
    where
        F: Fn(&str) -> Result<(), String> + 'static,
    {
        self.validators.insert(key.into(), Box::new(validator));
        self
    }

    pub fn remove(&mut self, key: &FormValidatorKey) -> bool {
        self.validators.remove(key).is_some()
    }

    pub fn contains(&self, key: &FormValidatorKey) -> bool {
        self.validators.contains_key(key)
    }

    fn get(&self, key: &FormValidatorKey) -> Option<&FormValidator> {
        self.validators.get(key)
    }
}

/// Invalid custom-validator wiring is reported instead of being silently skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormValidationError {
    MissingValidator {
        field: String,
        key: FormValidatorKey,
    },
}

impl fmt::Display for FormValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValidator { field, key } => {
                write!(
                    formatter,
                    "field `{field}` references missing validator `{key}`"
                )
            }
        }
    }
}

impl std::error::Error for FormValidationError {}

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

impl FieldDef {
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            value: String::new(),
            rules: Vec::new(),
            status: ValidateStatus::None,
            message: String::new(),
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    pub fn rule(mut self, rule: ValidationRule) -> Self {
        self.rules.push(rule);
        self
    }
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

    pub fn with_field(mut self, field: FieldDef) -> Self {
        self.insert_field(field);
        self
    }

    pub fn insert_field(&mut self, field: FieldDef) {
        self.fields.insert(field.name.clone(), field);
    }

    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.get(name)
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

    pub fn validate(
        &mut self,
        validators: &FormValidatorTable,
    ) -> Result<bool, FormValidationError> {
        let mut all_valid = true;
        let field_results: Vec<(String, ValidationResult)> = self
            .fields
            .iter()
            .map(|(name, field)| {
                Self::validate_field(field, validators).map(|result| (name.clone(), result))
            })
            .collect::<Result<_, _>>()?;

        for (name, result) in &field_results {
            if let Some(field) = self.fields.get_mut(name.as_str()) {
                field.status = result.status;
                field.message = result.message.clone();
            }
            if result.status == ValidateStatus::Error {
                all_valid = false;
            }
        }

        Ok(all_valid)
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

    fn validate_field(
        field: &FieldDef,
        validators: &FormValidatorTable,
    ) -> Result<ValidationResult, FormValidationError> {
        for rule in &field.rules {
            if rule.required && field.value.is_empty() {
                return Ok(ValidationResult {
                    status: ValidateStatus::Error,
                    message: rule.message.clone(),
                });
            }
            if let Some(min) = rule.min {
                if let Ok(v) = field.value.parse::<f64>() {
                    if v < min {
                        return Ok(ValidationResult {
                            status: ValidateStatus::Error,
                            message: rule.message.clone(),
                        });
                    }
                }
            }
            if let Some(max) = rule.max {
                if let Ok(v) = field.value.parse::<f64>() {
                    if v > max {
                        return Ok(ValidationResult {
                            status: ValidateStatus::Error,
                            message: rule.message.clone(),
                        });
                    }
                }
            }
            if let Some(ref pattern) = rule.pattern {
                if !field.value.is_empty() && !simple_pattern_match(&field.value, pattern) {
                    return Ok(ValidationResult {
                        status: ValidateStatus::Error,
                        message: rule.message.clone(),
                    });
                }
            }
            if let Some(key) = &rule.validator_key {
                let Some(validator_fn) = validators.get(key) else {
                    return Err(FormValidationError::MissingValidator {
                        field: field.name.clone(),
                        key: key.clone(),
                    });
                };
                if let Err(msg) = validator_fn(&field.value) {
                    return Ok(ValidationResult {
                        status: ValidateStatus::Error,
                        message: msg,
                    });
                }
            }
        }
        Ok(ValidationResult {
            status: ValidateStatus::None,
            message: String::new(),
        })
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
