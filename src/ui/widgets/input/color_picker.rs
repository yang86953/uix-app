//! ColorPicker widget — 颜色选择器，Ant Design 风格。
//!
//! 预设色板选择，点击触发弹出面板。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::{Color, Radius};
use crate::native::traits::input::ControlSize;
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::Cell;

const PRESET_COLORS: &[u32] = &[
    0xF52222, 0xFA541C, 0xFA8C16, 0xFADB14, 0x52C41A, 0x13C2C2, 0x1677FF, 0x2F54EB, 0x722ED1,
    0xEB2F96, 0xFF85C0, 0xFFEC3D, 0x95DE64, 0x5CDBD3, 0x85A5FF, 0xB37FEB, 0xF0F0F0, 0xD9D9D9,
    0xBFBFBF, 0x8C8C8C, 0x434343, 0x262626, 0x1F1F1F, 0x141414,
];
const PANEL_GAP: f32 = 4.0;
const PANEL_COLUMNS: usize = 8;
const PANEL_CELL: f32 = 24.0;
const PANEL_PADDING: f32 = 8.0;

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
        highlighted_idx: Option<usize>,
        focused: bool,
        picker_size: ControlSize,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<Color>>,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let frame = self.interaction_frame();
                if frame.contains(*pos) {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    self.focused = true;
                    return EventResult::Handled;
                }
                if self.open {
                    if let Some(index) = color_index_at(
                        frame,
                        *pos,
                        self.preset_colors.len(),
                    ) {
                        self.highlighted_idx = Some(index);
                        self.commit_value(self.preset_colors[index]);
                        self.close();
                        return EventResult::Handled;
                    }
                    if color_panel_rect(frame, self.preset_colors.len()).contains(*pos) {
                        self.close();
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let frame = self.interaction_frame();
                let old_hovered = self.hovered;
                let old_highlight = self.highlighted_idx;
                if self.open {
                    self.hovered = frame.contains(*pos);
                    self.highlighted_idx = color_index_at(
                        frame,
                        *pos,
                        self.preset_colors.len(),
                    )
                    .or_else(|| self.default_highlight());
                } else {
                    self.hovered = frame.contains(*pos);
                    self.highlighted_idx = None;
                }
                if self.hovered != old_hovered || self.highlighted_idx != old_highlight {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let old_hovered = self.hovered;
                let old_highlight = self.highlighted_idx;
                self.hovered = false;
                self.highlighted_idx = if self.open {
                    self.default_highlight()
                } else {
                    None
                };
                if self.hovered != old_hovered || self.highlighted_idx != old_highlight {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => { self.close(); self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Escape if self.is_present() => {
                        self.close();
                        EventResult::Handled
                    }
                    KeyCode::Space | KeyCode::Enter if self.open => {
                        if let Some(index) = self.highlighted_idx {
                            self.commit_value(self.preset_colors[index]);
                        }
                        self.close();
                        EventResult::Handled
                    }
                    KeyCode::Space | KeyCode::Enter => {
                        self.open();
                        self.focused = true;
                        EventResult::Handled
                    }
                    KeyCode::Left if self.open => {
                        self.move_highlight(ColorMove::Previous);
                        EventResult::Handled
                    }
                    KeyCode::Right if self.open => {
                        self.move_highlight(ColorMove::Next);
                        EventResult::Handled
                    }
                    KeyCode::Up if self.open => {
                        self.move_highlight(ColorMove::PreviousRow);
                        EventResult::Handled
                    }
                    KeyCode::Down if self.open => {
                        self.move_highlight(ColorMove::NextRow);
                        EventResult::Handled
                    }
                    KeyCode::Home if self.open && !self.preset_colors.is_empty() => {
                        self.highlighted_idx = Some(0);
                        EventResult::Handled
                    }
                    KeyCode::End if self.open && !self.preset_colors.is_empty() => {
                        self.highlighted_idx = Some(self.preset_colors.len() - 1);
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
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
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let border = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let nominal_height = self.control_height();
        let scale = if nominal_height > 0.0 {
            (frame.h.min(frame.w) / nominal_height).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let available = frame.w.min(frame.h).max(0.0);
        let swatch_inset = (4.0 * scale).min(available * 0.5);
        let swatch_size = (available - swatch_inset * 2.0).max(0.0);

        ctx.push_clip(frame);
        if swatch_size > 0.0 {
            let swatch = Rect::new(
                frame.x + swatch_inset,
                frame.y + swatch_inset,
                swatch_size,
                swatch_size,
            );
            let radius = Some(Radius::uniform(
                (ctx.tokens().border_radius_sm() * scale).min(swatch_size * 0.5),
            ));
            paint_transparency_checkerboard(ctx, swatch, scale);
            ctx.fill_rect(swatch, self.value.get(), radius);
            let border_c = if self.hovered || self.focused { primary } else { border };
            ctx.stroke_rect(swatch, border_c, 1.5 * scale, radius);

            if self.focused {
                let focus_outset = scale.min(swatch_inset);
                ctx.stroke_rect(
                    Rect::new(
                        swatch.x - focus_outset,
                        swatch.y - focus_outset,
                        swatch.w + focus_outset * 2.0,
                        swatch.h + focus_outset * 2.0,
                    ),
                    primary,
                    scale,
                    radius,
                );
            }
        }
        ctx.pop_clip();

        if self.is_present() {
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let bg = fade_color(ctx.tokens().color_bg_elevated(), opacity);
            let border = fade_color(border, opacity);
            let panel_rect = color_panel_rect(frame, self.preset_colors.len());
            let panel_radius = Some(Radius::uniform(ctx.tokens().border_radius()));
            ctx.push_clip(panel_rect);
            ctx.fill_rect(panel_rect, bg, panel_radius);
            ctx.stroke_rect(panel_rect, border, 1.0, panel_radius);

            for (i, c) in self.preset_colors.iter().enumerate() {
                let cx = panel_rect.x + PANEL_PADDING + (i % PANEL_COLUMNS) as f32 * PANEL_CELL;
                let cy = panel_rect.y + PANEL_PADDING + (i / PANEL_COLUMNS) as f32 * PANEL_CELL;
                let cell_rect = Rect::new(cx + 1.0, cy + 1.0, PANEL_CELL - 2.0, PANEL_CELL - 2.0);
                ctx.fill_rect(cell_rect, fade_color(*c, opacity), Some(Radius::uniform(2.0)));
                if self.highlighted_idx == Some(i) {
                    let highlight_color = if c.is_light() {
                        Color::black()
                    } else {
                        Color::white()
                    };
                    ctx.stroke_rect(
                        cell_rect,
                        fade_color(highlight_color, opacity),
                        2.0,
                        Some(Radius::uniform(2.0)),
                    );
                }
                if self.value.get() == *c {
                    let icon_color = if c.is_light() {
                        Color::black()
                    } else {
                        Color::white()
                    };
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        "check",
                        cell_rect,
                        fade_color(icon_color, opacity),
                        12.0,
                    );
                }
            }
            ctx.pop_clip();
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        color_picker_dirty_rect(frame, self.preset_colors.len())
    }

    overlay_entry => (&self, id: ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.is_present().then(|| {
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(color_picker_dirty_rect(frame, self.preset_colors.len()))
                .z_index(900)
        })
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
            self.highlighted_idx = None;
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
            highlighted_idx: None,
            focused: false,
            picker_size: config.size,
            last_frame: Cell::new(None),
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
        self.highlighted_idx = self.default_highlight();
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
        self.highlighted_idx = self.selected_index();
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ColorPicker {
            value: self.value.get(),
            preset_colors: self.preset_colors.clone(),
            open: self.open,
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
        if self.open {
            self.highlighted_idx = self.default_highlight();
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

    fn interaction_frame(&self) -> Rect {
        self.last_frame.get().unwrap_or_else(|| {
            Rect::new(0.0, 0.0, self.intrinsic_size().w, self.intrinsic_size().h)
        })
    }

    fn selected_index(&self) -> Option<usize> {
        let value = self.value.get();
        self.preset_colors.iter().position(|color| *color == value)
    }

    fn default_highlight(&self) -> Option<usize> {
        self.selected_index()
            .or_else(|| (!self.preset_colors.is_empty()).then_some(0))
    }

    fn move_highlight(&mut self, direction: ColorMove) {
        let Some(last) = self.preset_colors.len().checked_sub(1) else {
            self.highlighted_idx = None;
            return;
        };
        let current = self.highlighted_idx.unwrap_or(0).min(last);
        let next = match direction {
            ColorMove::Previous => current.saturating_sub(1),
            ColorMove::Next => (current + 1).min(last),
            ColorMove::PreviousRow => current.saturating_sub(PANEL_COLUMNS),
            ColorMove::NextRow => (current + PANEL_COLUMNS).min(last),
        };
        self.highlighted_idx = Some(next);
    }
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self::new()
    }
}

fn color_picker_dirty_rect(frame: Rect, color_count: usize) -> Rect {
    frame.union(&color_panel_rect(frame, color_count))
}

fn color_panel_rect(frame: Rect, color_count: usize) -> Rect {
    let panel_w = PANEL_COLUMNS as f32 * PANEL_CELL + PANEL_PADDING * 2.0;
    let rows = color_count.div_ceil(PANEL_COLUMNS);
    let panel_h = rows as f32 * PANEL_CELL + PANEL_PADDING * 2.0;
    Rect::new(frame.x, frame.y + frame.h + PANEL_GAP, panel_w, panel_h)
}

fn color_index_at(frame: Rect, pos: Point, color_count: usize) -> Option<usize> {
    let panel = color_panel_rect(frame, color_count);
    let content = Rect::new(
        panel.x + PANEL_PADDING,
        panel.y + PANEL_PADDING,
        PANEL_COLUMNS as f32 * PANEL_CELL,
        color_count.div_ceil(PANEL_COLUMNS) as f32 * PANEL_CELL,
    );
    if pos.x < content.x
        || pos.x >= content.x + content.w
        || pos.y < content.y
        || pos.y >= content.y + content.h
    {
        return None;
    }
    let column = ((pos.x - content.x) / PANEL_CELL).floor() as usize;
    let row = ((pos.y - content.y) / PANEL_CELL).floor() as usize;
    let index = row * PANEL_COLUMNS + column;
    (index < color_count).then_some(index)
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

fn paint_transparency_checkerboard(ctx: &mut PaintContext<'_>, frame: Rect, scale: f32) {
    let tile = (4.0 * scale).max(1.0);
    let columns = (frame.w / tile).ceil() as usize;
    let rows = (frame.h / tile).ceil() as usize;
    ctx.push_clip(frame);
    ctx.fill_rect(frame, Color::white(), None);
    for row in 0..rows {
        for column in 0..columns {
            if (row + column) % 2 == 0 {
                ctx.fill_rect(
                    Rect::new(
                        frame.x + column as f32 * tile,
                        frame.y + row as f32 * tile,
                        tile,
                        tile,
                    ),
                    Color::from_rgb(0xD9, 0xD9, 0xD9),
                    None,
                );
            }
        }
    }
    ctx.pop_clip();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorMove {
    Previous,
    Next,
    PreviousRow,
    NextRow,
}
