//! FloatButton widget — 浮动按钮，Ant Design 风格。
//!
//! 固定在屏幕角落的操作按钮，支持图标、description、tooltip、badge 与窗口 placement。

// 声明 FloatButton 私有几何实现。
mod geometry;
// 声明 FloatButtonBackTop 便捷封装。
mod back_top;
// 声明 FloatButtonGroup 独立组件。
mod group;
// 公开回到顶部便捷封装。
pub use back_top::FloatButtonBackTop;
// 公开保留子 View 事件所有权的浮动按钮组包装器。
pub use group::{FloatButtonGroup, FloatButtonGroupView};

// 引入单一几何解析入口与输入输出类型。
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
use geometry::{FloatButtonGeometry, FloatButtonGeometryInput, resolve_float_button_geometry};
// 浮动按钮使用基础层共享的触发方式，不依赖反馈组件族。
use crate::ui::SnapshotFields;
use crate::ui::widgets::TriggerMode;
use crate::ui::{
    EventResult, KeyCode, MouseButton, OverlayEntry, OverlayKind, Placement, SystemEvent,
    ThemeTokens, View, ViewNode, WidgetTree,
};
use std::cell::Cell;

// 保存 FloatButton 的窗口锚定、说明、提示、徽标与损伤几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FloatButtonGeometryVisual {
    pub(crate) default_size: f32,
    pub(crate) surface_inset: f32,
    pub(crate) description_gap: f32,
    pub(crate) description_trailing_padding: f32,
    pub(crate) average_character_width: f32,
    pub(crate) tooltip_horizontal_padding: f32,
    pub(crate) tooltip_min_width: f32,
    pub(crate) tooltip_height: f32,
    pub(crate) tooltip_gap: f32,
    pub(crate) badge_count_size: f32,
    pub(crate) badge_dot_size: f32,
    pub(crate) shadow_left_outset: f32,
    pub(crate) shadow_top_outset: f32,
    pub(crate) shadow_width_extra: f32,
    pub(crate) shadow_height_extra: f32,
}

// 保存 FloatButton 自身的绘制参数与浮层层级。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FloatButtonPaintVisual {
    radius_factor: f32,
    shadow_blur: f32,
    shadow_offset_x: f32,
    shadow_offset_y: f32,
    shadow_alpha: u8,
    focus_stroke_width: f32,
    icon_size: f32,
    badge_font_size: f32,
    tooltip_border_width: f32,
    badge_overflow_threshold: i32,
    overlay_z_index: i32,
}

// 保存 FloatButton 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FloatButtonPaletteVisual {
    primary: ColorValue,
    primary_hover: ColorValue,
    primary_active: ColorValue,
    foreground: ColorValue,
    tooltip_text: ColorValue,
    shadow: ColorValue,
    focus_border: ColorValue,
    badge: ColorValue,
    tooltip_background: ColorValue,
    tooltip_border: ColorValue,
}

// 保存 FloatButton 使用的主题字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatButtonFontRole {
    Small,
}

impl FloatButtonFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.font_size_sm(),
        }
    }
}

// 保存 FloatButton 使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatButtonRadiusRole {
    Small,
}

impl FloatButtonRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 全部 FloatButton 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FloatButtonVisual {
    pub(crate) geometry: FloatButtonGeometryVisual,
    paint: FloatButtonPaintVisual,
    palette: FloatButtonPaletteVisual,
    description_font: FloatButtonFontRole,
    tooltip_font: FloatButtonFontRole,
    tooltip_radius: FloatButtonRadiusRole,
    default_icon: &'static str,
}

crate::uix_items!("src/ui/widgets/general/float_button/float_button.uix");

// 保存一次绘制解析后的主题视觉值，避免重复查询同一 token。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedFloatButtonVisual {
    primary: Color,
    primary_hover: Color,
    primary_active: Color,
    foreground: Color,
    tooltip_text: Color,
    shadow: Color,
    focus_border: Color,
    badge: Color,
    tooltip_background: Color,
    tooltip_border: Color,
    description_font_size: f32,
    tooltip_font_size: f32,
    tooltip_radius: f32,
}

