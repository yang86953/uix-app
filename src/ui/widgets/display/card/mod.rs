//! Card widget — Ant Design style container with elevation, shadow, optional
//! title, body, hover feedback, and configurable border radius.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};

use crate::core::{Constraints, EdgeInsets, Point, Rect, Size};
use crate::draw::painting::PaintPass;
use crate::draw::{Color, Radius};
use crate::ui::SnapshotFields;
use crate::ui::children::WidgetChildren;
use crate::ui::layout::{
    AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent, LayoutChild,
    flex::compute_flex_layout_into,
};
use crate::ui::view::{View, ViewNode};
use crate::widget;
// 导入共享的子项物理内容尺寸计算，避免 Card 自行复制 Flex 边距语义。
use crate::ui::layout::engine::content_size_from_children;
use crate::ui::widget_runtime::paint_context::PaintContext;
// Card 的自动高度必须读取子树自然尺寸，而不是 flex-grow 的零 basis。
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_natural_constraints;
// 接收 ViewAdapter 传入的通用尺寸样式窄契约。
use crate::ui::theme::style::{ColorValue, PaletteColor, Style};
use crate::ui::theme::{NeutralRole, ShadowToken};
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, Widget, WidgetId, WidgetTree,
};

// 保存由 UIX 声明的卡片默认尺寸、内边距与初始外观。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CardDefaultsVisual {
    width: f32,
    height: f32,
    padding: f32,
    bordered: bool,
    elevation: u8,
    max_elevation: u8,
    body_gap: f32,
}

// 保存由 UIX 声明的标题区尺寸、分隔线与字体收缩边界。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CardTitleVisual {
    block_height: f32,
    text_height: f32,
    separator_offset: f32,
    separator_thickness: f32,
    font_size: f32,
    min_font_size: f32,
    vertical_inset: f32,
}

// 保存由 UIX 声明的动作区排版、分隔线与焦点圈几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CardActionVisual {
    height: f32,
    font_size: f32,
    min_font_size: f32,
    horizontal_inset: f32,
    vertical_inset: f32,
    divider_inset: f32,
    divider_thickness: f32,
    focus_inset: f32,
    focus_stroke_width: f32,
    center_ratio: f32,
}

// 保存由 UIX 声明的表面、描边、悬停混色与顶部强调线几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CardSurfaceVisual {
    border_width: f32,
    hover_lighten: f32,
    radius_limit_ratio: f32,
    accent_min_elevation: u8,
    accent_horizontal_inset: f32,
    accent_height: f32,
    accent_radius_ratio: f32,
    shadow_directional_y_scale: f32,
    shadow_ambient_x_scale: f32,
    shadow_ambient_y_scale: f32,
    shadow_glow_offset_x: f32,
    shadow_glow_offset_y: f32,
}

// 保存单级阴影的三个层级缩放与脏区外扩。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CardElevationVisual {
    directional_scale: f32,
    ambient_scale: f32,
    glow_scale: f32,
    alpha_boost: f32,
    dirty_expand: f32,
}

// 卡片圆角使用的主题尺寸角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardRadiusRole {
    Large,
}

impl CardRadiusRole {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> f32 {
        match self {
            Self::Large => tokens.border_radius_lg(),
        }
    }
}

// 卡片阴影使用的主题令牌角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardShadowRole {
    Default,
}

impl CardShadowRole {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ShadowToken {
        match self {
            Self::Default => tokens.box_shadow(),
        }
    }
}

// 保存由 UIX 声明的卡片主题语义色映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CardPaletteVisual {
    background: ColorValue,
    elevated_background: ColorValue,
    primary: ColorValue,
    border: ColorValue,
    text: ColorValue,
    action_hover: ColorValue,
}

// 完整视觉配置由全部 Card 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CardVisual {
    defaults: CardDefaultsVisual,
    title: CardTitleVisual,
    action: CardActionVisual,
    surface: CardSurfaceVisual,
    elevations: [CardElevationVisual; 3],
    radius: CardRadiusRole,
    shadow: CardShadowRole,
    palette: CardPaletteVisual,
}

// 同目录 UIX 生成卡片全部分组视觉、三级阴影表、根记录及稳定借用。
crate::uix_items!("src/ui/widgets/display/card/card.uix");

