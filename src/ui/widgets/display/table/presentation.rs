//! Table 的 UIX 静态视觉契约与每帧主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use std::sync::OnceLock;

// 保存由 UIX 声明的固有尺寸、行列与交互几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TableGeometryVisual {
    pub(crate) default_width: f32,
    pub(crate) default_height: f32,
    pub(crate) min_height: f32,
    pub(crate) row_height: f32,
    pub(crate) min_row_height: f32,
    pub(crate) header_height: f32,
    pub(crate) expand_height: f32,
    pub(crate) body_separator: f32,
    pub(crate) wheel_step: f32,
    pub(crate) selection_width: f32,
    pub(crate) resize_handle_half_width: f32,
    pub(crate) min_resizable_column_width: f32,
}

// 保存由 UIX 声明的外框、空态、单元格、选择列、展开入口和焦点视觉。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TableFrameVisual {
    pub(crate) radius_frame_ratio: f32,
    pub(crate) border_width: f32,
    pub(crate) empty_inset: f32,
    pub(crate) empty_inset_ratio: f32,
    pub(crate) empty_font_size: f32,
    pub(crate) cell_inset: f32,
    pub(crate) cell_inset_ratio: f32,
    pub(crate) cell_font_size: f32,
    pub(crate) selection_icon_x: f32,
    pub(crate) selection_icon_width: f32,
    pub(crate) selection_icon_size: f32,
    pub(crate) selection_divider_width: f32,
    pub(crate) expand_icon_size: f32,
    pub(crate) expand_icon_right: f32,
    pub(crate) expand_icon_font_size: f32,
    pub(crate) center_ratio: f32,
    pub(crate) expanded_icon: &'static str,
    pub(crate) collapsed_icon: &'static str,
    pub(crate) checked_icon: &'static str,
    pub(crate) unchecked_icon: &'static str,
    pub(crate) focus_inset: f32,
    pub(crate) focus_stroke: f32,
}

// 保存由 UIX 声明的表头文字、排序槽、分隔线与调整列宽视觉。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TableHeaderVisual {
    pub(crate) horizontal_inset: f32,
    pub(crate) horizontal_inset_ratio: f32,
    pub(crate) title_font_size: f32,
    pub(crate) sort_slot: f32,
    pub(crate) sort_slot_ratio: f32,
    pub(crate) unsorted_icon: &'static str,
    pub(crate) ascending_icon: &'static str,
    pub(crate) descending_icon: &'static str,
    pub(crate) unsorted_icon_size: f32,
    pub(crate) sorted_icon_size: f32,
    pub(crate) sort_icon_width: f32,
    pub(crate) sort_icon_right: f32,
    pub(crate) sort_icon_right_ratio: f32,
    pub(crate) divider_width: f32,
    pub(crate) resize_indicator_width: f32,
    pub(crate) resize_active_indicator_width: f32,
}

// 保存由 UIX 声明的分页布局、图标与排版。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TablePaginationVisual {
    pub(crate) height: f32,
    pub(crate) item_size: f32,
    pub(crate) gap: f32,
    pub(crate) label_width: f32,
    pub(crate) inset: f32,
    pub(crate) inset_ratio: f32,
    pub(crate) gap_ratio: f32,
    pub(crate) label_width_ratio: f32,
    pub(crate) center_ratio: f32,
    pub(crate) divider_width: f32,
    pub(crate) button_stroke: f32,
    pub(crate) previous_icon: &'static str,
    pub(crate) next_icon: &'static str,
    pub(crate) icon_size: f32,
    pub(crate) label_font_size: f32,
}

// 保存由 UIX 声明的加载遮罩与转圈动画常量。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TableLoadingVisual {
    pub(crate) overlay_alpha: u8,
    pub(crate) radius: f32,
    pub(crate) radius_ratio: f32,
    pub(crate) dot_radius_ratio: f32,
    pub(crate) extent_padding: f32,
    pub(crate) dot_count: usize,
    pub(crate) step_sin: f32,
    pub(crate) step_cos: f32,
    pub(crate) opacity_base: f32,
    pub(crate) opacity_range: f32,
    pub(crate) duration_seconds: f32,
}

// Table 外框与分页按钮使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableRadiusRole {
    Small,
}

impl TableRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存由 UIX 声明的全部主题语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TablePaletteVisual {
    background: ColorValue,
    alternate_background: ColorValue,
    header_background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    primary: ColorValue,
    hover_background: ColorValue,
    selected_background: ColorValue,
    pressed_background: ColorValue,
    radius: TableRadiusRole,
}

// 全部 Table 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TableVisual {
    pub(crate) geometry: TableGeometryVisual,
    pub(crate) frame: TableFrameVisual,
    pub(crate) header: TableHeaderVisual,
    pub(crate) pagination: TablePaginationVisual,
    pub(crate) loading: TableLoadingVisual,
    palette: TablePaletteVisual,
}