impl FloatButtonVisual {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedFloatButtonVisual {
        ResolvedFloatButtonVisual {
            primary: self.palette.primary.resolve(tokens),
            primary_hover: self.palette.primary_hover.resolve(tokens),
            primary_active: self.palette.primary_active.resolve(tokens),
            foreground: self.palette.foreground.resolve(tokens),
            tooltip_text: self.palette.tooltip_text.resolve(tokens),
            shadow: self.palette.shadow.resolve(tokens),
            focus_border: self.palette.focus_border.resolve(tokens),
            badge: self.palette.badge.resolve(tokens),
            tooltip_background: self.palette.tooltip_background.resolve(tokens),
            tooltip_border: self.palette.tooltip_border.resolve(tokens),
            description_font_size: self.description_font.resolve(tokens),
            tooltip_font_size: self.tooltip_font.resolve(tokens),
            tooltip_radius: self.tooltip_radius.resolve(tokens),
        }
    }
}

pub(crate) const fn float_button_small_font() -> FloatButtonFontRole {
    FloatButtonFontRole::Small
}

pub(crate) const fn float_button_small_radius() -> FloatButtonRadiusRole {
    FloatButtonRadiusRole::Small
}

pub(crate) const fn float_button_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

pub(crate) const fn float_button_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}

pub(crate) const fn float_button_primary_active() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryActive)
}

pub(crate) const fn float_button_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}

pub(crate) const fn float_button_black() -> ColorValue {
    ColorValue::Palette(PaletteColor::Black)
}

pub(crate) const fn float_button_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}

pub(crate) const fn float_button_primary_border() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBorder)
}

pub(crate) const fn float_button_error() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}

pub(crate) const fn float_button_bg_elevated() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}

pub(crate) const fn float_button_border_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}

