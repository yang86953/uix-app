//! Card widget — Ant Design style container with elevation, shadow, optional
//! title, body, hover feedback, and configurable border radius.

use crate::component;
use crate::core::{Constraints, EdgeInsets, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::children::WidgetChildren;
use crate::ui::layout::{
    flex::compute_flex_layout, AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent,
};
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, SystemEvent, WidgetComponent, WidgetTree};

component! {
    /// Card widget with shadow elevation, hover highlight, and content padding.
    pub struct Card {
        title: Option<String>,
        children: WidgetChildren,
        bordered: bool,
        hoverable: bool,
        hovered: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        padding: f32,
        elevation: u8,
        flex_grow_val: f32,
        actions: Vec<String>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    flex_grow => (&self) -> f32 { self.flex_grow_val }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        self.children.take()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.hoverable {
            match event {
                SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
                SystemEvent::PointerLeave => { self.hovered = false; EventResult::Handled }
                _ => EventResult::NotHandled,
            }
        } else {
            EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let border_radius_lg = ctx.tokens().border_radius_lg();
        let bg_container = ctx.tokens().color_bg_container();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let primary = ctx.tokens().color_primary();
        let border_secondary = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();

        let card_radius = Some(Radius::uniform(border_radius_lg));

        // 阴影绘制由图形引擎内部处理 clip 绕过。
        // 引擎的 draw_box_shadow 会自动恢复到脏区域 clip，
        // 绕过父级 children_clip 但保持在脏区域内。
        draw_elevation_shadow(ctx, frame, self.elevation);

        // Background
        let bg = if self.hovered {
            Color::from_rgb(
                (bg_container.r as f32 * 0.95 + 255.0 * 0.05) as u8,
                (bg_container.g as f32 * 0.95 + 255.0 * 0.05) as u8,
                (bg_container.b as f32 * 0.95 + 255.0 * 0.05) as u8,
            )
        } else {
            bg_elevated
        };
        ctx.fill_rect(frame, bg, card_radius);

        // Top accent line
        if self.elevation > 1 {
            let accent_rect = Rect::new(frame.x + 24.0, frame.y, frame.w - 48.0, 3.0);
            ctx.fill_rect(accent_rect, primary, Some(Radius::uniform(1.5)));
        }

        // Border
        if self.bordered {
            ctx.stroke_rect(frame, border_secondary, 1.0, card_radius);
        }

        // Title
        if let Some(ref title) = self.title {
            let title_rect = Rect::new(frame.x + self.padding, frame.y, frame.w - self.padding * 2.0, 44.0);
            let title_y = ctx.visual_center_y(title_rect, 15.0);
            ctx.draw_text(title, Point::new(frame.x + self.padding, title_y), text, 15.0);
            let sep_y = frame.y + 44.0 + 4.0;
            ctx.fill_rect(Rect::new(frame.x + self.padding, sep_y, frame.w - self.padding * 2.0, 1.0), border_secondary, None);
        }

        // Actions
        if !self.actions.is_empty() {
            let action_h = 40.0;
            let action_y = frame.y + frame.h - action_h;
            ctx.fill_rect(Rect::new(frame.x, action_y, frame.w, action_h), bg_container, None);
            ctx.stroke_rect(Rect::new(frame.x, action_y, frame.w, action_h), border_secondary, 1.0, None);
            let btn_w = frame.w / self.actions.len() as f32;
            for (i, action) in self.actions.iter().enumerate() {
                let btn_rect = Rect::new(frame.x + i as f32 * btn_w, action_y, btn_w, action_h);
                let ay = ctx.visual_center_y(btn_rect, 13.0);
                let text_w = ctx.measure_text(action, 13.0).w;
                ctx.draw_text(action, Point::new(btn_rect.x + (btn_w - text_w) * 0.5, ay), primary, 13.0);
                if i < self.actions.len() - 1 {
                    ctx.fill_rect(Rect::new(btn_rect.x + btn_w - 1.0, action_y + 8.0, 1.0, action_h - 16.0), border_secondary, None);
                }
            }
        }
    }

    // 扩展脏区域覆盖完整阴影渲染范围。
    // 引擎的 draw_box_shadow 会自动恢复到脏区域 clip（绕过父级），
    // 若 dirty_rect 不覆盖阴影边界，会产生像素叠加拖影。
    dirty_rect => (&self, frame: Rect) -> Rect {
        let expand = match self.elevation {
            // elevation 1: layer_3 blur=48, bounds=±49px → ±50
            1 => 50.0,
            // elevation 2: layer_3 blur=66, bounds=±67px → ±70
            2 => 70.0,
            // elevation 3: layer_3 blur=90, bounds=±91px → ±95
            3 => 95.0,
            _ => 0.0,
        };
        if expand > 0.0 {
            Rect::new(frame.x - expand, frame.y - expand, frame.w + expand * 2.0, frame.h + expand * 2.0)
        } else {
            frame
        }
    }

    layout_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }

        // 标题区域占用顶部空间，剩余部分作为 flex column 容器
        let title_offset = if self.title.is_some() { 56.0 } else { self.padding };
        let inner = Rect::new(
            frame.x + self.padding, frame.y + title_offset,
            (frame.w - self.padding * 2.0).max(0.0),
            (frame.h - title_offset - self.padding).max(0.0),
        );
        if inner.w <= 0.0 || inner.h <= 0.0 { return Vec::new(); }

        let child_sizes: Vec<Size> = children
            .iter()
            .map(|&cid| {
                tree.get(cid)
                    .map(|c| c.measure(Constraints::unconstrained()))
                    .unwrap_or_default()
            })
            .collect();

        let flex_children: Vec<FlexChild> = children
            .iter()
            .map(|&cid| {
                let w = tree.get(cid);
                FlexChild {
                    flex_grow: w.and_then(|c| c.as_layout()).map(|l| l.flex_grow()).unwrap_or(0.0),
                    flex_shrink: w.and_then(|c| c.as_layout()).map(|l| l.flex_shrink()).unwrap_or(0.0),
                    align_self: w.and_then(|c| c.as_layout()).and_then(|l| l.align_self()),
                    ..FlexChild::default()
                }
            })
            .collect();

        let input = FlexInput {
            direction: FlexDirection::Column,
            gap: 0.0,
            padding: EdgeInsets::zero(),
            container: inner,
            children: flex_children,
            child_sizes,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Stretch,
            ..FlexInput::default()
        };

        let output = compute_flex_layout(&input);
        children
            .iter()
            .zip(output.child_rects)
            .map(|(&cid, rect)| (cid, rect))
            .collect()
    }
}

