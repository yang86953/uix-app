//! Tag widget — 彩色标签/徽标，支持关闭与勾选。

use std::cell::Cell;
use std::sync::OnceLock;

use crate::core::{Constraints, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::SnapshotFields;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{
    EventResult, KeyCode, MouseButton, PrimaryHue, SemanticEvent, SystemEvent, ThemeTokens,
    WidgetId,
};
use crate::widget;

const DEFAULT_TAG_FONT_SIZE: f32 = 12.0;

fn normalized_tag_font_size(size: f32, fallback: f32) -> f32 {
    if size.is_finite() && size > 0.0 {
        size
    } else {
        fallback
    }
}

/// 预设标签类型。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TagColor {
    /// 使用主题中性填充与正文颜色。
    Default,
    /// 使用主题成功状态色对。
    Success,
    /// 使用主题信息状态色对。
    Info,
    /// 使用主题警告状态色对。
    Warning,
    /// 使用主题错误状态色对。
    Error,
    /// 使用当前主题品牌主色对。
    Blue,
    /// 使用青色调色板的低强调色对。
    Cyan,
    /// 使用极客蓝调色板的低强调色对。
    Geekblue,
    /// 使用紫色调色板的低强调色对。
    Purple,
    /// 使用品红调色板的低强调色对。
    Magenta,
    /// 使用红色调色板的低强调色对。
    Red,
    /// 使用橙色调色板的低强调色对。
    Orange,
    /// 使用主题警告色对呈现金色标签。
    Gold,
    /// 使用青柠调色板的低强调色对。
    Lime,
    /// 使用主题成功色对呈现绿色标签。
    Green,
}

// 保存由 UIX 声明、由 Rust 测量与几何算法消费的标签视觉常量。
#[derive(Debug, Clone, Copy, PartialEq)]
struct TagLayoutVisual {
    default_font_size: f32,
    horizontal_padding: f32,
    vertical_padding: f32,
    compact_padding_ratio: f32,
    close_width: f32,
    check_width: f32,
    corner_radius_limit: f32,
    checked_inset: f32,
    checked_stroke_width: f32,
    focus_inset: f32,
    focus_stroke_width: f32,
    check_icon_size: f32,
    leading_icon_scale: f32,
    leading_icon_reserve_scale: f32,
    icon_gap: f32,
    close_icon_size: f32,
    light_hover_alpha: u8,
    light_pressed_alpha: u8,
    dark_hover_alpha: u8,
    dark_pressed_alpha: u8,
}

// 标签预设色可以来自主题 token 对，也可以来自扩展色阶。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagColorPair {
    Tokens {
        background: ColorValue,
        foreground: ColorValue,
    },
    Hue(PrimaryHue),
}

impl TagColorPair {
    fn resolve(self, tokens: &dyn ThemeTokens) -> (Color, Color) {
        match self {
            Self::Tokens {
                background,
                foreground,
            } => (background.resolve(tokens), foreground.resolve(tokens)),
            Self::Hue(hue) => hue.palette().subtle_pair(tokens.is_dark()),
        }
    }
}

// 保存全部公开 TagColor 变体对应的 UIX 视觉色对。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TagPaletteVisual {
    pairs: [TagColorPair; 15],
}

impl TagPaletteVisual {
    fn pair(&self, color: TagColor) -> TagColorPair {
        self.pairs[match color {
            TagColor::Default => 0,
            TagColor::Success => 1,
            TagColor::Info => 2,
            TagColor::Warning => 3,
            TagColor::Error => 4,
            TagColor::Blue => 5,
            TagColor::Cyan => 6,
            TagColor::Geekblue => 7,
            TagColor::Purple => 8,
            TagColor::Magenta => 9,
            TagColor::Red => 10,
            TagColor::Orange => 11,
            TagColor::Gold => 12,
            TagColor::Lime => 13,
            TagColor::Green => 14,
        }]
    }
}

// 按 UIX 声明的预设色表解析当前主题下的背景与前景。
fn resolve_tag_colors(
    color: TagColor,
    palette: &TagPaletteVisual,
    tokens: &dyn ThemeTokens,
) -> (Color, Color) {
    palette.pair(color).resolve(tokens)
}