// FloatButton — 浮动操作按钮。
widget! {
    /// 锚定窗口逻辑表面并支持图标、说明、提示与徽标的浮动操作按钮。
    pub struct FloatButton {
        icon: String,
        // 保存按钮展开说明文字。
        description: String,
        tooltip: String,
        badge_count: i32,
        // 保存圆点徽标语义。
        badge_dot: bool,
        size: f32,
        x: f32,
        y: f32,
        // 保存作者显式声明的窗口放置方向。
        placement: Option<Placement>,
        reserve_layout_space: bool,
        trigger_mode: TriggerMode,
        hovered: bool,
        pressed: bool,
        focused: bool,
        in_group: bool,
        // 表面缓存是派生几何输入，不属于 authored config 快照。
        #[snapshot(skip)]
        last_surface: Cell<Rect>,
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static FloatButtonVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    tab_index => (&self) -> i32 { 1 }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        // 命中区域与绘制、浮层登记消费同一几何结果。
        self.geometry(frame).control
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerEnter => {
                self.hovered = true;
                self.pointer_boundary_result()
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.pressed = false;
                self.pointer_boundary_result()
            }
            SystemEvent::PointerDown { button: MouseButton::Left, .. } => {
                self.pressed = true;
                EventResult::Handled
            }
            SystemEvent::PointerUp { button: MouseButton::Left, .. } => {
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Enter | KeyCode::Space, .. } => {
                self.pressed = true;
                EventResult::Handled
            }
            SystemEvent::KeyUp { key: KeyCode::Enter | KeyCode::Space, .. } => {
                self.pressed = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 损伤区域与绘制、命中消费同一几何结果。
        self.geometry(frame).paint_bounds
    }

    overlay_entry => (&self, id: crate::ui::WidgetId, frame: Rect) -> Option<OverlayEntry> {
        // 旧入口仅作为当前缓存表面的兼容委托。
        self.overlay_for_surface(id, frame, self.last_surface.get())
    }

    // 显式接收当前窗口逻辑表面，避免复用旧尺寸下的锚点。
    overlay_entry_for_surface => (&self, id: crate::ui::WidgetId, frame: Rect, surface: Rect) -> Option<OverlayEntry> {
        // 保存组件树本帧提供的权威表面。
        self.last_surface.set(surface);
        // 使用同一表面解析浮层登记区域。
        self.overlay_for_surface(id, frame, surface)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 读取当前绘制表面的逻辑尺寸。
        let surface_size = ctx.logical_surface_size();
        // 把逻辑尺寸映射为窗口客户区矩形。
        let surface = Rect::new(0.0, 0.0, surface_size.w, surface_size.h);
        // 保存绘制阶段使用的当前表面。
        self.last_surface.set(surface);
        // 一次解析本帧全部 FloatButton 几何。
        let geometry = self.geometry_for_surface(frame, surface);
        let loc = crate::ui::widget_runtime::locale::use_locale();
        let resolved = self.visual.resolve(ctx.tokens());
        let bg = if self.pressed {
            resolved.primary_active
        } else if self.hovered {
            resolved.primary_hover
        } else {
            resolved.primary
        };
        // 以最终控件高度派生圆角，说明模式保持胶囊形状。
        let r = Radius::uniform(geometry.control.h * self.visual.paint.radius_factor);
        // 借用共享几何中的完整控件区域。
        let btn_rect = geometry.control;
        // 阴影
        // 阴影：黑色 token + 原 alpha（保持视觉等价，色相随主题可换）。
        ctx.draw_box_shadow(
            btn_rect,
            self.visual.paint.shadow_blur,
            self.visual.paint.shadow_offset_x,
            self.visual.paint.shadow_offset_y,
            resolved.shadow.with_alpha(self.visual.paint.shadow_alpha),
            Some(r),
        );
        ctx.fill_rect(btn_rect, bg, Some(r));
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                btn_rect,
                resolved.focus_border,
                self.visual.paint.focus_stroke_width,
                Some(r),
            );
        }
        crate::ui::widgets::general::icon::Icon::paint_in_frame(
            // 传入绘制上下文。
            ctx,
            // 传入 Lucide 图标名称。
            &self.icon,
            // 只在共享图标区域内绘制。
            geometry.icon,
            // 浮动主按钮使用高对比前景色。
            resolved.foreground,
            // 沿用既有图标字号。
            self.visual.paint.icon_size,
        );
        // 展开说明存在时绘制到共享说明区域。
        if let Some(description) = geometry.description {
            // 说明文字与图标共享主按钮前景色。
            ctx.text_center(
                &self.description,
                description,
                resolved.foreground,
                resolved.description_font_size,
            );
        }
        // Badge
        if let Some(badge_rect) = geometry.badge {
            // 计算共享徽标区域的中心点。
            let badge_center = Point::new(
                // 计算横向中心。
                badge_rect.x + badge_rect.w * 0.5,
                // 计算纵向中心。
                badge_rect.y + badge_rect.h * 0.5,
            );
            // 绘制数字或圆点共用的错误色底。
            ctx.fill_circle(
                // 使用共享中心点。
                badge_center.x,
                // 使用共享中心点。
                badge_center.y,
                // 半径由最终徽标区域派生。
                badge_rect.w * 0.5,
                // 使用主题错误色。
                resolved.badge,
            );
            // 圆点徽标不绘制数字。
            if !self.badge_dot && self.badge_count > 0 {
                // 构造本地化溢出前的数字文本。
            let badge_count = self.badge_count.to_string();
                // 超过上限时使用本地化溢出文案。
            let badge = if self.badge_count > self.visual.paint.badge_overflow_threshold {
                loc.float_badge_overflow
            } else {
                &badge_count
            };
                // 把数字居中绘制到同一徽标区域。
                ctx.text_center(
                    badge,
                    badge_rect,
                    resolved.foreground,
                    self.visual.paint.badge_font_size,
                );
            }
        }
        let show_tooltip = match self.trigger_mode {
            TriggerMode::Hover => self.hovered,
            TriggerMode::Focus => self.focused,
            TriggerMode::Click | TriggerMode::ContextMenu => self.pressed,
        };
        // 只有触发状态满足且共享几何包含提示框时才绘制。
        if show_tooltip && let Some(tip) = geometry.tooltip {
            let tip_radius = Some(Radius::uniform(resolved.tooltip_radius));
            ctx.fill_rect(tip, resolved.tooltip_background, tip_radius);
            ctx.stroke_rect(
                tip,
                resolved.tooltip_border,
                self.visual.paint.tooltip_border_width,
                tip_radius,
            );
            ctx.text_center(
                &self.tooltip,
                tip,
                resolved.tooltip_text,
                resolved.tooltip_font_size,
            );
        }
    }
}