/// Render a multi-layer elevation shadow with directional and ambient layers.
///
/// Three distinct layers produce a rich, realistic shadow:
/// 1. **Contact shadow** (standard smoothstep) — tight, y-down offset, sharp edge
/// 2. **Ambient shadow** (super-gaussian falloff) — wider, softer, spread around
/// 3. **Diffuse glow** (super-gaussian) — very wide, no offset, fills uniformly
///
/// Each elevation level scales the layers differently so the visual depth
/// increases naturally: the contact shadow grows slightly, while the ambient
/// and glow layers expand significantly.
fn draw_elevation_shadow(ctx: &mut PaintContext, frame: Rect, elevation: u8) {
    if elevation == 0 {
        return;
    }
    let shadow = ctx.tokens().box_shadow();
    let corner_radius = Some(Radius::uniform(ctx.tokens().border_radius_lg()));

    // Per-elevation scaling factors for each layer role:
    //   (directional_scale, ambient_spread, glow_spread, alpha_boost)
    let (dir_s, amb_s, glow_s, alpha_b) = match elevation {
        1 => (0.8, 1.2, 1.6, 1.1),
        2 => (1.0, 1.6, 2.2, 1.0),
        3 => (1.2, 2.2, 3.0, 0.9),
        _ => return,
    };

    let boost = |c: Color| Color::from_rgba(c.r, c.g, c.b, (c.a as f32 * alpha_b).min(255.0) as u8);

    // ── Layer 1: Contact shadow ──
    // Directional (y-down), tight blur, sharp smoothstep falloff.
    let (ox1, oy1, bl1, col1) = shadow.layer_1;
    if bl1 > 0.0 && col1.a > 0 {
        ctx.draw_box_shadow(
            frame,
            bl1 * dir_s,       // blur
            ox1,               // x-offset (0 = centered contact)
            oy1 * dir_s * 1.5, // y-offset (emphasise downward)
            boost(col1),
            corner_radius,
        );
    }

    // ── Layer 2: Ambient shadow ──
    // Wider soft shadow that spreads around the card.
    // Uses ambient falloff (super-gaussian) for a softer transition.
    let (ox2, oy2, bl2, col2) = shadow.layer_2;
    if bl2 > 0.0 && col2.a > 0 {
        ctx.draw_box_shadow_ambient(
            frame,
            bl2 * amb_s,       // larger blur = wider ambient
            ox2 * 0.3,         // slight x-spread
            oy2 * amb_s * 0.6, // moderate y-offset
            boost(col2),
            corner_radius,
        );
    }

    // ── Layer 3: Diffuse glow ──
    // Wide, uniform glow with no directional bias.
    // Pure ambient occlusion fill — creates the "floating" feel.
    let (_, _, bl3, col3) = shadow.layer_3;
    if bl3 > 0.0 && col3.a > 0 {
        ctx.draw_box_shadow_ambient(
            frame,
            bl3 * glow_s, // very wide blur
            0.0,
            0.0, // no offset — uniform
            boost(col3),
            corner_radius,
        );
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl Card {
    fn intrinsic_size(&self) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(200.0),
            self.fixed_height.unwrap_or(0.0),
        )
    }

    pub fn new() -> Self {
        Self {
            title: None,
            children: WidgetChildren::new(),
            bordered: true,
            hoverable: false,
            hovered: false,
            fixed_width: None,
            fixed_height: None,
            padding: 16.0,
            elevation: 1,
            flex_grow_val: 0.0,
            actions: Vec::new(),
        }
    }

    pub fn title(mut self, t: &str) -> Self {
        self.title = Some(t.to_string());
        self
    }
    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self
    }
    pub fn hoverable(mut self) -> Self {
        self.hoverable = true;
        self
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }
    pub fn padding(mut self, p: f32) -> Self {
        self.padding = p;
        self
    }
    pub fn elevation(mut self, e: u8) -> Self {
        self.elevation = e.min(3);
        self
    }
    pub fn flex_grow(mut self, v: f32) -> Self {
        self.flex_grow_val = v;
        self
    }
    pub fn actions(mut self, list: Vec<impl Into<String>>) -> Self {
        self.actions = list.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn child(self, w: impl WidgetComponent + 'static) -> Self {
        self.children.add(w);
        self
    }
    pub fn children(self, widgets: Vec<Box<dyn WidgetComponent>>) -> Self {
        self.children.set_all(widgets);
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Card {
            title: self.title.clone(),
            bordered: self.bordered,
            hoverable: self.hoverable,
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
            padding: self.padding,
            elevation: self.elevation,
            flex_grow: self.flex_grow_val,
            actions: self.actions.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_card_size() {
        let measured = Card::new()
            .size(240.0, 120.0)
            .measure(Constraints::loose(Size::new(100.0, 60.0)));

        assert_eq!(measured, Size::new(100.0, 60.0));
    }
}
