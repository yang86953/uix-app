//! Result widget — 结果页，Ant Design 风格。
//!
//! 用于展示操作结果（成功/错误/警告/信息/404/403/500），
//! 包含图标、标题、副标题、额外操作区域。

use std::borrow::Cow;
use std::cell::Cell;
use std::sync::OnceLock;

use crate::core::{Constraints, Rect, Size};
use crate::draw::Color;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SnapshotFields, SystemEvent, ThemeTokens, WidgetTree,
};
use crate::widget;

// 保存由 UIX 声明、由 Rust 布局算法消费的几何配置。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResultLayoutVisual {
    intrinsic_width: f32,
    intrinsic_height: f32,
    horizontal_padding_max: f32,
    horizontal_padding_ratio: f32,
    vertical_padding_max: f32,
    vertical_padding_ratio: f32,
    action_height: f32,
    action_gap_max: f32,
    action_gap_ratio: f32,
    icon_ratio_with_subtitle: f32,
    icon_ratio_without_subtitle: f32,
    icon_size_max: f32,
    icon_width_ratio: f32,
    icon_title_gap_max: f32,
    icon_title_gap_ratio: f32,
    subtitle_gap_max: f32,
    subtitle_gap_ratio: f32,
    title_extra_ratio: f32,
    action_horizontal_padding: f32,
    action_padding_ratio: f32,
    action_radius: f32,
    focus_inset: f32,
    focus_stroke_width: f32,
}

// 保存由 UIX 声明、由 Rust 文本测量与栅格化消费的排版配置。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResultTypographyVisual {
    title_font_size: f32,
    title_max_lines: u8,
    subtitle_font_size: f32,
    subtitle_max_lines: u8,
    action_font_fallback: f32,
    action_font: ResultFontRole,
    line_height: f32,
    status_code_font_size_max: f32,
    status_code_height_ratio: f32,
    icon_radius_ratio: f32,
    icon_glyph_ratio: f32,
    icon_glyph_min_size: f32,
}

// 结果页文本使用的主题字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultFontRole {
    Body,
}

impl ResultFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Body => tokens.font_size(),
        }
    }
}

// 保存结果页各绘制区域使用的主题语义色。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResultPalette {
    text: ColorValue,
    text_secondary: ColorValue,
    white: ColorValue,
    primary: ColorValue,
    primary_active: ColorValue,
    success: ColorValue,
    error: ColorValue,
    info: ColorValue,
    warning: ColorValue,
}

// 结果状态图标使用的语义色角色，避免每项重复保存完整颜色值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultStatusColorRole {
    TextSecondary,
    Success,
    Error,
    Info,
    Warning,
}

impl ResultStatusColorRole {
    fn resolve(self, palette: &ResultPalette, tokens: &dyn ThemeTokens) -> Color {
        match self {
            Self::TextSecondary => palette.text_secondary,
            Self::Success => palette.success,
            Self::Error => palette.error,
            Self::Info => palette.info,
            Self::Warning => palette.warning,
        }
        .resolve(tokens)
    }
}

// 保存由 UIX 声明的单个结果状态图标与颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResultStatusVisual {
    icon: &'static str,
    color: ResultStatusColorRole,
}

// 完整视觉配置由所有 ResultView 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResultVisual {
    layout: ResultLayoutVisual,
    typography: ResultTypographyVisual,
    palette: ResultPalette,
    statuses: [ResultStatusVisual; 7],
}

impl ResultVisual {
    fn status(&self, type_: ResultType) -> ResultStatusVisual {
        self.statuses[match type_ {
            ResultType::Success => 0,
            ResultType::Error => 1,
            ResultType::Info => 2,
            ResultType::Warning => 3,
            ResultType::NotFound => 4,
            ResultType::Forbidden => 5,
            ResultType::ServerError => 6,
        }]
    }
}

/// 结果类型。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResultType {
    /// 操作成功。
    Success,
    /// 操作失败。
    Error,
    /// 一般信息。
    Info,
    /// 警告信息。
    Warning,
    /// 资源未找到，对应 HTTP 404 语义。
    NotFound,
    /// 禁止访问，对应 HTTP 403 语义。
    Forbidden,
    /// 服务器错误，对应 HTTP 500 语义。
    ServerError,
}