// 标签圆角使用的主题令牌角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagRadiusRole {
    Small,
}

impl TagRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 完整视觉配置由全部 Tag 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
struct TagVisual {
    layout: TagLayoutVisual,
    palette: TagPaletteVisual,
    custom_light_text: ColorValue,
    custom_dark_text: ColorValue,
    light_overlay: ColorValue,
    dark_overlay: ColorValue,
    focus_color: ColorValue,
    corner_radius: TagRadiusRole,
    check_icon: &'static str,
    close_icon: &'static str,
}

// 组合 UIX 声明的标签尺寸、交互描边与图标比例。
#[allow(clippy::too_many_arguments)]
const fn tag_layout(
    default_font_size: f32,
    horizontal_padding: f32,
    vertical_padding: f32,
    compact_padding_ratio: f32,
    close_width: f32,
    check_width: f32,
    corner_radius_limit: f32,
    checked_inset: f32,
    checked_stroke_width: f32,
    focus_inset: f32,
    focus_stroke_width: f32,
    check_icon_size: f32,
    leading_icon_scale: f32,
    leading_icon_reserve_scale: f32,
    icon_gap: f32,
    close_icon_size: f32,
    light_hover_alpha: f32,
    light_pressed_alpha: f32,
    dark_hover_alpha: f32,
    dark_pressed_alpha: f32,
) -> TagLayoutVisual {
    TagLayoutVisual {
        default_font_size,
        horizontal_padding,
        vertical_padding,
        compact_padding_ratio,
        close_width,
        check_width,
        corner_radius_limit,
        checked_inset,
        checked_stroke_width,
        focus_inset,
        focus_stroke_width,
        check_icon_size,
        leading_icon_scale,
        leading_icon_reserve_scale,
        icon_gap,
        close_icon_size,
        light_hover_alpha: light_hover_alpha as u8,
        light_pressed_alpha: light_pressed_alpha as u8,
        dark_hover_alpha: dark_hover_alpha as u8,
        dark_pressed_alpha: dark_pressed_alpha as u8,
    }
}

// 组合 UIX 声明的主题 token 色对。
const fn tag_token_pair(background: ColorValue, foreground: ColorValue) -> TagColorPair {
    TagColorPair::Tokens {
        background,
        foreground,
    }
}

// 组合 UIX 声明的扩展色阶对。
const fn tag_hue_pair(hue: PrimaryHue) -> TagColorPair {
    TagColorPair::Hue(hue)
}

// 按公开 TagColor 顺序组合全部预设色对。
#[allow(clippy::too_many_arguments)]
const fn tag_palette(
    default: TagColorPair,
    success: TagColorPair,
    info: TagColorPair,
    warning: TagColorPair,
    error: TagColorPair,
    blue: TagColorPair,
    cyan: TagColorPair,
    geekblue: TagColorPair,
    purple: TagColorPair,
    magenta: TagColorPair,
    red: TagColorPair,
    orange: TagColorPair,
    gold: TagColorPair,
    lime: TagColorPair,
    green: TagColorPair,
) -> TagPaletteVisual {
    TagPaletteVisual {
        pairs: [
            default, success, info, warning, error, blue, cyan, geekblue, purple, magenta, red,
            orange, gold, lime, green,
        ],
    }
}

// 组合 UIX 声明的完整标签视觉配置。
#[allow(clippy::too_many_arguments)]
const fn tag_visual(
    layout: TagLayoutVisual,
    palette: TagPaletteVisual,
    custom_light_text: ColorValue,
    custom_dark_text: ColorValue,
    light_overlay: ColorValue,
    dark_overlay: ColorValue,
    focus_color: ColorValue,
    corner_radius: TagRadiusRole,
    check_icon: &'static str,
    close_icon: &'static str,
) -> TagVisual {
    TagVisual {
        layout,
        palette,
        custom_light_text,
        custom_dark_text,
        light_overlay,
        dark_overlay,
        focus_color,
        corner_radius,
        check_icon,
        close_icon,
    }
}

// 向 UIX 提供标签默认填充主题角色。
const fn tag_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}

// 向 UIX 提供标签正文主题角色。
const fn tag_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

