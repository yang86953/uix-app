// 引入颜色面板几何所需的点与矩形类型。
use crate::core::{Point, Rect};

// 引入 UIX 生成的唯一颜色面板视觉表。
use super::COLOR_PICKER_VISUAL_REF;

// 复用浮层共享几何纯函数：归一化、相对/绝对互换、表面合并与翻转决策。
use crate::ui::widgets::overlay::{
    absolute_rect, local_rect, normalize_rect, resolve_vertical_dropdown_rect, surface_union_rect,
    vertical_fallback_surface,
};

// 描述任意实际颜色面板中绘制与命中共享的缩放指标。
#[derive(Debug, Clone, Copy)]
// 保存最终矩形、网格尺寸和视觉缩放比例。
pub(super) struct ColorPanelGeometry {
    // 保存归一化后的实际颜色面板矩形。
    popup: Rect,
    // 保存当前颜色数量。
    color_count: usize,
    // 保存缩放后的水平内边距。
    padding_x: f32,
    // 保存缩放后的垂直内边距。
    padding_y: f32,
    // 保存缩放后的色块槽宽度。
    cell_width: f32,
    // 保存缩放后的色块槽高度。
    cell_height: f32,
    // 保存圆角、描边与图标使用的较小轴比例。
    visual_scale: f32,
}

// 为实际颜色面板提供共享网格解析。
impl ColorPanelGeometry {
    // 从最终颜色面板矩形和颜色数量构造布局指标。
    pub(super) fn new(popup: Rect, color_count: usize) -> Self {
        let layout = COLOR_PICKER_VISUAL_REF.layout;
        // 归一化实际颜色面板矩形。
        let popup = normalize_color_rect(popup);
        // 计算当前颜色数量占用的自然行数。
        let rows = color_rows(color_count);
        // 计算当前颜色面板的自然宽度。
        let natural_width = natural_color_popup_width();
        // 计算当前颜色面板的自然高度。
        let natural_height = natural_color_popup_height(color_count);
        // 按自然宽度计算有限水平缩放比例。
        let x_scale = if natural_width > 0.0 {
            // 将放大请求限制为自然尺寸。
            (popup.w / natural_width).clamp(0.0, 1.0)
        // 处理退化自然宽度。
        } else {
            // 退化宽度不生成可见水平指标。
            0.0
        };
        // 按自然高度计算有限垂直缩放比例。
        let y_scale = if natural_height > 0.0 {
            // 将放大请求限制为自然尺寸。
            (popup.h / natural_height).clamp(0.0, 1.0)
        // 处理退化自然高度。
        } else {
            // 退化高度不生成可见垂直指标。
            0.0
        };
        // 返回绘制与命中共同使用的缩放指标。
        Self {
            // 保存最终颜色面板。
            popup,
            // 保存颜色数量用于索引边界。
            color_count,
            // 按实际宽度缩放水平内边距。
            padding_x: layout.panel_padding * x_scale,
            // 按实际高度缩放垂直内边距。
            padding_y: layout.panel_padding * y_scale,
            // 按实际宽度缩放八列色块槽。
            cell_width: layout.panel_cell * x_scale,
            // 按实际高度和实际行数缩放色块槽。
            cell_height: if rows > 0 {
                // 有颜色行时使用垂直缩放后的自然槽高。
                layout.panel_cell * y_scale
            // 处理空颜色面板。
            } else {
                // 空面板没有可命中的色块槽。
                0.0
            },
            // 较小轴比例确保视觉元素不会越出压缩后的色块。
            visual_scale: x_scale.min(y_scale),
        }
    }

    // 返回圆角、描边与图标使用的缩放比例。
    pub(super) fn visual_scale(self) -> f32 {
        // 返回已经限制到自然尺寸的较小轴比例。
        self.visual_scale
    }

    // 返回指定颜色索引的实际绘制矩形。
    pub(super) fn cell_rect(self, index: usize) -> Option<Rect> {
        // 越界索引或退化网格不生成绘制矩形。
        if index >= self.color_count || self.cell_width <= 0.0 || self.cell_height <= 0.0 {
            // 返回无色块。
            return None;
        }
        // 按固定八列计算颜色所在列。
        let columns = COLOR_PICKER_VISUAL_REF.layout.panel_columns.max(1);
        let column = index % columns;
        // 按固定八列计算颜色所在行。
        let row = index / columns;
        // 水平内缩随实际槽宽缩放。
        let inset_x = (COLOR_PICKER_VISUAL_REF.layout.cell_inset * self.visual_scale)
            .min(self.cell_width * 0.5);
        // 垂直内缩随实际槽高缩放。
        let inset_y = (COLOR_PICKER_VISUAL_REF.layout.cell_inset * self.visual_scale)
            .min(self.cell_height * 0.5);
        // 计算当前色块槽的左边。
        let x = self.popup.x + self.padding_x + column as f32 * self.cell_width;
        // 计算当前色块槽的顶边。
        let y = self.popup.y + self.padding_y + row as f32 * self.cell_height;
        // 返回扣除缩放内缩后的可见色块矩形。
        Some(Rect::new(
            // 左边避开槽边界。
            x + inset_x,
            // 顶边避开槽边界。
            y + inset_y,
            // 宽度不得收敛为负数。
            (self.cell_width - inset_x * 2.0).max(0.0),
            // 高度不得收敛为负数。
            (self.cell_height - inset_y * 2.0).max(0.0),
        ))
    }