impl FloatButton {
    /// 创建使用指定 Lucide 名称的浮动图标按钮。
    pub fn new(icon: &str) -> Self {
        Self {
            icon: icon.to_string(),
            // 默认不显示展开说明。
            description: String::new(),
            tooltip: String::new(),
            badge_count: 0,
            // 默认不显示圆点徽标。
            badge_dot: false,
            size: FLOAT_BUTTON_VISUAL_REF.geometry.default_size,
            x: 0.0,
            y: 0.0,
            // 未显式 placement 时保留既有 frame-relative 行为。
            placement: None,
            reserve_layout_space: false,
            trigger_mode: TriggerMode::Hover,
            hovered: false,
            pressed: false,
            focused: false,
            in_group: false,
            // 新组件尚未获得窗口逻辑表面。
            last_surface: Cell::new(Rect::zero()),
            visual: FLOAT_BUTTON_VISUAL_REF,
        }
    }

    /// 设置相对当前 frame 或显式 placement 锚点的作者偏移。
    pub fn position(mut self, x: f32, y: f32) -> Self {
        // 非有限横向偏移回退为零。
        self.x = finite_or_zero(x);
        // 非有限纵向偏移回退为零。
        self.y = finite_or_zero(y);
        // 返回更新后的构建值。
        self
    }

    /// 设置相对窗口逻辑客户区的放置方向。
    pub fn placement(mut self, placement: Placement) -> Self {
        // 保存 overlay 模块拥有的公开放置语义。
        self.placement = Some(placement);
        // 返回更新后的构建值。
        self
    }

    /// 设置按钮内部展开显示的说明文字。
    pub fn description(mut self, description: impl Into<String>) -> Self {
        // 保存作者说明文字。
        self.description = description.into();
        // 返回更新后的构建值。
        self
    }

    /// 设置按当前触发方式显示的提示文字。
    pub fn tooltip(mut self, t: &str) -> Self {
        self.tooltip = t.to_string();
        self
    }
    /// 设置非负数字徽标计数；负数归一化为零。
    pub fn badge(mut self, count: i32) -> Self {
        self.badge_count = count.max(0);
        self
    }

    /// 设置圆点徽标；启用时视觉上优先于数字徽标。
    pub fn badge_dot(mut self, dot: bool) -> Self {
        // 保存作者圆点徽标配置。
        self.badge_dot = dot;
        // 返回更新后的构建值。
        self
    }

    /// 设置按钮直径；非正数或非有限值回退为 40 像素。
    pub fn size(mut self, s: f32) -> Self {
        self.size = positive_or(s, self.visual.geometry.default_size);
        self
    }

    /// 为按钮锚点保留与直径相同的布局空间；默认浮动模式仍保持零占位。
    pub fn reserve_layout_space(mut self, reserve: bool) -> Self {
        self.reserve_layout_space = reserve;
        self
    }

    /// 设置提示或按钮组展开状态采用的交互触发方式。
    pub fn trigger(mut self, trigger: TriggerMode) -> Self {
        self.trigger_mode = trigger;
        self
    }

    fn intrinsic_size(&self) -> Size {
        if self.reserve_layout_space {
            Size::new(self.size, self.size)
        } else {
            Size::zero()
        }
    }

    // 使用最近一次权威 surface 解析共享几何。
    fn geometry(&self, frame: Rect) -> FloatButtonGeometry {
        // 委托显式表面入口。
        self.geometry_for_surface(frame, self.last_surface.get())
    }