// 保存 Table 每帧只解析一次的主题颜色与圆角。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedTableVisual {
    pub(crate) background: Color,
    pub(crate) alternate_background: Color,
    pub(crate) header_background: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) primary: Color,
    pub(crate) hover_background: Color,
    pub(crate) selected_background: Color,
    pub(crate) pressed_background: Color,
    pub(crate) radius: f32,
}

impl TableVisual {
    pub(crate) fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedTableVisual {
        ResolvedTableVisual {
            background: self.palette.background.resolve(tokens),
            alternate_background: self.palette.alternate_background.resolve(tokens),
            header_background: self.palette.header_background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            hover_background: self.palette.hover_background.resolve(tokens),
            selected_background: self.palette.selected_background.resolve(tokens),
            pressed_background: self.palette.pressed_background.resolve(tokens),
            radius: self.palette.radius.resolve(tokens),
        }
    }
}

// 分页标签复用内部 String，页码未变化时不产生临时分配。
#[derive(Debug, Default)]
pub(crate) struct TablePaginationLabelCache {
    pub(crate) current: usize,
    pub(crate) total: usize,
    pub(crate) value: String,
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn table_geometry_visual(
    default_width: f32,
    default_height: f32,
    min_height: f32,
    row_height: f32,
    min_row_height: f32,
    header_height: f32,
    expand_height: f32,
    body_separator: f32,
    wheel_step: f32,
    selection_width: f32,
    resize_handle_half_width: f32,
    min_resizable_column_width: f32,
) -> TableGeometryVisual {
    TableGeometryVisual {
        default_width,
        default_height,
        min_height,
        row_height,
        min_row_height,
        header_height,
        expand_height,
        body_separator,
        wheel_step,
        selection_width,
        resize_handle_half_width,
        min_resizable_column_width,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn table_frame_visual(
    radius_frame_ratio: f32,
    border_width: f32,
    empty_inset: f32,
    empty_inset_ratio: f32,
    empty_font_size: f32,
    cell_inset: f32,
    cell_inset_ratio: f32,
    cell_font_size: f32,
    selection_icon_x: f32,
    selection_icon_width: f32,
    selection_icon_size: f32,
    selection_divider_width: f32,
    expand_icon_size: f32,
    expand_icon_right: f32,
    expand_icon_font_size: f32,
    center_ratio: f32,
    expanded_icon: &'static str,
    collapsed_icon: &'static str,
    checked_icon: &'static str,
    unchecked_icon: &'static str,
    focus_inset: f32,
    focus_stroke: f32,
) -> TableFrameVisual {
    TableFrameVisual {
        radius_frame_ratio,
        border_width,
        empty_inset,
        empty_inset_ratio,
        empty_font_size,
        cell_inset,
        cell_inset_ratio,
        cell_font_size,
        selection_icon_x,
        selection_icon_width,
        selection_icon_size,
        selection_divider_width,
        expand_icon_size,
        expand_icon_right,
        expand_icon_font_size,
        center_ratio,
        expanded_icon,
        collapsed_icon,
        checked_icon,
        unchecked_icon,
        focus_inset,
        focus_stroke,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn table_header_visual(
    horizontal_inset: f32,
    horizontal_inset_ratio: f32,
    title_font_size: f32,
    sort_slot: f32,
    sort_slot_ratio: f32,
    unsorted_icon: &'static str,
    ascending_icon: &'static str,
    descending_icon: &'static str,
    unsorted_icon_size: f32,
    sorted_icon_size: f32,
    sort_icon_width: f32,
    sort_icon_right: f32,
    sort_icon_right_ratio: f32,
    divider_width: f32,
    resize_indicator_width: f32,
    resize_active_indicator_width: f32,
) -> TableHeaderVisual {
    TableHeaderVisual {
        horizontal_inset,
        horizontal_inset_ratio,
        title_font_size,
        sort_slot,
        sort_slot_ratio,
        unsorted_icon,
        ascending_icon,
        descending_icon,
        unsorted_icon_size,
        sorted_icon_size,
        sort_icon_width,
        sort_icon_right,
        sort_icon_right_ratio,
        divider_width,
        resize_indicator_width,
        resize_active_indicator_width,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn table_pagination_visual(
    height: f32,
    item_size: f32,
    gap: f32,
    label_width: f32,
    inset: f32,
    inset_ratio: f32,
    gap_ratio: f32,
    label_width_ratio: f32,
    center_ratio: f32,
    divider_width: f32,
    button_stroke: f32,
    previous_icon: &'static str,
    next_icon: &'static str,
    icon_size: f32,
    label_font_size: f32,
) -> TablePaginationVisual {
    TablePaginationVisual {
        height,
        item_size,
        gap,
        label_width,
        inset,
        inset_ratio,
        gap_ratio,
        label_width_ratio,
        center_ratio,
        divider_width,
        button_stroke,
        previous_icon,
        next_icon,
        icon_size,
        label_font_size,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn table_loading_visual(
    overlay_alpha: f32,
    radius: f32,
    radius_ratio: f32,
    dot_radius_ratio: f32,
    extent_padding: f32,
    dot_count: f32,
    step_sin: f32,
    step_cos: f32,
    opacity_base: f32,
    opacity_range: f32,
    duration_seconds: f32,
) -> TableLoadingVisual {
    let dot_count = dot_count as usize;
    TableLoadingVisual {
        overlay_alpha: overlay_alpha as u8,
        radius,
        radius_ratio,
        dot_radius_ratio,
        extent_padding,
        dot_count: if dot_count == 0 { 1 } else { dot_count },
        step_sin,
        step_cos,
        opacity_base,
        opacity_range,
        duration_seconds,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn table_palette_visual(
    background: ColorValue,
    alternate_background: ColorValue,
    header_background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    primary: ColorValue,
    hover_background: ColorValue,
    selected_background: ColorValue,
    pressed_background: ColorValue,
    radius: TableRadiusRole,
) -> TablePaletteVisual {
    TablePaletteVisual {
        background,
        alternate_background,
        header_background,
        border,
        text,
        text_secondary,
        primary,
        hover_background,
        selected_background,
        pressed_background,
        radius,
    }
}

pub(crate) const fn table_visual(
    geometry: TableGeometryVisual,
    frame: TableFrameVisual,
    header: TableHeaderVisual,
    pagination: TablePaginationVisual,
    loading: TableLoadingVisual,
    palette: TablePaletteVisual,
) -> TableVisual {
    TableVisual {
        geometry,
        frame,
        header,
        pagination,
        loading,
        palette,
    }
}

// 向 UIX 提供受限表达式不能直接书写的图标与主题角色。
pub(crate) const fn table_expanded_icon() -> &'static str {
    "chevron-up"
}
pub(crate) const fn table_collapsed_icon() -> &'static str {
    "chevron-down"
}
pub(crate) const fn table_checked_icon() -> &'static str {
    "check-square"
}
pub(crate) const fn table_unchecked_icon() -> &'static str {
    "square"
}
pub(crate) const fn table_unsorted_icon() -> &'static str {
    "chevron-down"
}
pub(crate) const fn table_ascending_icon() -> &'static str {
    "chevron-up"
}
pub(crate) const fn table_descending_icon() -> &'static str {
    "chevron-down"
}
pub(crate) const fn table_previous_icon() -> &'static str {
    "chevron-left"
}
pub(crate) const fn table_next_icon() -> &'static str {
    "chevron-right"
}
pub(crate) const fn table_small_radius() -> TableRadiusRole {
    TableRadiusRole::Small
}
pub(crate) const fn table_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn table_alternate_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn table_header_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn table_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn table_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn table_secondary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn table_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn table_hover_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillQuaternary)
}
pub(crate) const fn table_selected_background_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBg)
}
pub(crate) const fn table_pressed_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}

