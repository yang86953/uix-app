//! Rate widget — 星级评分，支持半星、hover 预览、disabled、clearable。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::resources::font::text_backend::estimate_text_metrics;
use crate::draw::{Color, Radius};
use crate::platform::windowing::ControlSize;
use crate::ui::SnapshotFields;
use crate::ui::reactive::state::State;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, View, ViewNode, WidgetId,
    WidgetTree,
};
use crate::widget;
use std::cell::Cell;

// Rate 使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RateRadiusRole {
    Small,
}

impl RateRadiusRole {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存 UIX 声明的星级数量、尺寸映射、字符排版与焦点几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RateVisual {
    default_count: usize,
    small_scale: f32,
    medium_scale: f32,
    large_scale: f32,
    base_cell_width: f32,
    base_font_size: f32,
    custom_character_padding: f32,
    half_ratio: f32,
    default_icon: &'static str,
    focus_stroke_width: f32,
    focus_radius: RateRadiusRole,
    active: ColorValue,
    active_disabled: ColorValue,
    empty: ColorValue,
    empty_disabled: ColorValue,
    focus: ColorValue,
}

// 同目录 UIX 生成唯一评分视觉值及静态借用。
crate::uix_items!("src/ui/widgets/input/rate/rate.uix");

// 保存每帧一次解析后的颜色与圆角。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedRateVisual {
    active: Color,
    active_disabled: Color,
    empty: Color,
    empty_disabled: Color,
    focus: Color,
    focus_radius: f32,
}

impl RateVisual {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedRateVisual {
        ResolvedRateVisual {
            active: self.active.resolve(tokens),
            active_disabled: self.active_disabled.resolve(tokens),
            empty: self.empty.resolve(tokens),
            empty_disabled: self.empty_disabled.resolve(tokens),
            focus: self.focus.resolve(tokens),
            focus_radius: self.focus_radius.resolve(tokens),
        }
    }
}

// 向 UIX 提供圆角和零分配主题角色。
const fn rate_small_radius() -> RateRadiusRole {
    RateRadiusRole::Small
}
const fn rate_active() -> ColorValue {
    ColorValue::Palette(PaletteColor::Warning)
}
const fn rate_active_disabled() -> ColorValue {
    ColorValue::Palette(PaletteColor::WarningBorder)
}
const fn rate_empty() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
const fn rate_empty_disabled() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
const fn rate_focus() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