impl ResultType {
    pub(crate) fn localized_title(self) -> &'static str {
        let loc = crate::ui::widget_runtime::locale::use_locale();
        match self {
            Self::Success => loc.result_success,
            Self::Error => loc.result_error,
            Self::Info => loc.result_info,
            Self::Warning => loc.result_warning,
            Self::NotFound => loc.result_404,
            Self::Forbidden => loc.result_403,
            Self::ServerError => loc.result_500,
        }
    }

    pub(crate) fn localized_subtitle(self) -> &'static str {
        let loc = crate::ui::widget_runtime::locale::use_locale();
        match self {
            Self::NotFound => loc.result_404_desc,
            Self::Forbidden => loc.result_403_desc,
            Self::ServerError => loc.result_500_desc,
            _ => "",
        }
    }
}

// 组合 UIX 声明的结果页几何常量。
#[allow(clippy::too_many_arguments)]
const fn result_layout(
    intrinsic_width: f32,
    intrinsic_height: f32,
    horizontal_padding_max: f32,
    horizontal_padding_ratio: f32,
    vertical_padding_max: f32,
    vertical_padding_ratio: f32,
    action_height: f32,
    action_gap_max: f32,
    action_gap_ratio: f32,
    icon_ratio_with_subtitle: f32,
    icon_ratio_without_subtitle: f32,
    icon_size_max: f32,
    icon_width_ratio: f32,
    icon_title_gap_max: f32,
    icon_title_gap_ratio: f32,
    subtitle_gap_max: f32,
    subtitle_gap_ratio: f32,
    title_extra_ratio: f32,
    action_horizontal_padding: f32,
    action_padding_ratio: f32,
    action_radius: f32,
    focus_inset: f32,
    focus_stroke_width: f32,
) -> ResultLayoutVisual {
    ResultLayoutVisual {
        intrinsic_width,
        intrinsic_height,
        horizontal_padding_max,
        horizontal_padding_ratio,
        vertical_padding_max,
        vertical_padding_ratio,
        action_height,
        action_gap_max,
        action_gap_ratio,
        icon_ratio_with_subtitle,
        icon_ratio_without_subtitle,
        icon_size_max,
        icon_width_ratio,
        icon_title_gap_max,
        icon_title_gap_ratio,
        subtitle_gap_max,
        subtitle_gap_ratio,
        title_extra_ratio,
        action_horizontal_padding,
        action_padding_ratio,
        action_radius,
        focus_inset,
        focus_stroke_width,
    }
}

// 组合 UIX 声明的结果页排版与图标比例常量。
#[allow(clippy::too_many_arguments)]
const fn result_typography(
    title_font_size: f32,
    title_max_lines: f32,
    subtitle_font_size: f32,
    subtitle_max_lines: f32,
    action_font_fallback: f32,
    action_font: ResultFontRole,
    line_height: f32,
    status_code_font_size_max: f32,
    status_code_height_ratio: f32,
    icon_radius_ratio: f32,
    icon_glyph_ratio: f32,
    icon_glyph_min_size: f32,
) -> ResultTypographyVisual {
    ResultTypographyVisual {
        title_font_size,
        title_max_lines: title_max_lines as u8,
        subtitle_font_size,
        subtitle_max_lines: subtitle_max_lines as u8,
        action_font_fallback,
        action_font,
        line_height,
        status_code_font_size_max,
        status_code_height_ratio,
        icon_radius_ratio,
        icon_glyph_ratio,
        icon_glyph_min_size,
    }
}

// 组合 UIX 声明的结果页主题色角色。
#[allow(clippy::too_many_arguments)]
const fn result_palette(
    text: ColorValue,
    text_secondary: ColorValue,
    white: ColorValue,
    primary: ColorValue,
    primary_active: ColorValue,
    success: ColorValue,
    error: ColorValue,
    info: ColorValue,
    warning: ColorValue,
) -> ResultPalette {
    ResultPalette {
        text,
        text_secondary,
        white,
        primary,
        primary_active,
        success,
        error,
        info,
        warning,
    }
}

