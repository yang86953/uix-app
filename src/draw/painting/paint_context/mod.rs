//! PaintContext — 绘制上下文（Phase 3 迁入 draw）。
//!
//! 组件与自定义绘制只能通过此类型提交可录制命令；底层 Canvas 不对调用方暴露。

use std::ptr::NonNull;
use std::sync::Arc;

use crate::core::{Point, Rect, Size};
use crate::draw::Canvas2D;
use crate::draw::GradientDirection;
use crate::draw::debug::DebugRenderService;
use crate::draw::geometry::path::{FillRule, Path, PathBuilder};
use crate::draw::geometry::spatial::{AABB3D, Orientation, PhysicalUnit, SpatialContext, Vec3};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::painting::display_list::{DisplayList, PaintOp, PaintPass};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::font::text::TextRenderService;
use crate::draw::resources::image::{BitmapHandle, ImageService, blit_handle};
use crate::draw::{BlendMode, Color, FontHandle, Radius, Transform};

/// 绘制表面配置（组合 dpi/pr/orientation/size 减少参数传递）。
#[derive(Clone, Copy, Debug)]
pub struct PaintSurfaceConfig {
    /// 绘制表面的每英寸像素数，用于把物理单位换算为绘制坐标。
    pub dpi: f32,
    /// 逻辑像素到设备像素的缩放比。
    pub device_pixel_ratio: f32,
    /// 绘制表面采用的屏幕坐标轴方向。
    pub orientation: Orientation,
    /// 绘制表面在屏幕像素坐标系中的宽度。
    pub surface_w: i32,
    /// 绘制表面在屏幕像素坐标系中的高度。
    pub surface_h: i32,
}

/// 组件绘制入口，负责向当前表面提交或录制规范化绘制命令。
///
/// 上下文在一次绘制期间独占借用底层画布，并组合空间变换、文字、图片和调试绘制服务；
/// 组件无需也不能通过此类型访问具体渲染后端。
pub struct PaintContext<'a> {
    /// 3D 空间上下文（持有 Canvas2D 引用）。
    spatial: SpatialContext<'a>,

    /// 文本渲染服务。
    text: TextRenderService<'a>,

    /// 图片资源服务。
    image_service: &'a ImageService,

    /// 调试渲染服务。
    debug: DebugRenderService,

    /// 当前绘制阶段（合成器设置）。
    paint_pass: PaintPass,

    /// 可选：录制绘制指令到 DisplayList（独立生命周期，不绑定 canvas）。
    recorder: Option<NonNull<DisplayList>>,

    /// 是否向 recorder 写入（replay 时关闭）。
    record_ops: bool,

    /// 当前录制是否完整。
    recording_complete: bool,
}

/// 解析字号：如果指定了物理单位则通过当前 DPI 转换，否则返回 base。
///
/// widget render 中的标准用法：
pub fn resolve_font_size(base: f32, unit: Option<PhysicalUnit>, dpi: f32) -> f32 {
    unit.map(|u| u.to_dip(dpi)).unwrap_or(base)
}

mod frame_image;
mod shapes;
mod state;

pub(crate) use frame_image::blit_frame_image;
