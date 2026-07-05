/// Manages text content and rendering properties.
#[derive(Default, Clone)]
pub struct TextManager {
    text: String,
    font_size: f32,
    font_path: String,
    color: Option<crate::render::Color>,
    placeholder: String,
    bold: bool,
    italic: bool,
    alignment: crate::render::HAlign,
}

impl TextManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn font_size(&self) -> f32 {
        self.font_size
    }
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size;
    }

    pub fn font_path(&self) -> &str {
        &self.font_path
    }
    pub fn set_font_path(&mut self, path: impl Into<String>) {
        self.font_path = path.into();
    }

    pub fn color(&self) -> Option<crate::render::Color> {
        self.color
    }
    pub fn set_color(&mut self, c: crate::render::Color) {
        self.color = Some(c);
    }

    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }
    pub fn set_placeholder(&mut self, p: impl Into<String>) {
        self.placeholder = p.into();
    }

    pub fn alignment(&self) -> &crate::render::HAlign {
        &self.alignment
    }
    pub fn set_alignment(&mut self, a: crate::render::HAlign) {
        self.alignment = a;
    }

    pub fn set_bold(&mut self, v: bool) {
        self.bold = v;
    }
    pub fn bold(&self) -> bool {
        self.bold
    }

    pub fn set_italic(&mut self, v: bool) {
        self.italic = v;
    }
    pub fn italic(&self) -> bool {
        self.italic
    }
}
