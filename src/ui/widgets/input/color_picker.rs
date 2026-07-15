//! ColorPicker widget — 颜色选择器，Ant Design 风格。
//!
//! 预设色板选择，点击触发弹出面板。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::native::traits::input::ControlSize;
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, SemanticEvent, SnapshotFields, SystemEvent, WidgetTree,
};
use std::cell::Cell;

const PRESET_COLORS: &[u32] = &[
    0xF52222, 0xFA541C, 0xFA8C16, 0xFADB14, 0x52C41A, 0x13C2C2, 0x1677FF, 0x2F54EB, 0x722ED1,
    0xEB2F96, 0xFF85C0, 0xFFEC3D, 0x95DE64, 0x5CDBD3, 0x85A5FF, 0xB37FEB, 0xF0F0F0, 0xD9D9D9,
    0xBFBFBF, 0x8C8C8C, 0x434343, 0x262626, 0x1F1F1F, 0x141414,
];

// ColorPicker — 颜色选择器。
component! {
    pub struct ColorPicker {
        value: Cell<Color>,
        value_binding: Option<State<Color>>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        preset_colors: Vec<Color>,
        hovered: bool,
        hovered_idx: Option<usize>,
        focused: bool,
        picker_size: ControlSize,
        pending_change: Cell<Option<Color>>,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                if pos.y >= 0.0 && pos.y <= self.control_height() {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    self.focused = true;
                    return EventResult::Handled;
                }
                if self.is_present() && pos.y > self.control_height() {
                    let cols = 8;
                    let cell = 24.0;
                    let pad = 8.0;
                    let panel_x = pos.x;
                    let panel_y = pos.y - self.panel_offset();
                    let panel_w = cols as f32 * cell + pad * 2.0;
                    let rows = self.preset_colors.len().div_ceil(cols);
                    let panel_h = rows as f32 * cell + pad * 2.0;
                    if panel_x >= pad
                        && panel_x < panel_w - pad
                        && panel_y >= pad
                        && panel_y < panel_h - pad
                    {
                        let ci = ((panel_x - pad) / cell) as usize;
                        let ri = ((panel_y - pad) / cell) as usize;
                        let idx = ri * cols + ci;
                        if idx < self.preset_colors.len() {
                            let next = self.preset_colors[idx];
                            self.commit_value(next);
                            self.close();
                            return EventResult::Handled;
                        }
                    }
                    self.close();
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.is_present() && pos.y > self.panel_offset() {
                    let cols = 8;
                    let cell = 24.0;
                    let pad = 8.0;
                    let panel_x = pos.x;
                    let panel_y = pos.y - self.panel_offset();
                    let ci = ((panel_x - pad) / cell) as usize;
                    let ri = ((panel_y - pad) / cell) as usize;
                    let idx = ri * cols + ci;
                    if idx < self.preset_colors.len() && panel_x >= pad && panel_y >= pad {
                        self.hovered_idx = Some(idx);
                    } else {
                        self.hovered_idx = None;
                    }
                } else {
                    self.hovered = pos.y >= 0.0 && pos.y <= self.control_height();
                    self.hovered_idx = None;
                }
                EventResult::Handled
            }
            SystemEvent::PointerLeave => { self.hovered = false; self.hovered_idx = None; EventResult::Handled }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => { self.close(); self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                if *key == KeyCode::Escape
                    && self.open { self.close(); return EventResult::Handled; }
                if *key == KeyCode::Space || *key == KeyCode::Enter {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    self.focused = true;
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|value| SemanticEvent::change(id, value.to_string()))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            color_picker_dirty_rect(frame, self.preset_colors.len())
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.sync_bound_value();
        let border = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let swatch_inset = 4.0;
        let swatch_size = (frame.h - swatch_inset * 2.0).max(8.0);
        let swatch = Rect::new(
            frame.x,
            frame.y + swatch_inset,
            swatch_size,
            swatch_size,
        );
        ctx.fill_rect(swatch, self.value.get(), r);
        let border_c = if self.hovered || self.focused { primary } else { border };
        ctx.stroke_rect(swatch, border_c, 1.5, r);

        if self.focused {
            ctx.stroke_rect(
                Rect::new(
                    swatch.x - 1.0,
                    swatch.y - 1.0,
                    swatch.w + 2.0,
                    swatch.h + 2.0,
                ),
                primary,
                1.0,
                None,
            );
        }

        if self.is_present() {
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let cols = 8;
            let cell = 24.0;
            let pad = 8.0;
            let panel_w = cols as f32 * cell + pad * 2.0;
            let rows = self.preset_colors.len().div_ceil(cols);
            let panel_h = rows as f32 * cell + pad * 2.0;
            let panel_x = frame.x;
            let panel_y = frame.y + frame.h + 4.0;
            let bg = fade_color(ctx.tokens().color_bg_elevated(), opacity);
            let border = fade_color(border, opacity);
            let panel_rect = Rect::new(panel_x, panel_y, panel_w, panel_h);
            let panel_radius = Some(Radius::uniform(ctx.tokens().border_radius()));
            ctx.fill_rect(panel_rect, bg, panel_radius);
            ctx.stroke_rect(panel_rect, border, 1.0, panel_radius);

            for (i, c) in self.preset_colors.iter().enumerate() {
                let cx = panel_x + pad + (i % cols) as f32 * cell;
                let cy = panel_y + pad + (i / cols) as f32 * cell;
                let cell_rect = Rect::new(cx + 1.0, cy + 1.0, cell - 2.0, cell - 2.0);
                ctx.fill_rect(cell_rect, fade_color(*c, opacity), Some(Radius::uniform(2.0)));
                if self.hovered_idx == Some(i) {
                    ctx.stroke_rect(
                        cell_rect,
                        fade_color(Color::white(), opacity),
                        1.5,
                        Some(Radius::uniform(2.0)),
                    );
                }
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        color_picker_dirty_rect(frame, self.preset_colors.len())
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.open = false;
            self.closing = false;
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            color_picker_dirty_rect(frame, self.preset_colors.len())
        } else {
            Rect::zero()
        }
    }
}
impl ColorPicker {
    fn intrinsic_size(&self) -> Size {
        let height = self.control_height();
        Size::new(height, height)
    }

    pub fn new() -> Self {
        let config = crate::ui::config::use_config();
        Self {
            value: Cell::new(Color::default()),
            value_binding: None,
            open: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
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
            picker_size: config.size,
            pending_change: Cell::new(None),
        }
    }
    /// 将颜色绑定到外部 `State<Color>`。
    pub fn value(mut self, state: &State<Color>) -> Self {
        self.value_binding = Some(state.clone());
        self.value.set(state.get());
        self
    }

    /// 设置非受控颜色选择器的初始值。
    pub fn default_value(mut self, value: Color) -> Self {
        self.value_binding = None;
        self.value.set(value);
        self
    }

    /// 返回组件当前缓存值；controlled 用法应以绑定的 `State` 为真值来源。
    pub fn current_value(&self) -> Color {
        self.value.get()
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.picker_size = size;
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    pub fn open(&mut self) {
        self.open = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }

        self.open = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ColorPicker {
            value: self.value.get(),
            preset_colors: self.preset_colors.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_value = next.value_binding.as_ref().map(|_| next.value.get());
        self.value_binding = next.value_binding;
        self.preset_colors = next.preset_colors;
        self.picker_size = next.picker_size;
        if let Some(value) = controlled_value {
            self.value.set(value);
        }
    }

    fn sync_bound_value(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            self.value.set(state.get());
        }
    }

    fn commit_value(&self, value: Color) {
        if self.value.get() == value {
            return;
        }
        self.value.set(value);
        if let Some(state) = self.value_binding.as_ref() {
            if state.get() != value {
                state.set(value);
            }
        }
        self.pending_change.set(Some(value));
    }

    fn control_height(&self) -> f32 {
        crate::ui::config::control_height(self.picker_size)
    }

    fn panel_offset(&self) -> f32 {
        self.control_height() + 4.0
    }
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self::new()
    }
}

fn color_picker_dirty_rect(frame: Rect, color_count: usize) -> Rect {
    let cols = 8usize;
    let cell = 24.0;
    let pad = 8.0;
    let panel_w = cols as f32 * cell + pad * 2.0;
    let rows = color_count.div_ceil(cols);
    let panel_h = rows as f32 * cell + pad * 2.0;
    let panel = Rect::new(frame.x, frame.y + frame.h + 4.0, panel_w, panel_h);
    frame.union(&panel)
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}
