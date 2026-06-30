use uix_platform::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::Color;
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;
use std::collections::HashMap;

/// 校验状态。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValidateStatus { None, Success, Warning, Error, Validating }

/// 表单布局模式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormLayout { Horizontal, Vertical, Inline }

/// 校验规则。
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
        Self { required: true, message: msg.to_string(), min: None, max: None, pattern: None, validator: None }
    }
    pub fn min(mut self, v: f64, msg: &str) -> Self { self.min = Some(v); self.message = msg.to_string(); self }
    pub fn max(mut self, v: f64, msg: &str) -> Self { self.max = Some(v); self.message = msg.to_string(); self }
    pub fn pattern(mut self, p: &str, msg: &str) -> Self { self.pattern = Some(p.to_string()); self.message = msg.to_string(); self }
}

/// 校验结果。
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub status: ValidateStatus,
    pub message: String,
}

/// 表单项定义（用于 Form 管理）。
pub struct FieldDef {
    pub name: String,
    pub label: String,
    pub value: String,
    pub rules: Vec<ValidationRule>,
    pub status: ValidateStatus,
    pub message: String,
}

// FormItem — 单个表单项。
define_widget! {
    pub struct FormItem {
        label: String,
        name: String,
        required: bool,
        status: ValidateStatus,
        help: String,
        label_width: f32,
        layout: FormLayout,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        match self.layout {
            FormLayout::Vertical => Size::new(400.0, 56.0),
            FormLayout::Inline => Size::new(200.0, 44.0),
            _ => Size::new(400.0, 44.0),
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
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
                let status_color = match self.status {
                    ValidateStatus::Error => error,
                    ValidateStatus::Warning => warning,
                    ValidateStatus::Success => success,
                    _ => Color::transparent(),
                };
                if status_color.a > 0 {
                    ctx.fill_rect(Rect::new(frame.x, frame.y, 2.0, frame.h), status_color, None);
                }
                if !self.help.is_empty() {
                    let hc = match self.status {
                        ValidateStatus::Error => error,
                        ValidateStatus::Warning => warning,
                        ValidateStatus::Success => success,
                        _ => text_sec,
                    };
                    ctx.draw_text(&self.help, Point::new(frame.x, frame.y + frame.h - 16.0), hc, 11.0);
                }
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
            _ => {
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
                let status_color = match self.status {
                    ValidateStatus::Error => error,
                    ValidateStatus::Warning => warning,
                    ValidateStatus::Success => success,
                    _ => Color::transparent(),
                };
                if status_color.a > 0 {
                    ctx.fill_rect(Rect::new(frame.x, frame.y, 2.0, frame.h), status_color, None);
                }
                if !self.help.is_empty() {
                    let hc = match self.status {
                        ValidateStatus::Error => error,
                        ValidateStatus::Warning => warning,
                        ValidateStatus::Success => success,
                        _ => text_sec,
                    };
                    ctx.draw_text(&self.help, Point::new(frame.x + 8.0, frame.y + frame.h - 16.0), hc, 11.0);
                }
            }
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::widget::WidgetId], _tree: &WidgetTree)
        -> Vec<(crate::widget::WidgetId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }
        match self.layout {
            FormLayout::Vertical => {
                let content_y = frame.y + 18.0;
                let content_h = (frame.h - 18.0).max(28.0);
                children.iter().map(|&cid| (cid, Rect::new(frame.x, content_y, frame.w, content_h))).collect()
            }
            FormLayout::Inline => {
                let label_w = if self.label.is_empty() { 0.0 } else { 60.0 };
                let pad = 8.0;
                let content_x = frame.x + label_w + pad;
                let content_w = (frame.w - label_w - pad).max(80.0);
                children.iter().map(|&cid| (cid, Rect::new(content_x, frame.y + 2.0, content_w, frame.h - 4.0))).collect()
            }
            _ => {
                let pad = 8.0;
                let label_w = if self.label.is_empty() { 0.0 } else { self.label_width.max(60.0) };
                let content_x = frame.x + label_w + pad;
                let content_w = (frame.w - label_w - pad).max(100.0);
                children.iter().map(|&cid| (cid, Rect::new(content_x, frame.y + 2.0, content_w, frame.h - 18.0))).collect()
            }
        }
    }
}

impl FormItem {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(), name: String::new(),
            required: false, status: ValidateStatus::None,
            help: String::new(), label_width: 80.0, layout: FormLayout::Horizontal,
        }
    }
    pub fn name(mut self, n: impl Into<String>) -> Self { self.name = n.into(); self }
    pub fn required(mut self, v: bool) -> Self { self.required = v; self }
    pub fn status(mut self, s: ValidateStatus) -> Self { self.status = s; self }
    pub fn help(mut self, h: &str) -> Self { self.help = h.to_string(); self }
    pub fn label_width(mut self, w: f32) -> Self { self.label_width = w; self }
    pub fn layout(mut self, l: FormLayout) -> Self { self.layout = l; self }
    pub fn get_status(&self) -> ValidateStatus { self.status }
    pub fn set_status(&mut self, s: ValidateStatus) { self.status = s; }
}

/// Form 校验回调。
pub type FormFinishCallback = Box<dyn FnMut(HashMap<String, String>)>;
pub type FormFinishFailedCallback = Box<dyn FnMut(HashMap<String, ValidationResult>)>;

