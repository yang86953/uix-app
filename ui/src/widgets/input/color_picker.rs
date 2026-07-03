//! ColorPicker widget — 颜色选择器，Ant Design 风格。
//!
//! 预设色板选择，点击触发弹出面板。

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};
use uix_graphics::{Color, Radius};
use uix_platform::{Rect, Size};

const PRESET_COLORS: &[u32] = &[
    0xF52222, 0xFA541C, 0xFA8C16, 0xFADB14, 0x52C41A, 0x13C2C2, 0x1677FF, 0x2F54EB, 0x722ED1,
    0xEB2F96, 0xFF85C0, 0xFFEC3D, 0x95DE64, 0x5CDBD3, 0x85A5FF, 0xB37FEB, 0xF0F0F0, 0xD9D9D9,
    0xBFBFBF, 0x8C8C8C, 0x434343, 0x262626, 0x1F1F1F, 0x141414,
];

// ColorPicker — 颜色选择器。
define_widget! {
    pub struct ColorPicker {
        value: Color,
        open: bool,
        preset_colors: Vec<Color>,
        hovered: bool,
        hovered_idx: Option<usize>,
        focused: bool,
        on_change: Option<Box<dyn FnMut(Color) + 'static>>,
    }


    tab_index => (&self) -> i32 { 1 }
    preferred_size => (&self, _engine: Option<&dyn uix_graphics::traits::GraphicsEngine>) -> Size {
        Size::new(32.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                if pos.y >= 0.0 && pos.y <= 32.0 {
                    self.open = !self.open;
                    self.focused = true;
                    return EventResult::Handled;
                }
                if self.open && pos.y > 32.0 {
                    let cols = 8;
                    let cell = 24.0;
                    let pad = 8.0;
                    let panel_x = pos.x;
                    let panel_y = pos.y - 40.0;
                    let panel_w = cols as f32 * cell + pad * 2.0;
                    if panel_x >= 0.0 && panel_x < panel_w && panel_y >= pad {
                        let rows = self.preset_colors.len().div_ceil(cols);
                        let panel_h = rows as f32 * cell + pad * 2.0;
                        let ci = ((panel_x - pad) / cell) as usize;
                        let ri = ((panel_y - pad) / cell) as usize;
                        if panel_y >= 0.0 && panel_y <= panel_h {
                            let idx = ri * cols + ci;
                            if idx < self.preset_colors.len() {
                                self.value = self.preset_colors[idx];
                                self.open = false;
                                if let Some(ref mut cb) = self.on_change { cb(self.value); }
                                return EventResult::Handled;
                            }
                        }
                    }
                    self.open = false;
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            WidgetEvent::MouseMove { pos, .. } => {
                if self.open && pos.y > 36.0 {
                    let cols = 8;
                    let cell = 24.0;
                    let pad = 8.0;
                    let panel_x = pos.x;
                    let panel_y = pos.y - 40.0;
                    let ci = ((panel_x - pad) / cell) as usize;
                    let ri = ((panel_y - pad) / cell) as usize;
                    let idx = ri * cols + ci;
                    if idx < self.preset_colors.len() && panel_x >= pad && panel_y >= pad {
                        self.hovered_idx = Some(idx);
                    } else {
                        self.hovered_idx = None;
                    }
                } else {
                    self.hovered = pos.y >= 0.0 && pos.y <= 32.0;
                    self.hovered_idx = None;
                }
                EventResult::Handled
            }
            WidgetEvent::HoverLeave => { self.hovered = false; self.hovered_idx = None; EventResult::Handled }
            WidgetEvent::FocusIn => { self.focused = true; EventResult::Handled }
            WidgetEvent::FocusOut => { self.open = false; self.focused = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                if *key == KeyCode::Escape
                    && self.open { self.open = false; return EventResult::Handled; }
                if *key == KeyCode::Space || *key == KeyCode::Enter {
                    self.open = !self.open;
                    self.focused = true;
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let border = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let swatch = Rect::new(frame.x, frame.y + 4.0, 24.0, 24.0);
        ctx.fill_rect(swatch, self.value, r);
        let border_c = if self.hovered || self.focused { primary } else { border };
        ctx.stroke_rect(swatch, border_c, 1.5, r);

        if self.focused {
            ctx.stroke_rect(Rect::new(frame.x - 1.0, frame.y + 3.0, 26.0, 26.0), primary, 1.0, None);
        }

        if self.open {
            let cols = 8;
            let cell = 24.0;
            let pad = 8.0;
            let panel_w = cols as f32 * cell + pad * 2.0;
            let rows = self.preset_colors.len().div_ceil(cols);
            let panel_h = rows as f32 * cell + pad * 2.0;
            let panel_x = frame.x;
            let panel_y = frame.y + 36.0;
            let bg = ctx.tokens().color_bg_elevated();
            let panel_rect = Rect::new(panel_x, panel_y, panel_w, panel_h);
            let panel_radius = Some(Radius::uniform(ctx.tokens().border_radius()));
            ctx.fill_rect(panel_rect, bg, panel_radius);
            ctx.stroke_rect(panel_rect, border, 1.0, panel_radius);

            for (i, c) in self.preset_colors.iter().enumerate() {
                let cx = panel_x + pad + (i % cols) as f32 * cell;
                let cy = panel_y + pad + (i / cols) as f32 * cell;
                let cell_rect = Rect::new(cx + 1.0, cy + 1.0, cell - 2.0, cell - 2.0);
                ctx.fill_rect(cell_rect, *c, Some(Radius::uniform(2.0)));
                if self.hovered_idx == Some(i) {
                    ctx.stroke_rect(cell_rect, Color::white(), 1.5, Some(Radius::uniform(2.0)));
                }
            }
        }
    }
}
impl ColorPicker {
    pub fn new(value: Color) -> Self {
        Self {
            value,
            open: false,
            preset_colors: PRESET_COLORS
                .iter()
                .map(|&c| {
                    Color::from_rgba(
                        ((c >> 16) & 0xFF) as u8,
                        ((c >> 8) & 0xFF) as u8,
                        (c & 0xFF) as u8,
                        255,
                    )
                })
                .collect(),
            hovered: false,
            hovered_idx: None,
            focused: false,
            on_change: None,
        }
    }
    pub fn value(&self) -> Color {
        self.value
    }
    pub fn set_value(&mut self, v: Color) {
        self.value = v;
    }
    pub fn on_change<F: FnMut(Color) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}
