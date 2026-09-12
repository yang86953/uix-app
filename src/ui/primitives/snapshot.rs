//! Snapshots for framework primitives, independent of widget-library enums.
use crate::{
    draw::{Color, integration::geometry::spatial::PhysicalUnit},
    ui::{
        AccessibilityRole, AccessibilitySnapshot, ScrollDirection, SnapshotModel, Style,
        WidgetSnapshotFields,
    },
};
#[derive(Debug, Clone, PartialEq)]
pub enum PrimitiveSnapshot {
    Container {
        /// 容器当前使用的盒模型与视觉样式。
        style: Style,
    },
    Label {
        /// 标签显示的文字。
        text: String,
        /// 标签声明的基础字号。
        font_size: f32,
        /// 字号使用的可选物理单位。
        font_size_unit: Option<PhysicalUnit>,
        /// 标签显式声明的文字颜色。
        color: Option<Color>,
        /// 标签显式声明的固定宽度。
        fixed_width: Option<f32>,
        /// 标签显式声明的固定高度。
        fixed_height: Option<f32>,
        /// 标签可选的完整样式覆写。
        style: Option<Style>,
    },
    ScrollView {
        /// 滚动视图允许滚动的轴向。
        direction: ScrollDirection,
        /// 滚动视图显式声明的固定宽度。
        fixed_width: Option<f32>,
        /// 滚动视图显式声明的固定高度。
        fixed_height: Option<f32>,
        /// 滚动视图参与父级弹性增长的权重。
        flex_grow: f32,
        /// 滚动视图参与父级弹性收缩的权重。
        flex_shrink: f32,
        /// 是否显示可见滚动条。
        show_scrollbar: bool,
        /// 当前横向滚动偏移。
        scroll_x: f32,
        /// 当前纵向滚动偏移。
        scroll_y: f32,
    },
}
impl From<PrimitiveSnapshot> for WidgetSnapshotFields {
    fn from(value: PrimitiveSnapshot) -> Self {
        Self::new(value)
    }
}
impl SnapshotModel for PrimitiveSnapshot {
    fn text(&self) -> Option<String> {
        if let Self::Label { text, .. } = self {
            Some(text.clone())
        } else {
            None
        }
    }
    fn accessibility(&self) -> AccessibilitySnapshot {
        if let Self::Label { text, .. } = self {
            AccessibilitySnapshot::named(AccessibilityRole::Text, text.clone())
        } else {
            AccessibilitySnapshot::new(AccessibilityRole::Generic)
        }
    }
    fn config_changed(&self, next: &Self) -> bool {
        if let (
            Self::ScrollView {
                direction: a,
                fixed_width: b,
                fixed_height: c,
                flex_grow: d,
                flex_shrink: e,
                show_scrollbar: f,
                ..
            },
            Self::ScrollView {
                direction: g,
                fixed_width: h,
                fixed_height: i,
                flex_grow: j,
                flex_shrink: k,
                show_scrollbar: l,
                ..
            },
        ) = (self, next)
        {
            (a, b, c, d, e, f) != (g, h, i, j, k, l)
        } else {
            self != next
        }
    }
    fn layout_changed(&self, next: &Self) -> bool {
        match (self, next) {
            (Self::Container { style: a }, Self::Container { style: b }) => {
                style_layout_changed(a, b)
            }
            (
                Self::Label {
                    text: a,
                    font_size: b,
                    font_size_unit: c,
                    fixed_width: d,
                    fixed_height: e,
                    style: f,
                    ..
                },
                Self::Label {
                    text: g,
                    font_size: h,
                    font_size_unit: i,
                    fixed_width: j,
                    fixed_height: k,
                    style: l,
                    ..
                },
            ) => (a, b, c, d, e) != (g, h, i, j, k) || optional_style_layout_changed(f, l),
            _ => self.config_changed(next),
        }
    }
}
fn style_layout_changed(current: &Style, next: &Style) -> bool {
    // 盒模型字段直接改变内容区域或父级占位。
    current.margin != next.margin
        || current.padding != next.padding
        || current.border_width != next.border_width
        // 固定尺寸字段改变组件约束结果。
        || current.width != next.width
        || current.height != next.height
        // 容器布局字段改变 Flex 或 Grid 求解结果。
        || current.display != next.display
        || current.flex_direction != next.flex_direction
        || current.flex_wrap != next.flex_wrap
        || current.overflow_content != next.overflow_content
        || current.justify_content != next.justify_content
        || current.align_items != next.align_items
        || current.gap != next.gap
        || current.grid_template_columns != next.grid_template_columns
        || current.grid_template_rows != next.grid_template_rows
        || current.grid_column_gap != next.grid_column_gap
        || current.grid_row_gap != next.grid_row_gap
        // 子项布局字段改变父容器对当前节点的分配。
        || current.flex_grow != next.flex_grow
        || current.flex_shrink != next.flex_shrink
        || current.align_self != next.align_self
        || current.grid_cell != next.grid_cell
        || current.grid_column_span != next.grid_column_span
        || current.grid_row_span != next.grid_row_span
        // 字号参与文本组件的固有尺寸测量。
        || current.font_size != next.font_size
        // 显式行高参与文本行盒与多行测量。
        || current.line_height != next.line_height
        // 可见性决定节点是否参与布局。
        || current.visible != next.visible
}

// 比较可选样式；样式是否存在会改变 Label 的字号回退语义。
fn optional_style_layout_changed(current: &Option<Style>, next: &Option<Style>) -> bool {
    // 同时存在时只比较布局相关字段，同时缺失时保持布局稳定。
    match (current, next) {
        // 两份样式使用统一的几何字段比较。
        (Some(current), Some(next)) => style_layout_changed(current, next),
        // 两边都没有样式时没有布局变化。
        (None, None) => false,
        // 样式出现或消失会改变 Label 的字号与尺寸回退。
        _ => true,
    }
}
