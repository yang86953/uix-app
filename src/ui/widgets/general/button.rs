//! Button — 纯文本按钮，外观由 StyleSet 预设驱动；点击反馈为 Material 风格 ripple。

use std::sync::{Arc, OnceLock};

use super::icon::Icon;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::geometry::path::{FillRule, Path, PathBuilder};
use crate::draw::Color;
use crate::impl_widget_component;
use crate::native::traits::input::{ControlSize, KeyCode, MouseButton};
use crate::ui::animation::{Animation, Easing};
use crate::ui::style::{apply_style, ColorValue, PaletteColor, Style, StyleSet, StyleState};
use crate::ui::traits::{EventHandler, WidgetAnimation, WidgetLayout, WidgetRender};
use crate::ui::{EventResult, SystemEvent, WidgetTree};
use crate::ui::{SnapshotFields, SnapshotSource};

const DEFAULT_BUTTON_FONT_SIZE: f32 = 14.0;

fn normalized_button_font_size(size: f32) -> f32 {
    if size.is_finite() && size > 0.0 {
        size
    } else {
        DEFAULT_BUTTON_FONT_SIZE
    }
}

/// Material 风格水波纹：从触点扩大到盖住按钮，松手后淡出收束。
pub(crate) struct ButtonRipple {
    /// 按钮局部坐标原点（相对 frame 左上角）。
    pub(crate) origin: Point,
    pub(crate) expand: Animation<f32>,
    /// 按住时保持 1；松手后 1→0 淡出。
    fade: Animation<f32>,
    held: bool,
}

impl ButtonRipple {
    pub(crate) const EXPAND_SECS: f64 = 0.32;
    pub(crate) const FADE_SECS: f64 = 0.2;

    pub(crate) fn start(origin: Point) -> Self {
        Self {
            origin,
            expand: Animation::new(0.0, 1.0, Self::EXPAND_SECS).easing(Easing::CubicOut),
            fade: Animation::new(1.0, 1.0, 0.0),
            held: true,
        }
    }

    fn release(&mut self) {
        if !self.held {
            return;
        }
        self.held = false;
        let current = self.fade.value();
        self.fade = Animation::new(current, 0.0, Self::FADE_SECS).easing(Easing::QuadOut);
    }

    fn update(&mut self, dt: f64) -> bool {
        self.expand.update(dt);
        self.fade.update(dt);
        if self.held {
            // 扩到满后静止绘制，无需空转帧（#105）。
            !self.expand.is_finished()
        } else {
            !self.expand.is_finished() || !self.fade.is_finished()
        }
    }

    fn is_visible(&self) -> bool {
        self.held || !self.fade.is_finished()
    }

    fn opacity(&self) -> f32 {
        self.fade.value().clamp(0.0, 1.0)
    }
}

/// 按钮在 ButtonGroup 中的位置，控制视觉圆角连接。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonGroupPosition {
    Left,
    Middle,
    Right,
    Single,
}

pub(crate) fn cover_radius(origin: Point, size: Size) -> f32 {
    let corners = [(0.0, 0.0), (size.w, 0.0), (0.0, size.h), (size.w, size.h)];
    corners
        .into_iter()
        .map(|(x, y)| {
            let dx = x - origin.x;
            let dy = y - origin.y;
            (dx * dx + dy * dy).sqrt()
        })
        .fold(0.0_f32, f32::max)
}

pub(crate) fn rounded_rect_circle_intersection(
    rect: Rect,
    corner_radius: f32,
    center: Point,
    circle_radius: f32,
) -> Option<Path> {
    if rect.w <= 0.0
        || rect.h <= 0.0
        || !corner_radius.is_finite()
        || !circle_radius.is_finite()
        || circle_radius <= 0.0
    {
        return None;
    }

    let corner_radius = corner_radius.max(0.0).min(rect.w.min(rect.h) * 0.5);
    let clip = rounded_rect_points(rect, corner_radius);
    let mut subject = circle_points(center, circle_radius);
    for edge in clip.windows(2) {
        subject = clip_convex_polygon(&subject, edge[0], edge[1]);
        if subject.len() < 3 {
            return None;
        }
    }
    subject = clip_convex_polygon(&subject, *clip.last()?, clip[0]);
    if subject.len() < 3 {
        return None;
    }

    let mut path = PathBuilder::new();
    path.move_to(subject[0].x, subject[0].y);
    for point in &subject[1..] {
        path.line_to(point.x, point.y);
    }
    path.close();
    Some(path.build())
}