// 组合 UIX 声明的单个状态图标。
const fn result_status(icon: &'static str, color: ResultStatusColorRole) -> ResultStatusVisual {
    ResultStatusVisual { icon, color }
}

// 按公开 ResultType 顺序组合全部状态视觉。
const fn result_statuses(
    success: ResultStatusVisual,
    error: ResultStatusVisual,
    info: ResultStatusVisual,
    warning: ResultStatusVisual,
    not_found: ResultStatusVisual,
    forbidden: ResultStatusVisual,
    server_error: ResultStatusVisual,
) -> [ResultStatusVisual; 7] {
    [
        success,
        error,
        info,
        warning,
        not_found,
        forbidden,
        server_error,
    ]
}

// 组合 UIX 声明的完整结果页视觉配置。
const fn result_visual(
    layout: ResultLayoutVisual,
    typography: ResultTypographyVisual,
    palette: ResultPalette,
    statuses: [ResultStatusVisual; 7],
) -> ResultVisual {
    ResultVisual {
        layout,
        typography,
        palette,
        statuses,
    }
}

// 向 UIX 提供结果页正文主题角色。
const fn result_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

// 向 UIX 提供结果页次级正文主题角色。
const fn result_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}

// 向 UIX 提供结果页反白主题角色。
const fn result_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}

// 向 UIX 提供结果页主操作默认主题角色。
const fn result_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

// 向 UIX 提供结果页主操作按下主题角色。
const fn result_primary_active() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryActive)
}

// 向 UIX 提供结果页成功主题角色。
const fn result_success() -> ColorValue {
    ColorValue::Palette(PaletteColor::Success)
}

// 向 UIX 提供结果页错误主题角色。
const fn result_error() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}

// 向 UIX 提供结果页信息主题角色。
const fn result_info() -> ColorValue {
    ColorValue::Palette(PaletteColor::Info)
}

// 向 UIX 提供结果页警告主题角色。
const fn result_warning() -> ColorValue {
    ColorValue::Palette(PaletteColor::Warning)
}

// 向 UIX 提供结果页操作文字的正文主题字号角色。
const fn result_body_font() -> ResultFontRole {
    ResultFontRole::Body
}

// 向 UIX 提供状态图标的次级正文颜色角色。
const fn result_status_text_secondary() -> ResultStatusColorRole {
    ResultStatusColorRole::TextSecondary
}

// 向 UIX 提供状态图标的成功颜色角色。
const fn result_status_success() -> ResultStatusColorRole {
    ResultStatusColorRole::Success
}

// 向 UIX 提供状态图标的错误颜色角色。
const fn result_status_error() -> ResultStatusColorRole {
    ResultStatusColorRole::Error
}

// 向 UIX 提供状态图标的信息颜色角色。
const fn result_status_info() -> ResultStatusColorRole {
    ResultStatusColorRole::Info
}

// 向 UIX 提供状态图标的警告颜色角色。
const fn result_status_warning() -> ResultStatusColorRole {
    ResultStatusColorRole::Warning
}

// 向 UIX 提供成功状态图标名。
const fn result_icon_success() -> &'static str {
    "check"
}

// 向 UIX 提供错误状态图标名。
const fn result_icon_error() -> &'static str {
    "x"
}

// 向 UIX 提供信息状态图标名。
const fn result_icon_info() -> &'static str {
    "info"
}

// 向 UIX 提供警告状态图标名。
const fn result_icon_warning() -> &'static str {
    "alert-triangle"
}

// 向 UIX 提供资源不存在状态码文案。
const fn result_icon_not_found() -> &'static str {
    "404"
}

// 向 UIX 提供禁止访问状态码文案。
const fn result_icon_forbidden() -> &'static str {
    "403"
}

// 向 UIX 提供服务错误状态码文案。
const fn result_icon_server_error() -> &'static str {
    "500"
}