impl CardVisual {
    fn elevation(&self, elevation: u8) -> Option<&CardElevationVisual> {
        elevation
            .checked_sub(1)
            .and_then(|index| self.elevations.get(usize::from(index)))
    }
}

// 向 UIX 提供大圆角主题角色。
const fn card_large_radius() -> CardRadiusRole {
    CardRadiusRole::Large
}

// 向 UIX 提供默认盒阴影主题角色。
const fn card_default_shadow() -> CardShadowRole {
    CardShadowRole::Default
}

// 向 UIX 提供卡片容器背景角色。
const fn card_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}

// 向 UIX 提供卡片抬升背景角色。
const fn card_elevated_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}

// 向 UIX 提供卡片品牌主色角色。
const fn card_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

// 向 UIX 提供卡片次级边框角色。
const fn card_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}

// 向 UIX 提供卡片正文色角色。
const fn card_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

// 向 UIX 提供动作悬停填充角色。
const fn card_action_hover() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}

widget! {
    /// 支持阴影层级、悬停高亮和内容内边距的卡片组件。
    pub struct Card {
        title: Option<String>,
        children: WidgetChildren,
        bordered: bool,
        #[snapshot(skip)]
        bordered_authored: bool,
        hoverable: bool,
        hovered: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        padding: f32,
        #[snapshot(skip)]
        padding_authored: bool,
        elevation: u8,
        #[snapshot(skip)]
        elevation_authored: bool,
        flex_grow_val: f32,
        actions: Vec<String>,
        focused: bool,
        focused_action: usize,
        hovered_action: Cell<Option<usize>>,
        last_frame: Cell<Option<Rect>>,
        pending_submit: RefCell<Option<String>>,
        // 缓存 body 子树的真实内容尺寸，供未指定高度时撑开卡片。
        cached_content_size: Cell<Size>,
        #[snapshot(skip)]
        visual: &'static CardVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    flex_grow => (&self) -> f32 { self.flex_grow_val }

    tab_index => (&self) -> i32 { i32::from(!self.actions.is_empty()) }

    build => (&self) -> Vec<Box<dyn Widget>> {
        self.children.take()
    }

    on_children_changed => (&mut self, child_count: usize) {
        // 直接子树结构变化后旧内容尺寸失效，下一轮布局会写入新事实。
        self.cached_content_size.set(Size::zero());
        // 当前只需结构变化信号，不依赖变化后的子项数量。
        let _ = child_count;
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerEnter if self.hoverable => {
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered || self.hovered_action.get().is_some();
                self.hovered = false;
                self.hovered_action.set(None);
                if changed { EventResult::Handled } else { EventResult::NotHandled }
            }
            SystemEvent::PointerMove { pos, .. } if !self.actions.is_empty() => {
                let action = self.action_index_at(*pos);
                let changed = action != self.hovered_action.get();
                self.hovered_action.set(action);
                if changed { EventResult::Handled } else { EventResult::NotHandled }
            }
            SystemEvent::PointerDown { pos, button: MouseButton::Left, .. } => {
                if let Some(index) = self.action_index_at(*pos) {
                    self.focused = true;
                    self.focused_action = index;
                    self.submit_action(index);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn if !self.actions.is_empty() => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if !self.actions.is_empty() => match key {
                KeyCode::Left => {
                    self.focused_action = self.focused_action.saturating_sub(1);
                    EventResult::Handled
                }
                KeyCode::Right => {
                    self.focused_action = (self.focused_action + 1).min(self.actions.len() - 1);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.focused_action = 0;
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.focused_action = self.actions.len() - 1;
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space => {
                    self.submit_action(self.focused_action);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_submit
            .borrow_mut()
            .take()
            .map(|action| SemanticEvent::submit(id, action))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        if ctx.paint_pass() != PaintPass::Content {
            return;
        }
        let frame = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        self.last_frame.set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let border_radius_lg = self.visual.radius.resolve(ctx.tokens());
        let bg_container = self.visual.palette.background.resolve(ctx.tokens());
        let bg_elevated = self.visual.palette.elevated_background.resolve(ctx.tokens());
        let primary = self.visual.palette.primary.resolve(ctx.tokens());
        let border_secondary = self.visual.palette.border.resolve(ctx.tokens());
        let text = self.visual.palette.text.resolve(ctx.tokens());

        let card_radius = Some(Radius::uniform(
            border_radius_lg
                .min(frame.w * self.visual.surface.radius_limit_ratio)
                .min(frame.h * self.visual.surface.radius_limit_ratio)
                .max(0.0),
        ));

        // 阴影绘制由图形引擎内部处理 clip 绕过。
        // 引擎的 draw_box_shadow 会自动恢复到脏区域 clip，
        // 绕过父级 children_clip 但保持在脏区域内。
        draw_elevation_shadow(ctx, frame, self.elevation, self.visual);

        ctx.push_clip(frame);

        // Background
        let bg = if self.hovered {
            // 悬浮时向白色混合 5%（等价 Color::lighten(0.05)，基于主题底色 token）。
            bg_container.lighten(self.visual.surface.hover_lighten)
        } else {
            bg_elevated
        };
        ctx.fill_rect(frame, bg, card_radius);

        // Top accent line
        if self.elevation >= self.visual.surface.accent_min_elevation {
            let inset = self
                .visual
                .surface
                .accent_horizontal_inset
                .min(frame.w * self.visual.surface.radius_limit_ratio);
            let accent_w = (frame.w - inset * 2.0).max(0.0);
            let accent_h = self.visual.surface.accent_height.min(frame.h);
            if accent_w > 0.0 && accent_h > 0.0 {
                let accent_rect = Rect::new(frame.x + inset, frame.y, accent_w, accent_h);
                ctx.fill_rect(
                    accent_rect,
                    primary,
                    Some(Radius::uniform(
                        (accent_h * self.visual.surface.accent_radius_ratio)
                            .min(accent_w * self.visual.surface.accent_radius_ratio),
                    )),
                );
            }
        }

        // Border
        if self.bordered {
            ctx.stroke_rect(
                frame,
                border_secondary,
                self.visual.surface.border_width,
                card_radius,
            );
        }

        // Title
        if let Some(ref title) = self.title {
            if let Some(title_rect) = self.title_text_rect(frame) {
                if let Some((visible_title, font_size)) = fitted_text(
                    ctx,
                    title,
                    self.visual.title.font_size,
                    self.visual.title.min_font_size,
                    title_rect.w,
                    (title_rect.h - self.visual.title.vertical_inset * 2.0).max(0.0),
                ) {
                    let title_y = ctx.visual_center_y(title_rect, font_size);
                    ctx.push_clip(title_rect);
                    ctx.draw_text(
                        &visible_title,
                        Point::new(title_rect.x, title_y),
                        text,
                        font_size,
                    );
                    ctx.pop_clip();
                }
            }
            if let Some(separator) = self.title_separator_rect(frame) {
                ctx.fill_rect(separator, border_secondary, None);
            }
        }

        // Actions
        if let Some(action_rect) = self.action_rect(frame) {
            ctx.fill_rect(action_rect, bg_container, None);
            ctx.stroke_rect(
                action_rect,
                border_secondary,
                self.visual.surface.border_width,
                None,
            );
            let btn_w = action_rect.w / self.actions.len() as f32;
            for (i, action) in self.actions.iter().enumerate() {
                let btn_rect = Rect::new(
                    action_rect.x + i as f32 * btn_w,
                    action_rect.y,
                    btn_w,
                    action_rect.h,
                );
                if self.hovered_action.get() == Some(i) {
                    ctx.fill_rect(
                        btn_rect,
                        self.visual.palette.action_hover.resolve(ctx.tokens()),
                        None,
                    );
                }
                if self.focused && tree.keyboard_focus_visible() && self.focused_action == i {
                    let inset = self
                        .visual
                        .action
                        .focus_inset
                        .min(btn_rect.w * self.visual.surface.radius_limit_ratio)
                        .min(btn_rect.h * self.visual.surface.radius_limit_ratio);
                    ctx.stroke_rect(
                        Rect::new(
                            btn_rect.x + inset,
                            btn_rect.y + inset,
                            (btn_rect.w - inset * 2.0).max(0.0),
                            (btn_rect.h - inset * 2.0).max(0.0),
                        ),
                        primary,
                        self.visual.action.focus_stroke_width,
                        None,
                    );
                }
                if let Some((visible_action, font_size)) = fitted_text(
                    ctx,
                    action,
                    self.visual.action.font_size,
                    self.visual.action.min_font_size,
                    (btn_rect.w - self.visual.action.horizontal_inset * 2.0).max(0.0),
                    (btn_rect.h - self.visual.action.vertical_inset * 2.0).max(0.0),
                ) {
                    let ay = ctx.visual_center_y(btn_rect, font_size);
                    let text_w = ctx.measure_text(&visible_action, font_size).w;
                    ctx.push_clip(btn_rect);
                    ctx.draw_text(
                        &visible_action,
                        Point::new(
                            btn_rect.x
                                + (btn_w - text_w) * self.visual.action.center_ratio,
                            ay,
                        ),
                        primary,
                        font_size,
                    );
                    ctx.pop_clip();
                }
                if i < self.actions.len() - 1 {
                    let divider_h =
                        (action_rect.h - self.visual.action.divider_inset * 2.0).max(0.0);
                    ctx.fill_rect(
                        Rect::new(
                            btn_rect.x + btn_w - self.visual.action.divider_thickness,
                            action_rect.y
                                + self.visual.action.divider_inset.min(action_rect.h),
                            self.visual.action.divider_thickness,
                            divider_h,
                        ),
                        border_secondary,
                        None,
                    );
                }
            }
        }
        ctx.pop_clip();
    }

    // 扩展脏区域覆盖完整阴影渲染范围。
    // 引擎的 draw_box_shadow 会自动恢复到脏区域 clip（绕过父级），
    // 若 dirty_rect 不覆盖阴影边界，会产生像素叠加拖影。
    dirty_rect => (&self, frame: Rect) -> Rect {
        let expand = self
            .visual
            .elevation(self.elevation)
            .map_or(0.0, |visual| visual.dirty_expand);
        if expand > 0.0 {
            Rect::new(frame.x - expand, frame.y - expand, frame.w + expand * 2.0, frame.h + expand * 2.0)
        } else {
            frame
        }
    }

    measure_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let mut output = Vec::with_capacity(children.len());
        self.measure_children_reusing(frame, children, tree, &mut output);
        output
    }

    measure_children_into => (
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>
    ) {
        self.measure_children_reusing(frame, children, tree, output);
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut scratch = crate::ui::LayoutEngineScratch::default();
        let mut output = Vec::with_capacity(children.len());
        self.layout_children_reusing(frame, children, &mut scratch, &mut output);
        output
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[LayoutChild],
        _tree: &WidgetTree,
        scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>
    ) {
        self.layout_children_reusing(frame, children, scratch, output);
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        Some(self.body_rect(frame))
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
fn draw_elevation_shadow(ctx: &mut PaintContext, frame: Rect, elevation: u8, visual: &CardVisual) {
    let Some(elevation_visual) = visual.elevation(elevation) else {
        return;
    };
    let shadow = visual.shadow.resolve(ctx.tokens());
    let corner_radius = Some(Radius::uniform(visual.radius.resolve(ctx.tokens())));

    let boost =
        |c: Color| c.with_alpha((c.a as f32 * elevation_visual.alpha_boost).min(255.0) as u8);

    // ── Layer 1: Contact shadow ──
    // Directional (y-down), tight blur, sharp smoothstep falloff.
    let (ox1, oy1, bl1, col1) = shadow.layer_1;
    if bl1 > 0.0 && col1.a > 0 {
        ctx.draw_box_shadow(
            frame,
            bl1 * elevation_visual.directional_scale, // blur
            ox1,                                      // x-offset (0 = centered contact)
            oy1 * elevation_visual.directional_scale * visual.surface.shadow_directional_y_scale,
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
            bl2 * elevation_visual.ambient_scale, // larger blur = wider ambient
            ox2 * visual.surface.shadow_ambient_x_scale,
            oy2 * elevation_visual.ambient_scale * visual.surface.shadow_ambient_y_scale,
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
            bl3 * elevation_visual.glow_scale, // very wide blur
            visual.surface.shadow_glow_offset_x,
            visual.surface.shadow_glow_offset_y,
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

// 把卡片 Rust 内核与已有拥有型 View 子树融合为 UIX 声明的单一根节点。
fn build_card_view(
    mut kernel: Card,
    children: Vec<ViewNode>,
    visual: &'static CardVisual,
) -> ViewNode {
    if !kernel.bordered_authored {
        kernel.bordered = visual.defaults.bordered;
    }
    if !kernel.padding_authored {
        kernel.padding = visual.defaults.padding;
    }
    if !kernel.elevation_authored {
        kernel.elevation = visual.defaults.elevation;
    }
    kernel.elevation = kernel.elevation.min(visual.defaults.max_elevation);
    kernel.visual = visual;
    ViewNode::new(kernel, children)
}

impl View for Card {
    fn build(self) -> ViewNode {
        // 独立卡片同样经由组件自己的 UIX 根声明构建。
        self.build_view_with_children(Vec::new())
    }
}

impl Card {
    // 统一拥有型与树级复用入口，保持 Card 自然尺寸测量契约不分叉。
    fn measure_children_reusing(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>,
    ) {
        let inner = self.body_rect(frame);
        let max_height = if self.fixed_height.is_none() {
            f32::INFINITY
        } else {
            inner.h
        };
        let constraints = Constraints::loose(Size::new(inner.w, max_height));
        output.clear();
        output.extend(children.iter().map(|&id| {
            let mut child = child_from_tree_with_natural_constraints(id, tree, constraints);
            child.measured_size = constraints.clamp(child.measured_size);
            if tree.get(id).and_then(|node| node.as_layout()).is_none() {
                child.flex_shrink = 0.0;
            }
            child
        }));
    }

    // 布局树独占共享 Flex 缓冲；Card 只维护自身内容尺寸缓存。
    fn layout_children_reusing(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        output.clear();
        if children.is_empty() {
            self.cached_content_size.set(Size::zero());
            return;
        }

        let inner = self.body_rect(frame);
        if inner.w <= 0.0 || (inner.h <= 0.0 && self.fixed_height.is_some()) {
            output.reserve(children.len());
            output.extend(
                children
                    .iter()
                    .map(|child| (child.id, Rect::new(inner.x, inner.y, 0.0, 0.0))),
            );
            return;
        }

        scratch.flex_children.clear();
        scratch
            .flex_children
            .extend(children.iter().map(|child| FlexChild {
                flex_grow: child.flex_grow,
                flex_shrink: child.flex_shrink,
                align_self: child.align_self,
                measured_size: child.measured_size,
                margin: child.margin,
                ..FlexChild::default()
            }));
        let input = FlexInput {
            direction: FlexDirection::Column,
            gap: self.visual.defaults.body_gap,
            padding: EdgeInsets::zero(),
            container: inner,
            children: &scratch.flex_children,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Stretch,
            intrinsic_main: self.fixed_height.is_none(),
            ..FlexInput::default()
        };
        let _ = compute_flex_layout_into(&input, &mut scratch.flex);
        let positions = &scratch.flex.child_rects;
        self.cached_content_size
            .set(content_size_from_children(inner, positions, children));
        output.reserve(children.len());
        output.extend(
            children
                .iter()
                .zip(positions)
                .map(|(child, rect)| (child.id, *rect)),
        );
    }

    fn body_rect(&self, frame: Rect) -> Rect {
        // 标题和 actions 为固定区，body 只使用二者之间的剩余空间。
        let frame_w = frame.w.max(0.0);
        let frame_h = frame.h.max(0.0);
        let padding = self.padding.max(0.0);
        let title_offset = if self.title.is_some() {
            self.visual.title.block_height
        } else {
            padding
        }
        .min(frame_h);
        let action_top = self
            .action_rect(frame)
            .map(|rect| rect.y)
            .unwrap_or(frame.y + frame_h);
        let body_y = (frame.y + title_offset).min(action_top);
        let left_padding = padding.min(frame_w);
        let right_padding = padding.min((frame_w - left_padding).max(0.0));
        let bottom_padding = padding.min((action_top - body_y).max(0.0));
        Rect::new(
            frame.x + left_padding,
            body_y,
            (frame_w - left_padding - right_padding).max(0.0),
            (action_top - body_y - bottom_padding).max(0.0),
        )
    }

    fn padded_horizontal_rect(&self, frame: Rect, y: f32, height: f32) -> Rect {
        let frame_w = frame.w.max(0.0);
        let padding = self.padding.max(0.0);
        let left_padding = padding.min(frame_w);
        let right_padding = padding.min((frame_w - left_padding).max(0.0));
        Rect::new(
            frame.x + left_padding,
            y,
            (frame_w - left_padding - right_padding).max(0.0),
            height.max(0.0),
        )
    }

    // `max`/`min` intentionally collapse a NaN delta to zero; `clamp` would retain NaN.
    #[allow(clippy::manual_clamp)]
    fn title_available_height(&self, frame: Rect) -> f32 {
        let frame_h = frame.h.max(0.0);
        let action_top = self
            .action_rect(frame)
            .map(|rect| rect.y)
            .unwrap_or(frame.y + frame_h);
        (action_top - frame.y)
            .max(0.0)
            .min(self.visual.title.block_height)
    }

    fn title_text_rect(&self, frame: Rect) -> Option<Rect> {
        self.title.as_ref()?;
        let height = self
            .title_available_height(frame)
            .min(self.visual.title.text_height);
        (height > 0.0).then(|| self.padded_horizontal_rect(frame, frame.y, height))
    }

    fn title_separator_rect(&self, frame: Rect) -> Option<Rect> {
        self.title.as_ref()?;
        let available_height = self.title_available_height(frame);
        if available_height
            < self.visual.title.separator_offset + self.visual.title.separator_thickness
        {
            return None;
        }
        let rect = self.padded_horizontal_rect(
            frame,
            frame.y + self.visual.title.separator_offset,
            self.visual.title.separator_thickness,
        );
        (rect.w > 0.0).then_some(rect)
    }

    pub(crate) fn action_rect(&self, frame: Rect) -> Option<Rect> {
        if self.actions.is_empty() {
            return None;
        }
        let frame_h = frame.h.max(0.0);
        let action_h = self.visual.action.height.min(frame_h);
        Some(Rect::new(
            frame.x,
            frame.y + frame_h - action_h,
            frame.w.max(0.0),
            action_h,
        ))
    }

    fn intrinsic_size(&self) -> Size {
        // 读取上一轮由真实子树布局得到的 body 内容尺寸。
        let cached = self.cached_content_size.get();
        // 标题存在时占用固定标题区，否则 body 从顶部内边距后开始。
        let top = if self.title.is_some() {
            // 标题块高度包含标题文本与分隔线区域。
            self.visual.title.block_height
        } else {
            // 无标题卡片保留顶部内容内边距。
            self.padding
        };
        // 只有真实内容高度可用时才替换兼容的默认高度。
        let content_height = if cached.h > 0.0 {
            // 自然高度包含顶部区域、body、底部内边距和可选动作区。
            top + cached.h
                + self.padding
                + if self.actions.is_empty() {
                    // 无动作时不预留页尾操作区。
                    0.0
                } else {
                    // 有动作时把固定操作区完整计入 Card border-box。
                    self.visual.action.height
                }
        } else {
            // 首轮尚无子树缓存时沿用原有默认高度完成 bootstrap。
            self.visual.defaults.height
        };
        // 固有尺寸保留原有默认宽度，并只让未显式指定的高度由内容撑开。
        Size::new(
            self.fixed_width.unwrap_or(self.visual.defaults.width),
            self.fixed_height
                .unwrap_or(self.visual.defaults.height.max(content_height)),
        )
    }

    /// 创建带边框、一级阴影和默认内边距的卡片。
    pub fn new() -> Self {
        let visual = CARD_VISUAL_REF;
        Self {
            title: None,
            children: WidgetChildren::new(),
            bordered: visual.defaults.bordered,
            bordered_authored: false,
            hoverable: false,
            hovered: false,
            fixed_width: None,
            fixed_height: None,
            padding: visual.defaults.padding,
            padding_authored: false,
            elevation: visual.defaults.elevation,
            elevation_authored: false,
            flex_grow_val: 0.0,
            actions: Vec::new(),
            focused: false,
            focused_action: 0,
            hovered_action: Cell::new(None),
            last_frame: Cell::new(None),
            pending_submit: RefCell::new(None),
            // 新卡片在首次子树布局前没有可复用的内容尺寸事实。
            cached_content_size: Cell::new(Size::zero()),
            visual,
        }
    }

    /// 设置卡片标题。
    pub fn title(mut self, t: &str) -> Self {
        self.title = Some(t.to_string());
        self
    }
    /// 设置是否绘制卡片边框。
    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self.bordered_authored = true;
        self
    }
    /// 启用卡片悬停高亮。
    pub fn hoverable(mut self) -> Self {
        self.hoverable = true;
        self
    }
    /// 设置固定宽高；非有限值或非正值会恢复对应的默认尺寸。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Self::optional_dimension(w);
        self.fixed_height = Self::optional_dimension(h);
        self
    }
    /// 设置内容内边距；非有限值归零，负值截断为零。
    pub fn padding(mut self, p: f32) -> Self {
        self.padding = if p.is_finite() { p.max(0.0) } else { 0.0 };
        self.padding_authored = true;
        self
    }
    /// 设置阴影层级，最大为三级。
    pub fn elevation(mut self, e: u8) -> Self {
        self.elevation = e.min(CARD_VISUAL.defaults.max_elevation);
        self.elevation_authored = true;
        self
    }
    /// 设置弹性布局增长因子；非有限值归零，负值截断为零。
    pub fn flex_grow(mut self, v: f32) -> Self {
        self.flex_grow_val = if v.is_finite() { v.max(0.0) } else { 0.0 };
        self
    }
    // 把 View 声明的显式尺寸与 Flex 覆盖应用到 Card 私有布局状态。
    pub(crate) fn apply_view_layout_style(&mut self, style: &Style, flex_grow: Option<f32>) {
        // 只消费显式宽度，未声明轴继续由 Card 默认尺寸负责。
        if let Some(width) = style.width {
            // 复用 Card 的尺寸有效值规则。
            self.fixed_width = Self::optional_dimension(width);
        }
        // 只消费显式高度，未声明轴继续保持内容自适应。
        if let Some(height) = style.height {
            // 复用 Card 的尺寸有效值规则。
            self.fixed_height = Self::optional_dimension(height);
        }
        // 只有 ViewNode 明确覆盖时才替换组件构建器已有的增长因子。
        if let Some(grow) = flex_grow {
            // 非有限值与负值继续按 Card 公共构建器规则归零。
            self.flex_grow_val = if grow.is_finite() { grow.max(0.0) } else { 0.0 };
        }
    }
    /// 设置操作标签，并移除仅含空白的项目。
    pub fn actions(mut self, list: Vec<impl Into<String>>) -> Self {
        self.actions = list
            .into_iter()
            .map(Into::into)
            .filter(|action: &String| !action.trim().is_empty())
            .collect();
        self
    }
    /// 返回当前聚焦的操作索引；没有操作时返回 `None`。
    pub fn focused_action(&self) -> Option<usize> {
        (!self.actions.is_empty()).then_some(self.focused_action)
    }
    /// 返回卡片操作标签。
    pub fn action_labels(&self) -> &[String] {
        &self.actions
    }
    /// 在卡片末尾追加一个子组件。
    pub fn child(self, w: impl Widget + 'static) -> Self {
        self.children.add(w);
        self
    }
    /// 替换卡片中的全部子组件。
    pub fn children(self, widgets: Vec<Box<dyn Widget>>) -> Self {
        self.children.set_all(widgets);
        self
    }

    /// 经由同目录 UIX 根声明构建卡片及其拥有型 View 子树。
    #[doc(hidden)]
    pub fn build_view_with_children(self, children: Vec<ViewNode>) -> ViewNode {
        // UIX 拥有公开根；Rust 内核继续独占布局、交互、几何与绘制机制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/card/card.uix")
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
            focused_action: self.focused_action(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.title = next.title;
        self.bordered = next.bordered;
        self.bordered_authored = next.bordered_authored;
        self.hoverable = next.hoverable;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.padding = next.padding;
        self.padding_authored = next.padding_authored;
        self.elevation = next.elevation;
        self.elevation_authored = next.elevation_authored;
        self.flex_grow_val = next.flex_grow_val;
        self.visual = next.visual;
        self.actions = next.actions;
        self.focused_action = self
            .focused_action
            .min(self.actions.len().saturating_sub(1));
        self.hovered_action.set(
            self.hovered_action
                .get()
                .filter(|index| *index < self.actions.len()),
        );
        let pending_is_valid = self
            .pending_submit
            .borrow()
            .as_ref()
            .is_none_or(|pending| self.actions.contains(pending));
        if !pending_is_valid {
            self.pending_submit.borrow_mut().take();
        }
        if self.actions.is_empty() {
            self.focused = false;
        }
    }

    // 测试目标观察 UIX 声明的关键视觉契约，不暴露到公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, f32, f32, f32) {
        (
            self.visual.defaults.width,
            self.visual.defaults.height,
            self.visual.defaults.padding,
            self.visual.title.block_height,
            self.visual.title.font_size,
            self.visual.action.height,
            self.visual.action.font_size,
            self.visual.surface.hover_lighten,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }

    fn optional_dimension(value: f32) -> Option<f32> {
        (value.is_finite() && value > 0.0).then_some(value)
    }

    fn action_index_at(&self, pos: Point) -> Option<usize> {
        let frame = self.last_frame.get()?;
        let action_rect = self.action_rect(frame)?;
        if !action_rect.contains(pos) || action_rect.w <= 0.0 {
            return None;
        }
        let width = action_rect.w / self.actions.len() as f32;
        let index = ((pos.x - action_rect.x) / width) as usize;
        Some(index.min(self.actions.len() - 1))
    }

    fn submit_action(&self, index: usize) {
        if let Some(action) = self.actions.get(index) {
            self.pending_submit.replace(Some(action.clone()));
        }
    }

    // 测试目标保留卡片 frame 注入入口，供交互几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn set_frame_for_test(&self, frame: Rect) {
        self.last_frame.set(Some(Rect::new(
            0.0,
            0.0,
            frame.w.max(0.0),
            frame.h.max(0.0),
        )));
    }
}

fn fitted_text<'a>(
    ctx: &mut PaintContext,
    text: &'a str,
    base_size: f32,
    min_size: f32,
    max_width: f32,
    max_height: f32,
) -> Option<(Cow<'a, str>, f32)> {
    if !base_size.is_finite()
        || base_size <= 0.0
        || !min_size.is_finite()
        || min_size <= 0.0
        || !max_width.is_finite()
        || max_width <= 0.0
        || !max_height.is_finite()
        || max_height <= 0.0
    {
        return None;
    }
    let visible = if text.contains(['\r', '\n']) {
        Cow::Owned(text.replace(['\r', '\n'], " "))
    } else {
        Cow::Borrowed(text)
    };
    let base_height = conservative_text_height(ctx, &visible, base_size);
    let height_scale = if base_height > 0.0 {
        (max_height / base_height).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let font_size = (base_size * height_scale).min(base_size);
    let minimum = min_size.min(base_size);
    let font_size = if font_size >= minimum {
        font_size
    } else if conservative_text_height(ctx, &visible, minimum) <= max_height {
        minimum
    } else {
        return None;
    };
    // 复用已规范化窄入口，避免对同一标题或操作文案再次替换并分配。
    let visible = match visible {
        Cow::Borrowed(value) => {
            ctx.elide_normalized_single_line_cow(value, font_size, max_width)?
        }
        Cow::Owned(value) => {
            match ctx.elide_normalized_single_line_cow(&value, font_size, max_width)? {
                // 规范化字符串完整容纳时直接把其现有分配移入结果。
                Cow::Borrowed(_) => Cow::Owned(value),
                // 截断结果已经拥有唯一所需分配，丢弃较长的规范化缓冲。
                Cow::Owned(truncated) => Cow::Owned(truncated),
            }
        }
    };
    Some((visible, font_size))
}

fn conservative_text_height(ctx: &mut PaintContext, text: &str, font_size: f32) -> f32 {
    ctx.measure_text(text, font_size)
        .h
        .max(ctx.line_box_height(font_size))
}