// 向 UIX 提供成功背景主题角色。
const fn tag_success_bg() -> ColorValue {
    ColorValue::Palette(PaletteColor::SuccessBg)
}

// 向 UIX 提供成功前景主题角色。
const fn tag_success() -> ColorValue {
    ColorValue::Palette(PaletteColor::Success)
}

// 向 UIX 提供信息背景主题角色。
const fn tag_info_bg() -> ColorValue {
    ColorValue::Palette(PaletteColor::InfoBg)
}

// 向 UIX 提供信息前景主题角色。
const fn tag_info() -> ColorValue {
    ColorValue::Palette(PaletteColor::Info)
}

// 向 UIX 提供警告背景主题角色。
const fn tag_warning_bg() -> ColorValue {
    ColorValue::Palette(PaletteColor::WarningBg)
}

// 向 UIX 提供警告前景主题角色。
const fn tag_warning() -> ColorValue {
    ColorValue::Palette(PaletteColor::Warning)
}

// 向 UIX 提供错误背景主题角色。
const fn tag_error_bg() -> ColorValue {
    ColorValue::Palette(PaletteColor::ErrorBg)
}

// 向 UIX 提供错误前景主题角色。
const fn tag_error() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}

// 向 UIX 提供品牌弱背景主题角色。
const fn tag_primary_bg() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBg)
}

// 向 UIX 提供品牌主色主题角色。
const fn tag_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

// 向 UIX 提供黑色主题角色。
const fn tag_black() -> ColorValue {
    ColorValue::Palette(PaletteColor::Black)
}

// 向 UIX 提供白色主题角色。
const fn tag_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}

// 向 UIX 提供青色扩展色阶。
const fn tag_cyan_hue() -> PrimaryHue {
    PrimaryHue::Cyan
}

// 向 UIX 提供极客蓝扩展色阶。
const fn tag_geekblue_hue() -> PrimaryHue {
    PrimaryHue::Geekblue
}

// 向 UIX 提供紫色扩展色阶。
const fn tag_purple_hue() -> PrimaryHue {
    PrimaryHue::Purple
}

// 向 UIX 提供品红扩展色阶。
const fn tag_magenta_hue() -> PrimaryHue {
    PrimaryHue::Magenta
}

// 向 UIX 提供红色扩展色阶。
const fn tag_red_hue() -> PrimaryHue {
    PrimaryHue::Red
}

// 向 UIX 提供橙色扩展色阶。
const fn tag_orange_hue() -> PrimaryHue {
    PrimaryHue::Orange
}

// 向 UIX 提供青柠扩展色阶。
const fn tag_lime_hue() -> PrimaryHue {
    PrimaryHue::Lime
}

// 向 UIX 提供标签小号圆角主题角色。
const fn tag_radius_sm() -> TagRadiusRole {
    TagRadiusRole::Small
}

// 向 UIX 提供勾选状态图标名。
const fn tag_check_icon() -> &'static str {
    "check"
}

// 向 UIX 提供关闭操作图标名。
const fn tag_close_icon() -> &'static str {
    "x"
}

// Rust 直接构造或绕过 View 声明根时保持既有视觉；正常 View 构建会改用 UIX 静态配置。
static DEFAULT_TAG_VISUAL: TagVisual = tag_visual(
    tag_layout(
        12.0, 8.0, 4.0, 0.25, 20.0, 14.0, 0.5, 0.75, 1.5, 1.0, 2.0, 10.0, 0.85, 1.0, 4.0, 10.0,
        14.0, 28.0, 16.0, 32.0,
    ),
    tag_palette(
        tag_token_pair(tag_fill_tertiary(), tag_text()),
        tag_token_pair(tag_success_bg(), tag_success()),
        tag_token_pair(tag_info_bg(), tag_info()),
        tag_token_pair(tag_warning_bg(), tag_warning()),
        tag_token_pair(tag_error_bg(), tag_error()),
        tag_token_pair(tag_primary_bg(), tag_primary()),
        tag_hue_pair(tag_cyan_hue()),
        tag_hue_pair(tag_geekblue_hue()),
        tag_hue_pair(tag_purple_hue()),
        tag_hue_pair(tag_magenta_hue()),
        tag_hue_pair(tag_red_hue()),
        tag_hue_pair(tag_orange_hue()),
        tag_token_pair(tag_warning_bg(), tag_warning()),
        tag_hue_pair(tag_lime_hue()),
        tag_token_pair(tag_success_bg(), tag_success()),
    ),
    tag_black(),
    tag_white(),
    tag_black(),
    tag_white(),
    tag_primary(),
    tag_radius_sm(),
    tag_check_icon(),
    tag_close_icon(),
);