// Rust 直接构造或绕过 View 声明根时保持既有视觉；正常 View 构建会改用 UIX 静态配置。
static DEFAULT_RESULT_VISUAL: ResultVisual = result_visual(
    result_layout(
        400.0, 300.0, 12.0, 0.1, 12.0, 0.08, 36.0, 12.0, 0.12, 0.35, 0.45, 64.0, 0.45, 10.0, 0.16,
        6.0, 0.1, 0.35, 16.0, 0.25, 6.0, 2.0, 2.0,
    ),
    result_typography(
        20.0,
        2.0,
        13.0,
        3.0,
        14.0,
        result_body_font(),
        1.5,
        48.0,
        0.82,
        0.5,
        0.8,
        1.0,
    ),
    result_palette(
        result_text(),
        result_text_secondary(),
        result_white(),
        result_primary(),
        result_primary_active(),
        result_success(),
        result_error(),
        result_info(),
        result_warning(),
    ),
    result_statuses(
        result_status(result_icon_success(), result_status_success()),
        result_status(result_icon_error(), result_status_error()),
        result_status(result_icon_info(), result_status_info()),
        result_status(result_icon_warning(), result_status_warning()),
        result_status(result_icon_not_found(), result_status_text_secondary()),
        result_status(result_icon_forbidden(), result_status_warning()),
        result_status(result_icon_server_error(), result_status_error()),
    ),
);

// 正常 UIX 构建首次写入声明配置，后续 ResultView 实例只共享该静态对象。
static UIX_RESULT_VISUAL: OnceLock<ResultVisual> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
struct ResultGeometry {
    frame: Rect,
    icon: Rect,
    title: Rect,
    title_line_count: u8,
    subtitle: Rect,
    subtitle_line_count: u8,
    action: Rect,
}

