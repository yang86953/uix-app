//! 在多列联动浮层中选择层级路径的级联选择器。

use crate::core::{Point, Rect};
use crate::draw::Color;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
/// 级联选择器中的一个树形选项。
pub struct CascaderOption {
    /// 选项显示文本。
    pub label: String,
    /// 选项稳定值。
    pub value: String,
    /// 下一层子选项。
    pub children: Vec<CascaderOption>,
    /// 该选项是否禁止选择。
    pub disabled: bool,
}

impl CascaderOption {
    /// 创建没有子项且可选择的级联选项。
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            children: vec![],
            disabled: false,
        }
    }

    /// 替换该选项的全部子项。
    pub fn children(mut self, children: Vec<CascaderOption>) -> Self {
        self.children = children;
        self
    }

    /// 设置该选项是否禁止选择。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
/// 一条已选择级联路径的显示文本和值。
pub struct CascaderValue {
    /// 从根到叶的显示文本路径。
    pub labels: Vec<String>,
    /// 从根到叶的稳定值路径。
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CascaderSearchResult {
    value: CascaderValue,
    disabled: bool,
    loading: bool,
}

mod methods;
mod presentation;
mod widget;

use presentation::*;
pub use widget::*;

fn collect_search_results(
    options: &[CascaderOption],
    loading_children: &HashSet<String>,
    query: &str,
    path: &mut CascaderValue,
    ancestor_disabled: bool,
    results: &mut Vec<CascaderSearchResult>,
) {
    for option in options {
        path.labels.push(option.label.clone());
        path.values.push(option.value.clone());
        let disabled = ancestor_disabled || option.disabled;
        let loading = loading_children.contains(&option.value);
        if loading || option.children.is_empty() {
            let searchable_path = path.labels.join(" / ").to_lowercase();
            if searchable_path.contains(query) {
                results.push(CascaderSearchResult {
                    value: path.clone(),
                    disabled,
                    loading,
                });
            }
        } else {
            collect_search_results(
                &option.children,
                loading_children,
                query,
                path,
                disabled,
                results,
            );
        }
        path.labels.pop();
        path.values.pop();
    }
}

fn byte_index_for_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

fn paint_loading_spinner(ctx: &mut PaintContext, row: Rect, phase: f32, color: Color) {
    let layout = CASCADER_VISUAL_REF.layout;
    let slot = Rect::new(
        row.x + row.w - layout.trailing_slot_width,
        row.y,
        layout.trailing_slot_width,
        row.h,
    );
    let radius = layout
        .loading_radius
        .min(slot.w.min(slot.h) * layout.loading_radius_ratio);
    if radius > 0.0 {
        ctx.stroke_arc(
            slot.x + slot.w * 0.5,
            slot.y + slot.h * 0.5,
            radius,
            phase,
            phase + std::f32::consts::PI * layout.loading_arc_pi,
            color,
            layout.loading_stroke_width,
        );
    }
}

fn first_enabled_index(options: &[CascaderOption]) -> Option<usize> {
    options.iter().position(|option| !option.disabled)
}

fn next_enabled_index(options: &[CascaderOption], current: usize, forward: bool) -> Option<usize> {
    let len = options.len();
    if len == 0 {
        return None;
    }

    (1..=len)
        .map(|step| {
            if forward {
                (current + step) % len
            } else {
                (current + len - (step % len)) % len
            }
        })
        .find(|index| !options[*index].disabled)
}

// 保存级联弹层最终矩形与按实际总宽均分后的列宽。
#[derive(Debug, Clone, Copy, PartialEq)]
// 该结构只在级联选择模块内部传递共享几何。
pub(crate) struct CascaderPopupGeometry {
    // 使用窗口绝对逻辑坐标保存弹层矩形。
    rect: Rect,
    // 保存每列实际可绘制与命中的宽度。
    column_width: f32,
}

// 使用当前逻辑表面解析级联弹层最终几何。
fn resolve_cascader_popup_geometry(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收当前可见列数。
    level_count: usize,
    // 接收当前窗口逻辑表面。
    surface: Rect,
    // 返回限制在表面内的弹层与列宽。
) -> CascaderPopupGeometry {
    let layout = CASCADER_VISUAL_REF.layout;
    // 归一化触发器矩形。
    let frame = normalize_cascader_rect(frame);
    // 归一化逻辑表面矩形。
    let surface = normalize_cascader_rect(surface);
    // 空列集合仍按单列弹层处理。
    let level_count = level_count.max(1);
    // 计算既有规格要求的自然列宽。
    let natural_column_width = frame.w.max(layout.popup_column_min_width);
    // 计算所有可见列的自然总宽度。
    let natural_width = finite_nonnegative(natural_column_width * level_count as f32);
    // 将总宽限制在当前表面内。
    let width = natural_width.min(surface.w).max(0.0);
    // 无可用表面时返回稳定空几何。
    if width <= 0.0 || surface.h <= 0.0 {
        // 返回零矩形与零列宽。
        return CascaderPopupGeometry {
            // 空弹层不参与绘制和命中。
            rect: Rect::zero(),
            // 空弹层没有可用列宽。
            column_width: 0.0,
        };
    }

    // 计算横向起点允许的最大绝对值。
    let max_x = surface.x + surface.w - width;
    // 将触发器锚点横向收敛到当前表面。
    let x = frame.x.clamp(surface.x, max_x);
    // 计算带间距的控件下方可用高度。
    let available_below = (surface.y + surface.h - frame.y - frame.h - layout.popup_gap).max(0.0);
    // 计算带间距的控件上方可用高度。
    let available_above = (frame.y - layout.popup_gap - surface.y).max(0.0);
    // 优先完整向下；否则完整向上；两侧都不足时选择更大空间。
    let place_below = if layout.popup_height <= available_below {
        // 下方完整容纳固定自然高度时保持默认方向。
        true
    } else if layout.popup_height <= available_above {
        // 只有上方完整容纳时翻转。
        false
    } else {
        // 两侧都不足时选择空间更大的一侧，平局保持向下。
        available_below >= available_above
    };
    // 读取最终方向的实际可用高度。
    let available_height = if place_below {
        // 使用触发器下方空间。
        available_below
    } else {
        // 使用触发器上方空间。
        available_above
    };
    // 将自然高度限制在最终方向的可用空间内。
    let height = layout.popup_height.min(available_height).max(0.0);
    // 计算最终绝对纵坐标。
    let y = if place_below {
        // 向下弹层保留既有二像素间距。
        frame.y + frame.h + layout.popup_gap
    } else {
        // 向上弹层用实际高度紧贴触发器上方间距。
        frame.y - layout.popup_gap - height
    };

    // 返回所有消费者共享的最终几何。
    CascaderPopupGeometry {
        // 保存受表面约束的绝对弹层矩形。
        rect: Rect::new(x, y, width, height),
        // 多列在最终总宽内等分，避免任一列越出弹层。
        column_width: width / level_count as f32,
    }
}

// 将绝对级联弹层几何转换为相对触发器原点的缓存。
fn local_cascader_popup_geometry(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收已经解析的绝对弹层几何。
    geometry: CascaderPopupGeometry,
    // 返回可供组件本地事件复用的几何。
) -> CascaderPopupGeometry {
    // 归一化触发器以保证偏移有限。
    let frame = normalize_cascader_rect(frame);
    // 返回相对矩形并保留实际列宽。
    CascaderPopupGeometry {
        // 从绝对弹层坐标扣除触发器原点。
        rect: Rect::new(
            // 保存横向相对偏移。
            geometry.rect.x - frame.x,
            // 保存纵向相对偏移。
            geometry.rect.y - frame.y,
            // 保留最终总宽。
            geometry.rect.w,
            // 保留最终高度。
            geometry.rect.h,
        ),
        // 保留最终列宽。
        column_width: geometry.column_width,
    }
}

// 将相对触发器的弹层矩形转换为窗口绝对坐标。
fn absolute_cascader_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 叠加触发器原点并保留最终尺寸。
    Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
}

