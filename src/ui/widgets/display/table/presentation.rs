//! Table 的 UIX 静态视觉契约与每帧主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

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

// 同目录 UIX 生成表格全部分组视觉、根记录及稳定借用。
crate::uix_items!("src/ui/widgets/display/table/table.uix");

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

// 限制 UIX 声明的加载圆点数量，保持动画步进除数为正。
pub(crate) const fn table_loading_dot_count(value: usize) -> usize {
    if value == 0 { 1 } else { value }
}

// 向 UIX 提供主题语义角色。
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