// 正常 UIX 构建首次写入声明配置，后续 Tag 实例只共享该静态对象。
static UIX_TAG_VISUAL: OnceLock<TagVisual> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagAction {
    Closed,
    Checked,
    Unchecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagTarget {
    Body,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagPress {
    Pointer(TagTarget),
    Key(KeyCode),
}

#[derive(Debug, Clone, Copy)]
struct TagGeometry {
    frame: Rect,
    body: Rect,
    close: Option<Rect>,
    check: Option<Rect>,
    text: Rect,
}

widget! {
    /// 展示可关闭或可勾选短文本状态的标签组件。
    pub struct Tag {
        text: String,
        color: TagColor,
        closable: bool,
        font_size: f32,
        #[snapshot(skip)]
        font_size_authored: bool,
        custom_color: Option<Color>,
        checkable: bool,
        checked: bool,
        visible: bool,
        focused: bool,
        /// Lucide 图标名称，绘制在文字之前。
        icon: String,
        #[snapshot(skip)]
        cached_text_width: Cell<f32>,
        last_size: Cell<Size>,
        layout_requested: Cell<bool>,
        pending_action: Cell<Option<TagAction>>,
        hovered_target: Cell<Option<TagTarget>>,
        pressed: Cell<Option<TagPress>>,
        #[snapshot(skip)]
        visual: &'static TagVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        if self.visible {
            constraints.clamp(self.intrinsic_size())
        } else {
            Size::zero()
        }
    }

    visible => (&self) -> bool { self.visible }

    tab_index => (&self) -> i32 {
        i32::from(self.visible && (self.closable || self.checkable))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.visible || (!self.closable && !self.checkable) {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(target) = self.target_at(*pos) {
                    self.hovered_target.set(Some(target));
                    self.pressed.set(Some(TagPress::Pointer(target)));
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(TagPress::Pointer(pressed)) = self.pressed.replace(None) else {
                    return EventResult::NotHandled;
                };
                let released = self.target_at(*pos);
                self.hovered_target.set(released);
                if released == Some(pressed) {
                    self.commit_target(pressed);
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let next = self.target_at(*pos);
                if self.hovered_target.replace(next) != next {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let pointer_pressed = matches!(self.pressed.get(), Some(TagPress::Pointer(_)));
                if pointer_pressed {
                    self.pressed.set(None);
                }
                let changed = self.hovered_target.replace(None).is_some() | pointer_pressed;
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyDown {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                if self.pressed.get().is_none() {
                    self.pressed.set(Some(TagPress::Key(*key)));
                }
                EventResult::Handled
            }
            SystemEvent::KeyUp {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                if self.pressed.replace(None) == Some(TagPress::Key(*key)) {
                    self.commit_target(if self.checkable {
                        TagTarget::Body
                    } else {
                        TagTarget::Close
                    });
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyDown {
                key: KeyCode::Delete | KeyCode::Escape,
                ..
            } if self.closable => {
                self.dismiss();
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed.set(None);
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_action.replace(None).map(|action| {
            let payload = match action {
                TagAction::Closed => "closed",
                TagAction::Checked => "checked",
                TagAction::Unchecked => "unchecked",
            };
            SemanticEvent::change(id, payload)
        })
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let geometry = self.geometry(frame);
        let frame = geometry.frame;
        self.last_size.set(Size::new(frame.w, frame.h));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let layout = &self.visual.layout;
        let font_size = normalized_tag_font_size(self.font_size, layout.default_font_size);
        let (bg, fg) = if let Some(cc) = self.custom_color {
            let text = if cc.is_light() {
                self.visual.custom_light_text
            } else {
                self.visual.custom_dark_text
            };
            (cc, text.resolve(ctx.tokens()))
        } else {
            resolve_tag_colors(self.color, &self.visual.palette, ctx.tokens())
        };
        let radius = self
            .visual
            .corner_radius
            .resolve(ctx.tokens())
            .min(frame.w.min(frame.h) * layout.corner_radius_limit);
        let r = Some(Radius::uniform(radius));
        ctx.push_clip(frame);
        ctx.fill_rect(frame, bg, r);
        let visual_target = match self.pressed.get() {
            Some(TagPress::Pointer(target)) if self.hovered_target.get() == Some(target) => {
                Some((target, true))
            }
            Some(TagPress::Key(_)) => Some((
                if self.checkable {
                    TagTarget::Body
                } else {
                    TagTarget::Close
                },
                true,
            )),
            _ => self.hovered_target.get().map(|target| (target, false)),
        };
        if let Some((target, pressed)) = visual_target {
            let target_frame = match target {
                TagTarget::Body => geometry.body,
                TagTarget::Close => geometry.close.unwrap_or(geometry.frame),
            };
            let overlay = if bg.is_light() {
                self.visual.light_overlay.resolve(ctx.tokens()).with_alpha(
                    if pressed {
                        layout.light_pressed_alpha
                    } else {
                        layout.light_hover_alpha
                    },
                )
            } else {
                self.visual.dark_overlay.resolve(ctx.tokens()).with_alpha(
                    if pressed {
                        layout.dark_pressed_alpha
                    } else {
                        layout.dark_hover_alpha
                    },
                )
            };
            ctx.fill_rect(target_frame, overlay, None);
        }
        if self.checked {
            let inset = layout
                .checked_inset
                .min(frame.w * 0.5)
                .min(frame.h * 0.5);
            ctx.stroke_rect(
                Rect::new(
                    frame.x + inset,
                    frame.y + inset,
                    (frame.w - inset * 2.0).max(0.0),
                    (frame.h - inset * 2.0).max(0.0),
                ),
                fg,
                layout.checked_stroke_width,
                Some(Radius::uniform(radius)),
            );
        }
        if self.focused && tree.keyboard_focus_visible() {
            let inset = layout
                .focus_inset
                .min(frame.w * 0.5)
                .min(frame.h * 0.5);
            ctx.stroke_rect(
                Rect::new(
                    frame.x + inset,
                    frame.y + inset,
                    (frame.w - inset * 2.0).max(0.0),
                    (frame.h - inset * 2.0).max(0.0),
                ),
                self.visual.focus_color.resolve(ctx.tokens()),
                layout.focus_stroke_width,
                Some(Radius::uniform(radius)),
            );
        }
        if self.checked {
            if let Some(check) = geometry.check {
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    self.visual.check_icon,
                    check,
                    fg,
                    layout.check_icon_size,
                );
            }
        }
        // 图标绘制（在文字之前）
        if !self.icon.is_empty() {
            let icon_size = font_size * layout.leading_icon_scale;
            let icon_rect = Rect::new(
                geometry.text.x,
                geometry.text.y,
                icon_size.min(geometry.text.w),
                geometry.text.h,
            );
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                &self.icon,
                icon_rect,
                fg,
                icon_size,
            );
            let remaining = Rect::new(
                icon_rect.x + icon_rect.w + layout.icon_gap,
                geometry.text.y,
                (geometry.text.x + geometry.text.w
                    - icon_rect.x
                    - icon_rect.w
                    - layout.icon_gap)
                    .max(0.0),
                geometry.text.h,
            );
            Self::paint_single_line(ctx, &self.text, remaining, fg, font_size);
        } else {
            Self::paint_single_line(ctx, &self.text, geometry.text, fg, font_size);
        }
        if let Some(close) = geometry.close {
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.close_icon,
                close,
                fg,
                layout.close_icon_size,
            );
        }
        ctx.pop_clip();
    }
}

impl Default for Tag {
    fn default() -> Self {
        Self::new("")
    }
}

// 把 UIX 声明的共享视觉配置融合进标签 Rust 交互与绘制内核。
fn build_tag_view(mut kernel: Tag, declared_visual: TagVisual) -> ViewNode {
    let visual = UIX_TAG_VISUAL.get_or_init(|| declared_visual);
    // 单一同目录 UIX 源在同一程序中必须保持一份确定配置。
    debug_assert_eq!(*visual, declared_visual);
    if !kernel.font_size_authored {
        if kernel.font_size != visual.layout.default_font_size {
            kernel.cached_text_width.set(f32::NAN);
        }
        kernel.font_size = visual.layout.default_font_size;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Tag {
    fn build(self) -> ViewNode {
        // UIX 拥有视觉配置，Rust 保留关闭、勾选、状态协调与底层绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/tag/tag.uix")
    }
}

// 验证标签预设色服从主题 token 与明暗模式。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/display/tag__palette_tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod palette_tests;

// 验证 UIX 声明壳与 Rust 内核的单节点契约。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/display/tag__uix_tests.rs"]
mod uix_tests;

impl Tag {
    /// 使用文本创建默认中性色、不可关闭且不可勾选的标签。
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: TagColor::Default,
            closable: false,
            font_size: DEFAULT_TAG_FONT_SIZE,
            font_size_authored: false,
            custom_color: None,
            checkable: false,
            checked: false,
            visible: true,
            focused: false,
            icon: String::new(),
            cached_text_width: Cell::new(f32::NAN),
            last_size: Cell::new(Size::zero()),
            layout_requested: Cell::new(false),
            pending_action: Cell::new(None),
            hovered_target: Cell::new(None),
            pressed: Cell::new(None),
            visual: &DEFAULT_TAG_VISUAL,
        }
    }
    /// 设置由当前主题解析的预设标签颜色。
    pub fn color(mut self, c: TagColor) -> Self {
        self.color = c;
        self
    }
    /// 设置固定背景色，并根据背景亮度选择黑色或白色前景。
    pub fn custom_color(mut self, c: Color) -> Self {
        self.custom_color = Some(c);
        self
    }
    /// 启用标签关闭入口与对应交互。
    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }
    /// 设置标签是否允许勾选；关闭时同时清除勾选状态。
    pub fn checkable(mut self, v: bool) -> Self {
        self.checkable = v;
        if !v {
            self.checked = false;
        }
        self
    }
    /// 设置可勾选 Tag 的初始状态；用户交互后的状态在 reconcile 中保留。
    pub fn default_checked(mut self, checked: bool) -> Self {
        self.checkable = true;
        self.checked = checked;
        self
    }
    /// 设置标签字号；非法或非正值回退为默认字号。
    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size_authored = s.is_finite() && s > 0.0;
        self.font_size = normalized_tag_font_size(s, self.visual.layout.default_font_size);
        self.cached_text_width.set(f32::NAN);
        self
    }

    /// 在文字前显示 Lucide 图标。
    pub fn icon(mut self, name: impl Into<String>) -> Self {
        self.icon = name.into();
        self
    }

    fn intrinsic_size(&self) -> Size {
        let layout = &self.visual.layout;
        let font_size = normalized_tag_font_size(self.font_size, layout.default_font_size);
        let cached_text_width = self.cached_text_width.get();
        let text_width = if cached_text_width.is_finite() {
            cached_text_width
        } else {
            let measured = crate::draw::resources::font::text_backend::estimate_text_metrics(
                &self.text,
                f32::INFINITY,
                font_size,
            )
            .max_line_width;
            self.cached_text_width.set(measured);
            measured
        };
        let icon_width = if !self.icon.is_empty() {
            font_size * layout.leading_icon_reserve_scale + layout.icon_gap
        } else {
            0.0
        };
        let w = text_width
            + icon_width
            + layout.horizontal_padding * 2.0
            + if self.checkable {
                layout.check_width
            } else {
                0.0
            }
            + if self.closable {
                layout.close_width
            } else {
                0.0
            };
        Size::new(w, font_size + layout.vertical_padding * 2.0)
    }

    fn geometry(&self, frame: Rect) -> TagGeometry {
        let layout = &self.visual.layout;
        let frame = Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        );
        let close_width = if self.closable && frame.w > 0.0 {
            layout.close_width.min(frame.w)
        } else {
            0.0
        };
        let close = (close_width > 0.0).then(|| {
            Rect::new(
                frame.x + frame.w - close_width,
                frame.y,
                close_width,
                frame.h,
            )
        });
        let body = Rect::new(frame.x, frame.y, (frame.w - close_width).max(0.0), frame.h);
        let horizontal_padding = layout
            .horizontal_padding
            .min(body.w * layout.compact_padding_ratio);
        let check_width = if self.checkable {
            layout
                .check_width
                .min((body.w - horizontal_padding * 2.0).max(0.0))
        } else {
            0.0
        };
        let check = (self.checkable && check_width > 0.0)
            .then(|| Rect::new(body.x + horizontal_padding, body.y, check_width, body.h));
        let reserved_check = if check.is_some() { check_width } else { 0.0 };
        let text_x = body.x + horizontal_padding + reserved_check;
        let text = Rect::new(
            text_x,
            body.y,
            (body.x + body.w - horizontal_padding - text_x).max(0.0),
            body.h,
        );
        TagGeometry {
            frame,
            body,
            close,
            check,
            text,
        }
    }

    fn paint_single_line(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(value) = ctx.elide_single_line(value, font_size, frame.w) else {
            return;
        };
        ctx.push_clip(frame);
        ctx.draw_text_in_frame(&value, frame, color, font_size);
        ctx.pop_clip();
    }

    fn target_at(&self, point: crate::core::Point) -> Option<TagTarget> {
        let size = self.last_size.get();
        let size = if size.w > 0.0 && size.h > 0.0 {
            size
        } else {
            self.intrinsic_size()
        };
        let geometry = self.geometry(Rect::new(0.0, 0.0, size.w, size.h));
        if self.closable && geometry.close.is_some_and(|close| close.contains(point)) {
            Some(TagTarget::Close)
        } else if self.checkable && geometry.body.contains(point) {
            Some(TagTarget::Body)
        } else {
            None
        }
    }

    fn commit_target(&mut self, target: TagTarget) {
        match target {
            TagTarget::Body => self.toggle_checked(),
            TagTarget::Close => self.dismiss(),
        }
    }

    /// 返回标签当前是否参与布局与绘制。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 返回标签当前的勾选状态。
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// 在标签可勾选时更新勾选状态，否则忽略请求。
    pub fn set_checked(&mut self, checked: bool) {
        if self.checkable {
            self.checked = checked;
        }
    }

    /// 隐藏可见标签并清理焦点与指针交互状态。
    pub fn close(&mut self) {
        if self.visible {
            self.visible = false;
            self.focused = false;
            self.hovered_target.set(None);
            self.pressed.set(None);
            self.layout_requested.set(true);
        }
    }

    /// 重新显示已隐藏的标签并请求布局。
    pub fn open(&mut self) {
        if !self.visible {
            self.visible = true;
            self.layout_requested.set(true);
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Tag {
            text: self.text.clone(),
            color: self.color,
            closable: self.closable,
            font_size: self.font_size,
            custom_color: self.custom_color,
            checkable: self.checkable,
            checked: self.checked,
            visible: self.visible,
            icon: self.icon.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let was_checkable = self.checkable;
        let runtime_checked = self.checked;
        let text_metrics_changed = self.text != next.text
            || self.font_size != next.font_size
            || !std::ptr::eq(self.visual, next.visual);
        self.text = next.text;
        self.color = next.color;
        self.closable = next.closable;
        self.font_size = next.font_size;
        self.font_size_authored = next.font_size_authored;
        self.custom_color = next.custom_color;
        self.checkable = next.checkable;
        self.icon = next.icon;
        self.visual = next.visual;
        if text_metrics_changed {
            self.cached_text_width.set(f32::NAN);
        }
        self.checked = if !self.checkable {
            false
        } else if was_checkable {
            runtime_checked
        } else {
            next.checked
        };
        self.hovered_target.set(None);
        self.pressed.set(None);
    }

    fn dismiss(&mut self) {
        self.close();
        self.pending_action.set(Some(TagAction::Closed));
    }

    fn toggle_checked(&mut self) {
        self.checked = !self.checked;
        self.pending_action.set(Some(if self.checked {
            TagAction::Checked
        } else {
            TagAction::Unchecked
        }));
    }
}