fn rounded_rect_points(rect: Rect, radius: f32) -> Vec<Point> {
    if radius <= f32::EPSILON {
        return vec![
            Point::new(rect.x, rect.y),
            Point::new(rect.x + rect.w, rect.y),
            Point::new(rect.x + rect.w, rect.y + rect.h),
            Point::new(rect.x, rect.y + rect.h),
        ];
    }

    let segments = arc_segments(radius, std::f32::consts::FRAC_PI_2);
    let mut points = Vec::with_capacity(segments * 4 + 4);
    let corners = [
        (
            rect.x + rect.w - radius,
            rect.y + radius,
            -std::f32::consts::FRAC_PI_2,
        ),
        (rect.x + rect.w - radius, rect.y + rect.h - radius, 0.0),
        (
            rect.x + radius,
            rect.y + rect.h - radius,
            std::f32::consts::FRAC_PI_2,
        ),
        (rect.x + radius, rect.y + radius, std::f32::consts::PI),
    ];
    for (cx, cy, start) in corners {
        for step in 0..=segments {
            let angle = start + std::f32::consts::FRAC_PI_2 * step as f32 / segments as f32;
            points.push(Point::new(
                cx + radius * angle.cos(),
                cy + radius * angle.sin(),
            ));
        }
    }
    points
}