widget! {
    /// Rate — 星级评分，点击选择分值。
    pub struct Rate {
        count: usize,
        value: usize,
        value_binding: Option<State<u32>>,
        half: bool,
        disabled: bool,
        clearable: bool,
        hover_value: usize,
        focused: bool,
        pending_change: Cell<Option<usize>>,
        character: String,
        rate_size: ControlSize,
        control_rect: Cell<Rect>,
        #[snapshot(skip)]
        /// UIX 声明的星形、排版、焦点与主题角色。
        pub(crate) visual: &'static RateVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(new_value) = self.value_at(*pos) else {
                    return EventResult::NotHandled;
                };
                // clearable: 点击同一个值取消
                if self.clearable && new_value == self.value {
                    self.set_value(0);
                } else {
                    self.set_value(new_value);
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                self.hover_value = self.value_at(*pos).unwrap_or(0);
                EventResult::Handled
            }
            SystemEvent::PointerLeave => { self.hover_value = 0; EventResult::Handled }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => { self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Right | KeyCode::Up => {
                        let max_val = self.max_value();
                        if self.value < max_val {
                            self.set_value(self.value.saturating_add(1));
                        }
                        EventResult::Handled
                    }
                    KeyCode::Left | KeyCode::Down => {
                        if self.value > 0 {
                            self.set_value(self.value - 1);
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|value| SemanticEvent::change(id, value.to_string()))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.capture_bound_value_dependency();
        let control_height = frame.h.max(0.0).min(self.control_height());
        let control_rect = Rect::new(frame.x, frame.y, frame.w.max(0.0), control_height);
        self.control_rect
            .set(Rect::new(0.0, 0.0, control_rect.w, control_rect.h));
        if control_rect.w <= 0.0 || control_rect.h <= 0.0 || self.count == 0 {
            return;
        }

        let visual = self.visual.resolve(ctx.tokens());
        // hover 预览值优先于选中值
        let display_val = if self.hover_value > 0 { self.hover_value } else { self.value };
        let cell_width = self.cell_width_for_height(control_rect.h);
        let font_size = self.font_size_for_height(control_rect.h);
        let active_color = if self.disabled {
            visual.active_disabled
        } else {
            visual.active
        };
        let empty_color = if self.disabled {
            visual.empty_disabled
        } else {
            visual.empty
        };

        ctx.push_clip(control_rect);
        for i in 0..self.count {
            let sx = control_rect.x + i as f32 * cell_width;
            if sx >= control_rect.x + control_rect.w {
                break;
            }
            let star_rect = Rect::new(sx, control_rect.y, cell_width, control_rect.h);
            let filled = if self.half {
                display_val >= i * 2 + 2
            } else {
                display_val > i
            };

            if filled {
                self.paint_character(ctx, star_rect, active_color, font_size);
            } else if self.half && display_val == i * 2 + 1 {
                let half_width = star_rect.w * self.visual.half_ratio;
                ctx.push_clip(Rect::new(
                    star_rect.x,
                    star_rect.y,
                    half_width,
                    star_rect.h,
                ));
                self.paint_character(ctx, star_rect, active_color, font_size);
                ctx.pop_clip();
                ctx.push_clip(Rect::new(
                    star_rect.x + half_width,
                    star_rect.y,
                    star_rect.w - half_width,
                    star_rect.h,
                ));
                self.paint_character(ctx, star_rect, empty_color, font_size);
                ctx.pop_clip();
            } else {
                self.paint_character(ctx, star_rect, empty_color, font_size);
            }
        }
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                control_rect,
                visual.focus,
                self.visual.focus_stroke_width,
                Some(Radius::uniform(visual.focus_radius)),
            );
        }
        ctx.pop_clip();
    }
}

impl Default for Rate {
    fn default() -> Self {
        Self::new()
    }
}

impl Rate {
    fn value_at(&self, pos: Point) -> Option<usize> {
        let frame = self.control_rect.get();
        if !frame.contains(pos) || self.count == 0 {
            return None;
        }
        let cell_width = self.cell_width_for_height(frame.h);
        if !cell_width.is_finite() || cell_width <= 0.0 {
            return None;
        }
        let relative_x = pos.x - frame.x;
        let star_idx = (relative_x / cell_width).floor() as usize;
        if star_idx >= self.count {
            return None;
        }
        if self.half {
            let in_star_x = relative_x - star_idx as f32 * cell_width;
            Some(star_idx * 2 + usize::from(in_star_x >= cell_width * self.visual.half_ratio) + 1)
        } else {
            Some(star_idx + 1)
        }
    }

    fn set_value(&mut self, value: usize) {
        let value = value.min(self.max_value());
        if self.value == value {
            return;
        }
        self.value = value;
        self.write_bound_value();
        self.pending_change.set(Some(value));
    }

    fn max_value(&self) -> usize {
        if self.half {
            self.count.saturating_mul(2)
        } else {
            self.count
        }
    }

    fn state_value(&self) -> u32 {
        self.value.min(u32::MAX as usize) as u32
    }

    fn sync_bound_value(&mut self) {
        let Some(value) = self.value_binding.as_ref().map(State::get) else {
            return;
        };
        self.value = (value as usize).min(self.max_value());
    }

