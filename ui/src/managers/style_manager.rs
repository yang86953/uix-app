use uix_graphics::Color;
use uix_platform::EdgeInsets;

/// 预留给将来按需扩展的 widget 样式预设。
/// 当前未集成到 widget 系统中。如需使用，请直接使用 `ui::style::Style`。
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetStylePreset {
    pub bg_color: Option<Color>,
    pub text_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub border_radius: f32,
    pub padding: EdgeInsets,
    pub opacity: f32,
    pub elevation: f32,
    pub shadow_color: Color,
    pub shadow_blur: f32,
    pub shadow_offset_x: f32,
    pub shadow_offset_y: f32,
}

impl Default for WidgetStylePreset {
    fn default() -> Self {
        Self {
            bg_color: None,
            text_color: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            padding: EdgeInsets::zero(),
            opacity: 1.0,
            elevation: 0.0,
            shadow_color: Color::from_rgba(0, 0, 0, 64),
            shadow_blur: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
        }
    }
}

/// Manages widget Style — properties separate from layout.
/// 备注：当前未集成到 widget 系统中，保留为将来扩展预留。
#[derive(Default, Clone)]
pub struct StyleManager {
    style: WidgetStylePreset,
}

impl StyleManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn style(&self) -> &WidgetStylePreset {
        &self.style
    }
    pub fn style_mut(&mut self) -> &mut WidgetStylePreset {
        &mut self.style
    }

    pub fn set_bg(&mut self, color: Color) {
        self.style.bg_color = Some(color);
    }
    pub fn bg(&self) -> Option<Color> {
        self.style.bg_color
    }

    pub fn set_rounded(&mut self, radius: f32) {
        self.style.border_radius = radius;
    }
    pub fn rounded(&self) -> f32 {
        self.style.border_radius
    }

    pub fn set_border(&mut self, color: Color, width: f32) {
        self.style.border_color = Some(color);
        self.style.border_width = width;
    }

    pub fn set_elevation(&mut self, elevation: f32) {
        self.style.elevation = elevation;
    }
    pub fn elevation(&self) -> f32 {
        self.style.elevation
    }

    pub fn set_padding(&mut self, p: EdgeInsets) {
        self.style.padding = p;
    }
    pub fn padding(&self) -> EdgeInsets {
        self.style.padding
    }
}