pub(crate) static DEFAULT_TABLE_VISUAL: TableVisual = table_visual(
    table_geometry_visual(
        400.0, 300.0, 60.0, 28.0, 1.0, 32.0, 60.0, 1.0, 40.0, 32.0, 4.0, 32.0,
    ),
    table_frame_visual(
        0.5,
        1.0,
        16.0,
        0.25,
        14.0,
        8.0,
        0.25,
        12.0,
        6.0,
        18.0,
        14.0,
        1.0,
        18.0,
        6.0,
        10.0,
        0.5,
        "chevron-up",
        "chevron-down",
        "check-square",
        "square",
        1.0,
        2.0,
    ),
    table_header_visual(
        8.0,
        0.25,
        13.0,
        24.0,
        0.4,
        "chevron-down",
        "chevron-up",
        "chevron-down",
        10.0,
        11.0,
        18.0,
        4.0,
        0.25,
        1.0,
        1.0,
        2.0,
    ),
    table_pagination_visual(
        40.0,
        28.0,
        4.0,
        80.0,
        8.0,
        0.1,
        0.05,
        0.5,
        0.5,
        1.0,
        1.0,
        "chevron-left",
        "chevron-right",
        14.0,
        13.0,
    ),
    table_loading_visual(
        30.0, 10.0, 0.3, 0.18, 1.0, 8.0, 0.70710677, 0.70710677, 0.25, 0.75, 0.8,
    ),
    table_palette_visual(
        ColorValue::Neutral(NeutralRole::BgElevated),
        ColorValue::Neutral(NeutralRole::BgContainer),
        ColorValue::Neutral(NeutralRole::FillTertiary),
        ColorValue::Neutral(NeutralRole::Border),
        ColorValue::Neutral(NeutralRole::Text),
        ColorValue::Neutral(NeutralRole::TextSecondary),
        ColorValue::Palette(PaletteColor::Primary),
        ColorValue::Neutral(NeutralRole::FillQuaternary),
        ColorValue::Palette(PaletteColor::PrimaryBg),
        ColorValue::Neutral(NeutralRole::FillSecondary),
        TableRadiusRole::Small,
    ),
);

pub(crate) static UIX_TABLE_VISUAL: OnceLock<TableVisual> = OnceLock::new();