    fn capture_bound_value_dependency(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            let _ = state.get();
        }
    }

    fn write_bound_value(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            let value = self.state_value();
            if state.get() != value {
                state.set(value);
            }
        }
    }

    /// 创建默认五级、整级、可交互且不可清空的评分组件。
    pub fn new() -> Self {
        let config = crate::ui::widget_runtime::config::use_config();
        let visual = RATE_VISUAL_REF;
        Self {
            count: visual.default_count,
            value: 0,
            value_binding: None,
            half: false,
            disabled: false,
            clearable: false,
            hover_value: 0,
            focused: false,
            pending_change: Cell::new(None),
            character: String::new(),
            rate_size: config.size,
            control_rect: Cell::new(Rect::zero()),
            visual,
        }
    }
    /// 设置评分项数量，并将当前值夹紧到新的可用范围。
    pub fn count(mut self, n: usize) -> Self {
        self.count = n;
        if self.value_binding.is_some() {
            self.sync_bound_value();
        } else {
            self.value = self.value.min(self.max_value());
        }
        self
    }

    /// 将评分绑定到外部 `State<u32>`。
    pub fn value(mut self, state: &State<u32>) -> Self {
        self.value_binding = Some(state.clone());
        self.sync_bound_value();
        self
    }

    /// 设置非受控评分的初始值。
    pub fn default_value(mut self, value: u32) -> Self {
        self.value_binding = None;
        self.value = (value as usize).min(self.max_value());
        self
    }

    /// 返回当前评分状态值；半级模式下一个单位表示半级。
    pub fn current_value(&self) -> u32 {
        self.state_value()
    }

    /// 启用半级评分，并将状态值解释为半级单位。
    pub fn allow_half(mut self) -> Self {
        self.half = true;
        if self.value_binding.is_some() {
            self.sync_bound_value();
        } else {
            self.value = self.value.min(self.max_value());
        }
        self
    }
    /// 设置组件是否禁用指针与键盘评分交互。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    /// 允许再次选择当前评分时将其清零。
    pub fn clearable(mut self) -> Self {
        self.clearable = true;
        self
    }
    /// 设置每个评分项绘制的自定义字符；空字符串使用默认星形。
    pub fn character(mut self, c: impl Into<String>) -> Self {
        self.character = c.into();
        self
    }
    /// 设置评分项采用的控件尺寸规格。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.rate_size = size;
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.count as f32 * self.cell_width(), self.control_height())
    }

    fn control_height(&self) -> f32 {
        crate::ui::widget_runtime::config::control_height(self.rate_size)
    }

    fn visual_scale(&self) -> f32 {
        match self.rate_size {
            ControlSize::Small => self.visual.small_scale,
            ControlSize::Medium => self.visual.medium_scale,
            ControlSize::Large => self.visual.large_scale,
        }
    }

    fn cell_width(&self) -> f32 {
        self.cell_width_for_height(self.control_height())
    }

    fn visual_scale_for_height(&self, height: f32) -> f32 {
        if self.control_height() > 0.0 {
            self.visual_scale() * (height / self.control_height()).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn cell_width_for_height(&self, height: f32) -> f32 {
        let scale = self.visual_scale_for_height(height);
        let base_width = self.visual.base_cell_width * scale;
        if self.character.is_empty() {
            return base_width;
        }
        let font_size = self.visual.base_font_size * scale;
        let text_width =
            estimate_text_metrics(&self.character, f32::INFINITY, font_size).max_line_width;
        base_width.max(text_width + self.visual.custom_character_padding * scale)
    }

    fn font_size_for_height(&self, height: f32) -> f32 {
        self.visual.base_font_size * self.visual_scale_for_height(height)
    }

    fn paint_character(&self, ctx: &mut PaintContext, frame: Rect, color: Color, font_size: f32) {
        if self.character.is_empty() {
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.default_icon,
                frame,
                color,
                font_size,
            );
        } else {
            ctx.text_center(&self.character, frame, color, font_size);
        }
    }
}

impl Rate {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Rate {
            count: self.count,
            value: self.value,
            half: self.half,
            disabled: self.disabled,
            clearable: self.clearable,
            character: self.character.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_value = next.value_binding.as_ref().map(|_| next.value);
        self.count = next.count;
        self.value_binding = next.value_binding;
        self.half = next.half;
        self.disabled = next.disabled;
        self.clearable = next.clearable;
        self.character = next.character;
        self.rate_size = next.rate_size;
        self.value = controlled_value.unwrap_or_else(|| self.value.min(self.max_value()));
        self.visual = next.visual;
    }
}

// 把 Rate Rust 评分内核与 UIX 静态视觉组合为单一组件节点。
fn build_rate_view(mut kernel: Rate, visual: &'static RateVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Rate {
    fn build(self) -> ViewNode {
        build_rate_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_rate_uix_root(kernel: Rate) -> ViewNode {
    crate::uix!("src/ui/widgets/input/rate/rate.uix")
}