fn circle_points(center: Point, radius: f32) -> Vec<Point> {
    let segments = arc_segments(radius, std::f32::consts::TAU);
    (0..segments)
        .map(|step| {
            let angle = std::f32::consts::TAU * step as f32 / segments as f32;
            Point::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

fn arc_segments(radius: f32, sweep: f32) -> usize {
    // At UI sizes a 0.1 px sagitta is visually indistinguishable from the SDF
    // primitive while keeping path tessellation well below its vertex budget.
    const TOLERANCE: f32 = 0.1;
    let max_angle = if radius <= TOLERANCE {
        sweep
    } else {
        (2.0 * (1.0 - TOLERANCE / radius).clamp(-1.0, 1.0).acos()).max(0.05)
    };
    (sweep / max_angle).ceil().clamp(1.0, 128.0) as usize
}

fn clip_convex_polygon(subject: &[Point], edge_start: Point, edge_end: Point) -> Vec<Point> {
    let Some(mut previous) = subject.last().copied() else {
        return Vec::new();
    };
    let mut previous_inside = edge_side(edge_start, edge_end, previous) >= -1e-5;
    let mut output = Vec::with_capacity(subject.len() + 1);

    for &current in subject {
        let current_inside = edge_side(edge_start, edge_end, current) >= -1e-5;
        if current_inside != previous_inside {
            if let Some(point) = line_intersection(previous, current, edge_start, edge_end) {
                output.push(point);
            }
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_inside = current_inside;
    }
    output
}

fn edge_side(edge_start: Point, edge_end: Point, point: Point) -> f32 {
    (edge_end.x - edge_start.x) * (point.y - edge_start.y)
        - (edge_end.y - edge_start.y) * (point.x - edge_start.x)
}

fn line_intersection(
    line_start: Point,
    line_end: Point,
    edge_start: Point,
    edge_end: Point,
) -> Option<Point> {
    let line_dx = line_end.x - line_start.x;
    let line_dy = line_end.y - line_start.y;
    let edge_dx = edge_end.x - edge_start.x;
    let edge_dy = edge_end.y - edge_start.y;
    let denominator = edge_dx * line_dy - edge_dy * line_dx;
    if denominator.abs() <= f32::EPSILON {
        return None;
    }
    let t = (edge_dx * (edge_start.y - line_start.y) - edge_dy * (edge_start.x - line_start.x))
        / denominator;
    Some(Point::new(
        line_start.x + line_dx * t.clamp(0.0, 1.0),
        line_start.y + line_dy * t.clamp(0.0, 1.0),
    ))
}

/// 按钮组件。业务绑定不存放在组件内，由 HandlerTable 按 ComponentId 管理。
pub struct Button {
    text: String,
    button_size: ControlSize,
    disabled: bool,
    block: bool,
    loading: bool,
    loading_phase: f32,
    loading_dirty: bool,
    /// 图标名称（Lucide），纯图标按钮时 text 为空。
    icon: String,
    /// ButtonGroup 中的位置，控制视觉圆角。
    group_position: Option<ButtonGroupPosition>,
    hovered: bool,
    pub(crate) pressed: bool,
    pub(crate) focused: bool,
    pub(crate) ripple: Option<ButtonRipple>,
    /// 上一帧 `update_animation` 是否推进了 ripple（供窄标脏）。
    ripple_dirty: bool,
    pub(crate) style_set: Arc<StyleSet>,
    pub(crate) style: Arc<Style>,
}

impl_widget_component!(Button; Layout, Render, Event, Animation; tab_index => 1);

impl SnapshotSource for Button {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Button {
            text: self.text.clone(),
            disabled: self.disabled,
            block: self.block,
            loading: self.loading,
            icon: self.icon.clone(),
            group_position: self.group_position,
            style_set: self.style_set.clone(),
            style: self.style.clone(),
        }
    }
}

impl WidgetLayout for Button {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    fn flex_grow(&self) -> f32 {
        self.style.flex_grow
    }

    fn flex_shrink(&self) -> f32 {
        self.style.flex_shrink
    }

    fn layout_margin(&self) -> crate::core::EdgeInsets {
        self.style.margin
    }

    fn align_self(&self) -> Option<crate::ui::layout::AlignItems> {
        self.style.align_self
    }

    fn grid_cell(&self) -> Option<usize> {
        self.style.grid_cell
    }

    fn grid_column_span(&self) -> u32 {
        self.style.grid_column_span
    }

    fn grid_row_span(&self) -> u32 {
        self.style.grid_row_span
    }
}

impl EventHandler for Button {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled || self.loading {
            return EventResult::NotHandled;
        }

        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                self.pressed = true;
                self.ripple = Some(ButtonRipple::start(*pos));
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                button: MouseButton::Left,
                ..
            } => {
                self.pressed = false;
                if let Some(ripple) = self.ripple.as_mut() {
                    ripple.release();
                }
                EventResult::Handled
            }
            SystemEvent::PointerEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.pressed = false;
                if let Some(ripple) = self.ripple.as_mut() {
                    ripple.release();
                }
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed = false;
                if let Some(ripple) = self.ripple.as_mut() {
                    ripple.release();
                }
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if matches!(*key, KeyCode::Enter | KeyCode::Space) => {
                self.pressed = true;
                // 键盘激活：哨兵原点 → 绘制时取按钮中心。
                self.ripple = Some(ButtonRipple::start(Self::CENTER_ORIGIN));
                EventResult::Handled
            }
            SystemEvent::KeyUp { key, .. } if matches!(*key, KeyCode::Enter | KeyCode::Space) => {
                self.pressed = false;
                if let Some(ripple) = self.ripple.as_mut() {
                    ripple.release();
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }
}

impl WidgetRender for Button {
    fn render(&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let style = if tree.keyboard_focus_visible() {
            self.resolve_style()
        } else {
            self.resolve_style_with_focus_visibility(false)
        };
        apply_style(ctx, frame, &style);
        self.paint_ripple(frame, ctx, &style);
        if self.loading {
            self.paint_loading_spinner(frame, ctx, &style);
        } else if !self.icon.is_empty() && self.text.is_empty() {
            self.paint_icon(frame, ctx, &style);
        } else if !self.text.is_empty() {
            let content = frame.inset(style.padding);
            let font_size = normalized_button_font_size(style.resolve_font_size(ctx.tokens()));
            let color = style.resolve_color(ctx.tokens());
            // 布局职责：在 content 内交叉轴居中行盒；绘制只顶对齐 blit。
            let text_w = ctx.measure_text(&self.text, font_size).w;
            let text_h = ctx.line_box_height(font_size);
            let text_rect = Rect::new(
                content.x + (content.w - text_w) * 0.5,
                content.y + (content.h - text_h) * 0.5,
                text_w.max(0.0),
                text_h.max(0.0),
            );
            ctx.draw_text(
                &self.text,
                Point::new(text_rect.x, text_rect.y),
                color,
                font_size,
            );
        }
    }

    fn dirty_rect(&self, frame: Rect) -> Rect {
        frame
    }
}

impl WidgetAnimation for Button {
    fn update_animation(&mut self, dt: f64) -> bool {
        self.loading_dirty = false;
        if self.loading {
            let before = self.loading_phase;
            self.loading_phase = (self.loading_phase
                + (dt.max(0.0) as f32 * std::f32::consts::TAU / 0.8))
                .rem_euclid(std::f32::consts::TAU);
            self.loading_dirty = (self.loading_phase - before).abs() > f32::EPSILON;
        }
        let Some(ripple) = self.ripple.as_mut() else {
            self.ripple_dirty = false;
            return self.loading;
        };
        let before_expand = ripple.expand.value();
        let before_fade = ripple.fade.value();
        let active = ripple.update(dt);
        self.ripple_dirty = (ripple.expand.value() - before_expand).abs() > f32::EPSILON
            || (ripple.fade.value() - before_fade).abs() > f32::EPSILON;
        if !ripple.is_visible() {
            self.ripple = None;
            self.ripple_dirty = true;
            return false;
        }
        active || self.loading
    }

    fn dirty_bounds(&self, frame: Rect) -> Rect {
        if self.ripple_dirty || self.loading_dirty {
            frame
        } else {
            Rect::zero()
        }
    }
}

impl Button {
    /// 键盘激活用的中心原点哨兵（局部坐标不可能为负）。
    pub(crate) const CENTER_ORIGIN: Point = Point::new(-1.0, -1.0);

    pub fn new(text: impl Into<String>) -> Self {
        let config = crate::ui::config::use_config();
        let style_set = config
            .overrides
            .button
            .style_set
            .map(Arc::new)
            .unwrap_or_else(default_button_style_set);
        Self::assemble(text.into(), style_set, config.disabled, false, config.size)
    }

    pub(crate) fn assemble(
        text: String,
        style_set: Arc<StyleSet>,
        disabled: bool,
        block: bool,
        button_size: ControlSize,
    ) -> Self {
        Self {
            text,
            button_size,
            disabled,
            block,
            loading: false,
            loading_phase: 0.0,
            loading_dirty: false,
            icon: String::new(),
            group_position: None,
            hovered: false,
            pressed: false,
            focused: false,
            ripple: None,
            ripple_dirty: false,
            style_set,
            style: default_button_style(),
        }
    }

    pub fn style_set(mut self, style_set: StyleSet) -> Self {
        self.style_set = Arc::new(style_set);
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.button_size = size;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = Arc::new(style);
        self
    }

    pub fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.button_size = next.button_size;
        self.disabled = next.disabled;
        self.block = next.block;
        self.loading = next.loading;
        if !self.loading {
            self.loading_phase = 0.0;
            self.loading_dirty = false;
        }
        self.icon = next.icon;
        self.group_position = next.group_position;
        self.style_set = next.style_set;
        self.style = next.style;
    }

    pub fn primary(self) -> Self {
        Self {
            style_set: primary_button_style_set(),
            ..self
        }
    }

    pub fn ghost(self) -> Self {
        Self {
            style_set: ghost_button_style_set(),
            ..self
        }
    }

    pub fn danger(self) -> Self {
        Self {
            style_set: danger_button_style_set(),
            ..self
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn block(mut self, block: bool) -> Self {
        self.block = block;
        self
    }

    /// 显示加载旋转器并禁用交互；保持文本宽度不变。
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// 创建纯图标按钮。
    pub fn icon(name: impl Into<String>) -> Self {
        let config = crate::ui::config::use_config();
        let style_set = config
            .overrides
            .button
            .style_set
            .map(Arc::new)
            .unwrap_or_else(default_button_style_set);
        let mut btn = Self::assemble(
            String::new(),
            style_set,
            config.disabled,
            false,
            config.size,
        );
        btn.icon = name.into();
        btn
    }

    /// 设置 ButtonGroup 中的位置，控制视觉圆角连接。
    pub fn group_position(mut self, pos: ButtonGroupPosition) -> Self {
        self.group_position = Some(pos);
        self
    }

    pub(crate) fn resolve_style(&self) -> Style {
        self.resolve_style_with_focus_visibility(true)
    }

    fn resolve_style_with_focus_visibility(&self, focus_visible: bool) -> Style {
        let mut style = self
            .style_set
            .resolve(StyleState {
                hovered: self.hovered,
                pressed: self.pressed,
                // focused/pressed 预设无色变（#176）；仍传 flags 供自定义 StyleSet。
                focused: self.focused && focus_visible && !self.pressed,
                disabled: self.disabled,
            })
            .apply(self.style.as_ref().clone());
        if let Some(position) = self.group_position {
            if position != ButtonGroupPosition::Single {
                style.border_radius = 0.0;
                if matches!(
                    position,
                    ButtonGroupPosition::Middle | ButtonGroupPosition::Right
                ) {
                    style.border_width.left = 0.0;
                }
            }
        }
        style
    }

    fn intrinsic_size(&self) -> Size {
        let base = self
            .style_set
            .normal
            .clone()
            .apply(self.style.as_ref().clone());
        let font_size = normalized_button_font_size(base.font_size.default_size());
        // 按钮外框高度由 Style 固定；文字行盒在 render 时于 content 内居中。
        let height = base
            .height
            .unwrap_or_else(|| crate::ui::config::control_height(self.button_size));
        // 纯图标按钮使用正方形尺寸。
        let width = if !self.icon.is_empty() && self.text.is_empty() {
            height
        } else {
            // 无 FontService 时使用共享宽字符估算；真实宽在 paint 用 measure_text。
            let text_w = crate::draw::resources::font::text_backend::estimate_text_metrics(
                &self.text,
                f32::INFINITY,
                font_size,
            )
            .max_line_width;
            base.width
                .unwrap_or(text_w + base.padding.horizontal())
                .max(32.0)
        };
        if self.block {
            Size::new(f32::MAX, height)
        } else {
            Size::new(width, height)
        }
    }

    fn paint_ripple(&self, frame: Rect, ctx: &mut PaintContext<'_>, style: &Style) {
        let Some(ripple) = self.ripple.as_ref() else {
            return;
        };
        if !ripple.is_visible() {
            return;
        }

        let size = Size::new(frame.w, frame.h);
        let local = if ripple.origin.x < 0.0 || ripple.origin.y < 0.0 {
            Point::new(size.w * 0.5, size.h * 0.5)
        } else {
            ripple.origin
        };
        let max_r = cover_radius(local, size);
        let radius = max_r * ripple.expand.value();
        if radius <= 0.0 {
            return;
        }

        let ink = Self::ripple_ink_color(style, ctx);
        let alpha = (ink.a as f32 * ripple.opacity()).round() as u8;
        if alpha == 0 {
            return;
        }

        let center = Point::new(frame.x + local.x, frame.y + local.y);
        let color = ink.with_alpha(alpha);
        if style.border_radius > 0.0 {
            if let Some(path) =
                rounded_rect_circle_intersection(frame, style.border_radius, center, radius)
            {
                ctx.fill_path(&path, color, FillRule::NonZero);
            }
        } else {
            ctx.push_clip(frame);
            ctx.fill_circle(center.x, center.y, radius, color);
            ctx.pop_clip();
        }
    }

    /// 绘制纯图标按钮中的 Lucide 图标（复用 icon 模块基础设施）。
    fn paint_icon(&self, frame: Rect, ctx: &mut PaintContext<'_>, style: &Style) {
        let color = style.resolve_color(ctx.tokens());
        let content = frame.inset(style.padding);
        let font_size = normalized_button_font_size(style.resolve_font_size(ctx.tokens()));
        let icon_size = font_size * 1.2;
        let icon_rect = Rect::new(content.x, content.y, content.w, content.h);
        Icon::paint_in_frame(ctx, &self.icon, icon_rect, color, icon_size);
    }

    /// 绘制加载旋转器；只在 loading 状态登记动画帧。
    fn paint_loading_spinner(&self, frame: Rect, ctx: &mut PaintContext<'_>, style: &Style) {
        let color = style.resolve_color(ctx.tokens());
        let content = frame.inset(style.padding);
        let font_size = normalized_button_font_size(style.resolve_font_size(ctx.tokens()));
        let radius = (font_size * 0.42).min(content.w.min(content.h) * 0.35);
        if radius <= 0.0 {
            return;
        }
        let cx = content.x + content.w * 0.5;
        let cy = content.y + content.h * 0.5;
        ctx.stroke_arc(
            cx,
            cy,
            radius,
            self.loading_phase,
            self.loading_phase + std::f32::consts::PI * 1.45,
            color,
            1.8,
        );
    }

    fn ripple_ink_color(style: &Style, ctx: &PaintContext<'_>) -> Color {
        // 实心强调色按钮用浅色波；描边/浅底用深色波。
        let filled_dark = match style.background {
            Some(ColorValue::Palette(PaletteColor::Primary))
            | Some(ColorValue::Palette(PaletteColor::PrimaryHover))
            | Some(ColorValue::Palette(PaletteColor::PrimaryActive))
            | Some(ColorValue::Palette(PaletteColor::Error))
            | Some(ColorValue::Palette(PaletteColor::ErrorBorder)) => true,
            Some(bg) => {
                let c = bg.resolve(ctx.tokens());
                c.a > 200 && !c.is_light()
            }
            None => false,
        };
        if filled_dark {
            Color::white().with_alpha(56)
        } else {
            Color::black().with_alpha(36)
        }
    }
}

fn default_button_style_set() -> Arc<StyleSet> {
    static STYLE_SET: OnceLock<Arc<StyleSet>> = OnceLock::new();
    Arc::clone(STYLE_SET.get_or_init(|| Arc::new(StyleSet::button_default())))
}

fn primary_button_style_set() -> Arc<StyleSet> {
    static STYLE_SET: OnceLock<Arc<StyleSet>> = OnceLock::new();
    Arc::clone(STYLE_SET.get_or_init(|| Arc::new(StyleSet::button_primary())))
}

fn ghost_button_style_set() -> Arc<StyleSet> {
    static STYLE_SET: OnceLock<Arc<StyleSet>> = OnceLock::new();
    Arc::clone(STYLE_SET.get_or_init(|| Arc::new(StyleSet::button_ghost())))
}

fn danger_button_style_set() -> Arc<StyleSet> {
    static STYLE_SET: OnceLock<Arc<StyleSet>> = OnceLock::new();
    Arc::clone(STYLE_SET.get_or_init(|| Arc::new(StyleSet::button_danger())))
}

fn default_button_style() -> Arc<Style> {
    static STYLE: OnceLock<Arc<Style>> = OnceLock::new();
    Arc::clone(STYLE.get_or_init(|| Arc::new(Style::default())))
}