// 计算触发器、实际弹层与保守弹层共同占用的表面内脏区。
fn cascader_dirty_rect(frame: Rect, popup: Rect, surface: Rect) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_cascader_rect(frame);
    // 归一化逻辑表面矩形。
    let surface = normalize_cascader_rect(surface);
    // 将相对弹层转换为窗口绝对坐标。
    let popup = absolute_cascader_popup_rect(frame, popup);
    // 合并触发器和所有可能的弹层区域。
    frame
        // 将实际或保守弹层并入脏区。
        .union(&popup)
        // 裁掉逻辑表面外不可见区域。
        .intersect(&surface)
        // 完全不相交时返回空脏区。
        .unwrap_or_default()
}

// 构造尚未取得真实窗口表面时的有限回退表面。
fn cascader_fallback_surface(frame: Rect, level_count: usize) -> Rect {
    let layout = CASCADER_VISUAL_REF.layout;
    // 归一化触发器矩形。
    let frame = normalize_cascader_rect(frame);
    // 空列集合仍按单列自然宽处理。
    let level_count = level_count.max(1);
    // 计算所有列完整展示所需的自然宽度。
    let width = finite_nonnegative(
        // 使用既有最小列宽规格。
        frame.w.max(layout.popup_column_min_width) * level_count as f32,
    );
    // 在触发器上下各预留一份自然弹层空间。
    Rect::new(
        // 横向从触发器左边开始。
        frame.x,
        // 纵向向上预留间距与完整弹层高度。
        frame.y - layout.popup_gap - layout.popup_height,
        // 保留所有自然列宽。
        width,
        // 覆盖上下两份弹层、两份间距和触发器。
        layout.popup_height * layout.fallback_popup_sides
            + layout.popup_gap * layout.fallback_popup_sides
            + frame.h,
    )
}

// 归一化级联选择相关矩形。
fn normalize_cascader_rect(rect: Rect) -> Rect {
    // 替换非有限坐标并收敛负尺寸。
    Rect::new(
        // 非有限横坐标回退到原点。
        if rect.x.is_finite() { rect.x } else { 0.0 },
        // 非有限纵坐标回退到原点。
        if rect.y.is_finite() { rect.y } else { 0.0 },
        // 归一化宽度。
        finite_nonnegative(rect.w),
        // 归一化高度。
        finite_nonnegative(rect.h),
    )
}

// 将任意浮点尺寸收敛为有限非负值。
fn finite_nonnegative(value: f32) -> f32 {
    // 只保留有限输入。
    if value.is_finite() {
        // 负值收敛为零。
        value.max(0.0)
    } else {
        // 非有限值回退为零。
        0.0
    }
}

fn point_in_half_open_rect(rect: Rect, point: Point) -> bool {
    point.x >= rect.x && point.x < rect.x + rect.w && point.y >= rect.y && point.y < rect.y + rect.h
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

// 把 Cascader Rust 交互内核与 UIX 静态视觉组合为单一组件节点。
fn build_cascader_view(mut kernel: Cascader, visual: &'static CascaderVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_cascader_uix_root(kernel: Cascader) -> ViewNode {
    crate::uix!("src/ui/widgets/input/cascader/cascader.uix")
}

impl View for Cascader {
    fn build(self) -> ViewNode {
        build_cascader_uix_root(self)
    }
}