    // 在实际缩放网格中解析指针命中的颜色索引。
    pub(super) fn index_at(self, position: Point) -> Option<usize> {
        // 退化网格不参与命中。
        if self.cell_width <= 0.0 || self.cell_height <= 0.0 {
            // 返回未命中。
            return None;
        }
        // 计算实际网格的左边。
        let content_x = self.popup.x + self.padding_x;
        // 计算实际网格的顶边。
        let content_y = self.popup.y + self.padding_y;
        // 计算固定八列网格的实际宽度。
        let columns = COLOR_PICKER_VISUAL_REF.layout.panel_columns.max(1);
        let content_width = columns as f32 * self.cell_width;
        // 计算当前颜色行数对应的实际高度。
        let content_height = color_rows(self.color_count) as f32 * self.cell_height;
        // 面板内边距或网格外部不命中颜色。
        if position.x < content_x
            // 检查右侧半开边界。
            || position.x >= content_x + content_width
            // 检查顶部边界。
            || position.y < content_y
            // 检查底部半开边界。
            || position.y >= content_y + content_height
        {
            // 返回未命中。
            return None;
        }
        // 按实际槽宽解析列号。
        let column = ((position.x - content_x) / self.cell_width).floor() as usize;
        // 按实际槽高解析行号。
        let row = ((position.y - content_y) / self.cell_height).floor() as usize;
        // 将行列转换为固定八列索引。
        let index = row * columns + column;
        // 最后一行空槽不得映射到不存在的颜色。
        (index < self.color_count).then_some(index)
    }
}

// 使用当前逻辑表面解析颜色面板的最终绝对矩形。
pub(super) fn resolve_color_popup_rect(frame: Rect, surface: Rect, color_count: usize) -> Rect {
    let layout = COLOR_PICKER_VISUAL_REF.layout;
    // 归一化触发器绝对布局矩形。
    let frame = normalize_color_rect(frame);
    // 归一化当前逻辑表面矩形。
    let surface = normalize_color_rect(surface);
    // 计算当前颜色数量对应的自然高度。
    let natural_height = natural_color_popup_height(color_count);
    // 复用共享垂直下拉解析：保留间隙、优先向下、放不下翻向上、两侧不足取大侧。
    // 颜色网格的自然宽度由固定八列与内边距声明。
    resolve_vertical_dropdown_rect(
        frame,
        surface,
        natural_color_popup_width(),
        natural_height,
        frame.y + frame.h,
        layout.panel_gap,
    )
}

// 将绝对颜色面板转换为相对触发器原点的矩形。
pub(super) fn local_color_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 复用共享相对转换。
    local_rect(frame, popup)
}

// 将相对颜色面板转换为窗口绝对矩形。
pub(super) fn absolute_color_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 复用共享绝对转换。
    absolute_rect(frame, popup)
}

// 计算触发器与颜色面板共同占用的表面内矩形。
pub(super) fn color_surface_rect(frame: Rect, popup: Rect, surface: Rect) -> Rect {
    // 复用共享表面合并。
    surface_union_rect(frame, popup, surface)
}

// 构造首次正式登记前的有限回退表面。
pub(super) fn color_fallback_surface(frame: Rect, color_count: usize) -> Rect {
    let layout = COLOR_PICKER_VISUAL_REF.layout;
    // 计算当前颜色面板自然宽度。
    let width = natural_color_popup_width();
    // 计算当前颜色面板自然高度。
    let height = natural_color_popup_height(color_count);
    // 复用共享回退表面：在触发器上下各预留完整颜色面板和间隙。
    vertical_fallback_surface(
        frame,
        width,
        height,
        layout.panel_gap,
        layout.fallback_panel_sides,
    )
}

// 返回当前颜色数量占用的固定八列行数。
fn color_rows(color_count: usize) -> usize {
    // 使用向上取整保留最后一行不完整色块。
    color_count.div_ceil(COLOR_PICKER_VISUAL_REF.layout.panel_columns.max(1))
}

// 返回颜色面板的自然宽度。
fn natural_color_popup_width() -> f32 {
    // 合并八列自然槽宽与两侧内边距。
    let layout = COLOR_PICKER_VISUAL_REF.layout;
    layout.panel_columns as f32 * layout.panel_cell
        + layout.panel_padding * layout.fallback_panel_sides
}

// 返回当前颜色数量对应的自然高度。
fn natural_color_popup_height(color_count: usize) -> f32 {
    // 合并自然行高与上下内边距。
    let layout = COLOR_PICKER_VISUAL_REF.layout;
    color_rows(color_count) as f32 * layout.panel_cell
        + layout.panel_padding * layout.fallback_panel_sides
}

// 归一化颜色面板相关矩形。
pub(super) fn normalize_color_rect(rect: Rect) -> Rect {
    // 复用浮层共享归一化：非有限坐标回退原点、尺寸收敛为有限非负。
    normalize_rect(rect)
}