    // 使用调用方提供的当前 surface 解析共享几何。
    fn geometry_for_surface(&self, frame: Rect, surface: Rect) -> FloatButtonGeometry {
        // 构造只读 authored config 输入。
        resolve_float_button_geometry(FloatButtonGeometryInput {
            // 传入组件布局矩形。
            frame,
            // 传入当前窗口逻辑表面。
            surface,
            // 传入可选窗口 placement。
            placement: self.placement,
            // 传入有限作者偏移。
            offset: Point::new(self.x, self.y),
            // 传入按钮直径。
            size: self.size,
            // 借用展开说明。
            description: &self.description,
            // 借用提示文字。
            tooltip: &self.tooltip,
            // 传入数字徽标。
            badge_count: self.badge_count,
            // 传入圆点徽标。
            badge_dot: self.badge_dot,
            // 传入组内布局标记。
            in_group: self.in_group,
            // 传入普通布局占位标记。
            reserve_layout_space: self.reserve_layout_space,
            // 传入 UIX 拥有的全部几何参数。
            visual: self.visual.geometry,
        })
    }

    // 使用共享几何创建 FloatButton 的非模态浮层登记。
    fn overlay_for_surface(
        // 借用当前组件。
        &self,
        // 接收组件树稳定标识。
        id: crate::ui::WidgetId,
        // 接收组件布局矩形。
        frame: Rect,
        // 接收当前窗口逻辑表面。
        surface: Rect,
        // 返回可选浮层登记。
    ) -> Option<OverlayEntry> {
        // 普通布局占位模式不进入 OverlayStack。
        if self.reserve_layout_space {
            // 返回缺省登记。
            return None;
        }
        // 一次解析浮层命中所需的共享几何。
        let geometry = self.geometry_for_surface(frame, surface);
        // 返回非模态自定义浮层登记。
        Some(
            // 创建当前组件拥有的 Custom entry。
            OverlayEntry::new(id, OverlayKind::Custom)
                // 命中 bounds 使用完整 description 控件区域。
                .bounds(geometry.control)
                // 保持既有浮动按钮层级。
                .z_index(self.visual.paint.overlay_z_index),
        )
    }

    fn pointer_boundary_result(&self) -> EventResult {
        if self.in_group {
            EventResult::Bubbled
        } else {
            EventResult::Handled
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::FloatButton {
            icon: self.icon.clone(),
            // 保存展开说明 authored config。
            description: self.description.clone(),
            tooltip: self.tooltip.clone(),
            badge_count: self.badge_count,
            // 保存圆点徽标 authored config。
            badge_dot: self.badge_dot,
            size: self.size,
            x: self.x,
            y: self.y,
            // 保存显式窗口 placement。
            placement: self.placement,
            reserve_layout_space: self.reserve_layout_space,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.icon = next.icon;
        // 同步展开说明 authored config。
        self.description = next.description;
        self.tooltip = next.tooltip;
        self.badge_count = next.badge_count;
        // 同步圆点徽标 authored config。
        self.badge_dot = next.badge_dot;
        self.size = next.size;
        self.x = next.x;
        self.y = next.y;
        // 同步显式窗口 placement。
        self.placement = next.placement;
        self.reserve_layout_space = next.reserve_layout_space;
        self.trigger_mode = next.trigger_mode;
        self.in_group = next.in_group;
        self.visual = next.visual;
    }
}

impl Default for FloatButton {
    fn default() -> Self {
        Self::new(FLOAT_BUTTON_VISUAL_REF.default_icon)
    }
}

// 把 FloatButton Rust 内核与 UIX 静态视觉组合为单一叶节点。
fn build_float_button_view(
    mut kernel: FloatButton,
    visual: &'static FloatButtonVisual,
) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for FloatButton {
    fn build(self) -> ViewNode {
        build_float_button_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_float_button_uix_root(kernel: FloatButton) -> ViewNode {
    crate::uix!("src/ui/widgets/general/float_button/float_button.uix")
}

fn positive_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}