// Form — 表单容器。
define_widget! {
    pub struct Form {
        label_width: f32,
        gap: f32,
        layout: FormLayout,
        fields: HashMap<String, FieldDef>,
        on_finish: Option<FormFinishCallback>,
        on_finish_failed: Option<FormFinishFailedCallback>,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(400.0, 200.0)
    }

    render => (&self, _frame: Rect, _ctx: &mut RenderContext, _tree: &WidgetTree) {}

    layout_children => (&self, frame: Rect, children: &[crate::widget::WidgetId], tree: &WidgetTree)
        -> Vec<(crate::widget::WidgetId, Rect)>
    {
        let mut result = Vec::new();
        match self.layout {
            FormLayout::Inline => {
                let mut x = frame.x;
                for &cid in children {
                    let pref = tree.get(cid).map(|c| c.preferred_size(None)).unwrap_or(Size::new(200.0, 44.0));
                    let item_w = pref.w.max(120.0);
                    result.push((cid, Rect::new(x, frame.y, item_w, frame.h)));
                    x += item_w + self.gap;
                }
            }
            _ => {
                let mut y = frame.y;
                for &cid in children {
                    let pref = tree.get(cid).map(|c| c.preferred_size(None)).unwrap_or(Size::new(frame.w, 44.0));
                    let item_h = pref.h.max(44.0);
                    result.push((cid, Rect::new(frame.x, y, frame.w, item_h)));
                    y += item_h + self.gap;
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
    pub fn new() -> Self {
        Self {
            label_width: 80.0, gap: 8.0, layout: FormLayout::Horizontal,
            fields: HashMap::new(), on_finish: None, on_finish_failed: None,
        }
    }
    pub fn label_width(mut self, w: f32) -> Self { self.label_width = w; self }
    pub fn gap(mut self, g: f32) -> Self { self.gap = g; self }
    pub fn layout(mut self, l: FormLayout) -> Self { self.layout = l; self }
    pub fn on_finish<F: FnMut(HashMap<String, String>) + 'static>(mut self, f: F) -> Self {
        self.on_finish = Some(Box::new(f)); self
    }
    pub fn on_finish_failed<F: FnMut(HashMap<String, ValidationResult>) + 'static>(mut self, f: F) -> Self {
        self.on_finish_failed = Some(Box::new(f)); self
    }

    /// 设置字段值。
    pub fn set_field_value(&mut self, name: &str, value: &str) {
        if let Some(field) = self.fields.get_mut(name) {
            field.value = value.to_string();
        }
    }

    /// 设置字段校验状态。
    pub fn set_field_status(&mut self, name: &str, status: ValidateStatus, message: &str) {
        if let Some(field) = self.fields.get_mut(name) {
            field.status = status;
            field.message = message.to_string();
        }
    }

    /// 校验所有字段，返回是否通过。
    pub fn validate(&mut self) -> bool {
        let mut all_valid = true;
        let mut errors = HashMap::new();
        let mut values = HashMap::new();
        let field_results: Vec<(String, ValidationResult, String)> = self.fields.iter()
            .map(|(name, field)| {
                let result = Self::validate_field(field);
                (name.clone(), result, field.value.clone())
            })
            .collect();

        for (name, result, value) in &field_results {
            if let Some(f) = self.fields.get_mut(name.as_str()) {
                f.status = result.status;
                f.message = result.message.clone();
            }
            if result.status == ValidateStatus::Error {
                all_valid = false;
                errors.insert(name.clone(), result.clone());
            }
            values.insert(name.clone(), value.clone());
        }

        if all_valid {
            if let Some(ref mut cb) = self.on_finish { cb(values); }
        } else {
            if let Some(ref mut cb) = self.on_finish_failed { cb(errors); }
        }
        all_valid
    }

    fn validate_field(field: &FieldDef) -> ValidationResult {
        for rule in &field.rules {
            if rule.required && field.value.is_empty() {
                return ValidationResult { status: ValidateStatus::Error, message: rule.message.clone() };
            }
            if let Some(min) = rule.min {
                if let Ok(v) = field.value.parse::<f64>() {
                    if v < min {
                        return ValidationResult { status: ValidateStatus::Error, message: rule.message.clone() };
                    }
                }
            }
            if let Some(max) = rule.max {
                if let Ok(v) = field.value.parse::<f64>() {
                    if v > max {
                        return ValidationResult { status: ValidateStatus::Error, message: rule.message.clone() };
                    }
                }
            }
            if let Some(ref pattern) = rule.pattern {
                if !field.value.is_empty() {
                    // 简单通配符匹配（* 任意字符，? 单字符）
                    let matched = simple_pattern_match(&field.value, pattern);
                    if !matched {
                        return ValidationResult { status: ValidateStatus::Error, message: rule.message.clone() };
                    }
                }
            }
            if let Some(ref validator_fn) = rule.validator {
                if let Err(msg) = validator_fn(&field.value) { return ValidationResult { status: ValidateStatus::Error, message: msg } }
            }
        }
        ValidationResult { status: ValidateStatus::None, message: String::new() }
    }
}

/// 简单通配符匹配（* 匹配任意序列，? 匹配单字符）。
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
            vi += 1; pi += 1;
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
    while pi < p_chars.len() && p_chars[pi] == '*' { pi += 1; }
    pi >= p_chars.len()
}