// ResultView — 结果页组件。
widget! {
    /// 展示状态图标、标题、副标题和可选操作文本的结果页。
    pub struct ResultView {
        type_: ResultType,
        title: String,
        subtitle: String,
        extra_text: String,
        focused: bool,
        pressed: bool,
        last_action_rect: Cell<Rect>,
        #[snapshot(skip)]
        visual: &'static ResultVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    tab_index => (&self) -> i32 { i32::from(!self.extra_text.is_empty()) }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.extra_text.is_empty() {
            frame
        } else {
            let local_frame = Rect::new(0.0, 0.0, frame.w, frame.h);
            let action = self
                .layout(local_frame, self.visual.typography.action_font_fallback)
                .action;
            self.last_action_rect.set(action);
            Rect::new(
                frame.x + action.x,
                frame.y + action.y,
                action.w,
                action.h,
            )
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.extra_text.is_empty() {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown { pos, button: MouseButton::Left, .. }
                if self.action_rect_local().contains(*pos) =>
            {
                self.pressed = true;
                EventResult::Handled
            }
            SystemEvent::PointerUp { pos, button: MouseButton::Left, .. } if self.pressed => {
                let activated = self.action_rect_local().contains(*pos);
                self.pressed = false;
                if activated {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave if self.pressed => {
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
            _ => EventResult::NotHandled,
        }
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let action_font_size = self.visual.typography.action_font.resolve(ctx.tokens());
        let geometry = self.layout(frame, action_font_size);
        if geometry.frame.w <= 0.0 || geometry.frame.h <= 0.0 {
            self.last_action_rect.set(Rect::zero());
            return;
        }
        let palette = &self.visual.palette;
        let text = palette.text.resolve(ctx.tokens());
        let text_sec = palette.text_secondary.resolve(ctx.tokens());
        let (main_title, sub) = self.effective_content();
        let status = self.visual.status(self.type_);
        let icon_color = status.color.resolve(palette, ctx.tokens());
        let typography = &self.visual.typography;

        ctx.push_clip(geometry.frame);
        match self.type_ {
            ResultType::NotFound | ResultType::Forbidden | ResultType::ServerError => {
                let font_size = typography
                    .status_code_font_size_max
                    .min(geometry.icon.h * typography.status_code_height_ratio);
                ctx.text_center(status.icon, geometry.icon, icon_color, font_size);
            }
            _ => {
                let radius = geometry.icon.w.min(geometry.icon.h) * typography.icon_radius_ratio;
                let center_x = geometry.icon.x + geometry.icon.w * 0.5;
                let center_y = geometry.icon.y + geometry.icon.h * 0.5;
                ctx.fill_circle(center_x, center_y, radius, icon_color);
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    status.icon,
                    geometry.icon,
                    palette.white.resolve(ctx.tokens()),
                    (radius * typography.icon_glyph_ratio).max(typography.icon_glyph_min_size),
                );
            }
        }

        Self::paint_text_block(
            ctx,
            main_title,
            geometry.title,
            geometry.title_line_count,
            text,
            typography.title_font_size,
            typography.line_height,
        );
        Self::paint_text_block(
            ctx,
            sub,
            geometry.subtitle,
            geometry.subtitle_line_count,
            text_sec,
            typography.subtitle_font_size,
            typography.line_height,
        );

        if !self.extra_text.is_empty() {
            let btn_rect = geometry.action;
            self.last_action_rect.set(Rect::new(
                btn_rect.x - geometry.frame.x,
                btn_rect.y - geometry.frame.y,
                btn_rect.w,
                btn_rect.h,
            ));
            let layout = &self.visual.layout;
            let radius_value = layout
                .action_radius
                .min(btn_rect.w.min(btn_rect.h) * 0.5);
            let radius = Some(crate::draw::Radius::uniform(radius_value));
            let background = if self.pressed {
                palette.primary_active.resolve(ctx.tokens())
            } else {
                palette.primary.resolve(ctx.tokens())
            };
            ctx.fill_rect(btn_rect, background, radius);
            if self.focused && tree.keyboard_focus_visible() {
                let focus = Self::inset(btn_rect, layout.focus_inset);
                ctx.stroke_rect(
                    focus,
                    palette.white.resolve(ctx.tokens()),
                    layout.focus_stroke_width,
                    Some(crate::draw::Radius::uniform(
                        radius_value.min(focus.w.min(focus.h) * 0.5),
                    )),
                );
            }
            self.paint_action_text(ctx, &self.extra_text, btn_rect, action_font_size);
        } else {
            self.last_action_rect.set(Rect::zero());
        }
        ctx.pop_clip();
    }
}

impl ResultView {
    /// 创建使用指定结果类型及其本地化默认文案的结果页。
    pub fn new(type_: ResultType) -> Self {
        Self {
            type_,
            title: String::new(),
            subtitle: String::new(),
            extra_text: String::new(),
            focused: false,
            pressed: false,
            last_action_rect: Cell::new(Rect::zero()),
            visual: &DEFAULT_RESULT_VISUAL,
        }
    }
    /// 覆盖结果页标题；空文本继续使用本地化默认标题。
    pub fn title(mut self, t: &str) -> Self {
        self.title = t.to_string();
        self
    }
    /// 覆盖结果页副标题；空文本继续使用本地化默认副标题。
    pub fn subtitle(mut self, s: &str) -> Self {
        self.subtitle = s.to_string();
        self
    }
    /// 设置结果页底部的额外操作文本。
    pub fn extra_text(mut self, t: impl Into<String>) -> Self {
        self.extra_text = t.into();
        self
    }

    fn effective_content(&self) -> (&str, &str) {
        let title = if self.title.is_empty() {
            self.type_.localized_title()
        } else {
            &self.title
        };
        let subtitle = if self.subtitle.is_empty() {
            self.type_.localized_subtitle()
        } else {
            &self.subtitle
        };
        (title, subtitle)
    }

    fn layout(&self, frame: Rect, action_font_size: f32) -> ResultGeometry {
        let frame = Self::normalized_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return ResultGeometry {
                frame,
                icon: Rect::zero(),
                title: Rect::zero(),
                title_line_count: 0,
                subtitle: Rect::zero(),
                subtitle_line_count: 0,
                action: Rect::zero(),
            };
        }

        let layout = &self.visual.layout;
        let typography = &self.visual.typography;
        let horizontal_padding = layout
            .horizontal_padding_max
            .min(frame.w * layout.horizontal_padding_ratio);
        let vertical_padding = layout
            .vertical_padding_max
            .min(frame.h * layout.vertical_padding_ratio);
        let inner = Rect::new(
            frame.x + horizontal_padding,
            frame.y + vertical_padding,
            (frame.w - horizontal_padding * 2.0).max(0.0),
            (frame.h - vertical_padding * 2.0).max(0.0),
        );
        let has_action = !self.extra_text.is_empty();
        let action_height = if has_action {
            layout.action_height.min(inner.h)
        } else {
            0.0
        };
        let action_gap = if has_action {
            layout
                .action_gap_max
                .min((inner.h - action_height).max(0.0) * layout.action_gap_ratio)
        } else {
            0.0
        };
        let content_height = (inner.h - action_height - action_gap).max(0.0);
        let (title, subtitle) = self.effective_content();
        let has_subtitle = !subtitle.is_empty();
        let icon_fraction = if has_subtitle {
            layout.icon_ratio_with_subtitle
        } else {
            layout.icon_ratio_without_subtitle
        };
        let icon_size = layout
            .icon_size_max
            .min(inner.w * layout.icon_width_ratio)
            .min(content_height * icon_fraction)
            .max(0.0);
        let icon_title_gap = layout
            .icon_title_gap_max
            .min((content_height - icon_size).max(0.0) * layout.icon_title_gap_ratio);
        let text_height = (content_height - icon_size - icon_title_gap).max(0.0);
        let (title_desired, title_line_count) = Self::estimated_text_layout(
            title,
            inner.w,
            typography.title_font_size,
            typography.title_max_lines,
            typography.line_height,
        );
        let (subtitle_desired, subtitle_line_count) = Self::estimated_text_layout(
            subtitle,
            inner.w,
            typography.subtitle_font_size,
            typography.subtitle_max_lines,
            typography.line_height,
        );
        let subtitle_gap = if has_subtitle {
            layout
                .subtitle_gap_max
                .min(text_height * layout.subtitle_gap_ratio)
        } else {
            0.0
        };
        let available_text = (text_height - subtitle_gap).max(0.0);
        let (title_height, subtitle_height) = if has_subtitle {
            let minimum_title = (typography.title_font_size * typography.line_height)
                .min(available_text)
                .min(title_desired);
            let minimum_subtitle = (typography.subtitle_font_size * typography.line_height)
                .min((available_text - minimum_title).max(0.0))
                .min(subtitle_desired);
            let remaining = (available_text - minimum_title - minimum_subtitle).max(0.0);
            let extra_title = (title_desired - minimum_title)
                .max(0.0)
                .min(remaining * layout.title_extra_ratio);
            let title_height = minimum_title + extra_title;
            let subtitle_height = minimum_subtitle
                + (subtitle_desired - minimum_subtitle)
                    .max(0.0)
                    .min(remaining - extra_title);
            (title_height, subtitle_height)
        } else {
            (available_text.min(title_desired), 0.0)
        };

        let used_height = icon_size
            + icon_title_gap
            + title_height
            + subtitle_gap
            + subtitle_height
            + action_gap
            + action_height;
        let mut y = inner.y + (inner.h - used_height).max(0.0) * 0.5;
        let icon = Rect::new(
            inner.x + (inner.w - icon_size) * 0.5,
            y,
            icon_size,
            icon_size,
        );
        y += icon_size + icon_title_gap;
        let title = Rect::new(inner.x, y, inner.w, title_height);
        y += title_height + subtitle_gap;
        let subtitle = Rect::new(inner.x, y, inner.w, subtitle_height);
        y += subtitle_height + action_gap;
        let action = if has_action {
            let action_width = (Self::estimated_text_width(&self.extra_text, action_font_size)
                + layout.action_horizontal_padding * 2.0)
                .clamp(0.0, inner.w);
            Rect::new(
                inner.x + (inner.w - action_width) * 0.5,
                y,
                action_width,
                action_height,
            )
        } else {
            Rect::zero()
        };

        ResultGeometry {
            frame,
            icon,
            title,
            title_line_count,
            subtitle,
            subtitle_line_count,
            action,
        }
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
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
        )
    }

    fn inset(frame: Rect, amount: f32) -> Rect {
        let amount = amount.min(frame.w * 0.5).min(frame.h * 0.5).max(0.0);
        Rect::new(
            frame.x + amount,
            frame.y + amount,
            (frame.w - amount * 2.0).max(0.0),
            (frame.h - amount * 2.0).max(0.0),
        )
    }

    fn estimated_text_width(value: &str, font_size: f32) -> f32 {
        let value = Self::normalized_action_text(value);
        crate::draw::resources::font::text_backend::estimate_text_metrics(
            &value,
            f32::INFINITY,
            font_size,
        )
        .max_line_width
    }

    fn normalized_action_text(value: &str) -> Cow<'_, str> {
        if value.contains(['\r', '\n']) {
            Cow::Owned(value.replace(['\r', '\n'], " "))
        } else {
            Cow::Borrowed(value)
        }
    }

    fn estimated_text_layout(
        value: &str,
        width: f32,
        font_size: f32,
        max_lines: u8,
        line_height: f32,
    ) -> (f32, u8) {
        if value.is_empty() || width <= 0.0 {
            return (0.0, 0);
        }
        let line_count = crate::draw::resources::font::text_backend::estimate_text_metrics(
            value, width, font_size,
        )
        .line_count
        .clamp(1, max_lines as usize) as u8;
        (line_count as f32 * font_size * line_height, line_count)
    }

    fn paint_text_block(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        line_count: u8,
        color: Color,
        font_size: f32,
        line_height: f32,
    ) {
        if value.is_empty() || frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let line_box_height = font_size * line_height;
        let visible_lines = (frame.h / line_box_height).floor() as usize;
        if visible_lines == 0 {
            return;
        }
        ctx.push_clip(frame);
        if line_count <= 1 {
            ctx.text_center(value, frame, color, font_size);
        } else if visible_lines >= 2 {
            ctx.draw_text_wrapped(value, frame, color, font_size);
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        } else if let Some(value) = ctx.elide_single_line(value, font_size, frame.w) {
            ctx.text_center(&value, frame, color, font_size);
        }
        ctx.pop_clip();
    }

    fn paint_action_text(&self, ctx: &mut PaintContext, value: &str, frame: Rect, font_size: f32) {
        let layout = &self.visual.layout;
        let horizontal_padding = layout
            .action_horizontal_padding
            .min(frame.w * layout.action_padding_ratio);
        let content = Rect::new(
            frame.x + horizontal_padding,
            frame.y,
            (frame.w - horizontal_padding * 2.0).max(0.0),
            frame.h,
        );
        // 复用共享省略算法生成结果描述的可见文本。
        if let Some(value) = ctx.elide_single_line(value, font_size, content.w) {
            ctx.push_clip(content);
            ctx.text_center(
                &value,
                content,
                self.visual.palette.white.resolve(ctx.tokens()),
                font_size,
            );
            ctx.pop_clip();
        }
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(
            self.visual.layout.intrinsic_width,
            self.visual.layout.intrinsic_height,
        )
    }

    fn action_rect_local(&self) -> Rect {
        let rendered = self.last_action_rect.get();
        if rendered.w > 0.0 && rendered.h > 0.0 {
            return rendered;
        }
        let size = self.intrinsic_size();
        self.layout(
            Rect::new(0.0, 0.0, size.w, size.h),
            self.visual.typography.action_font_fallback,
        )
        .action
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Result {
            result_type: self.type_,
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            extra_text: self.extra_text.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let action_changed = self.extra_text != next.extra_text;
        self.type_ = next.type_;
        self.title = next.title;
        self.subtitle = next.subtitle;
        self.extra_text = next.extra_text;
        self.visual = next.visual;
        if action_changed {
            self.last_action_rect.set(Rect::zero());
        }
        if self.extra_text.is_empty() {
            self.focused = false;
            self.pressed = false;
        }
    }
}

impl Default for ResultView {
    fn default() -> Self {
        Self::new(ResultType::Info)
    }
}

// 把 UIX 声明的共享视觉配置融合进结果页 Rust 交互与绘制内核。
fn build_result_view(mut kernel: ResultView, declared_visual: ResultVisual) -> ViewNode {
    let visual = UIX_RESULT_VISUAL.get_or_init(|| declared_visual);
    // 单一同目录 UIX 源在同一程序中必须保持一份确定配置。
    debug_assert_eq!(*visual, declared_visual);
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for ResultView {
    fn build(self) -> ViewNode {
        // UIX 拥有视觉配置，Rust 保留本地化、交互、布局算法和底层绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/result/result.uix")
    }
}

// 集中验证 UIX 声明壳与 Rust 内核的单节点契约。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/display/result__tests.rs"]
mod tests;
