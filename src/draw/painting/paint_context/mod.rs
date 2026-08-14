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
    pub dpi: f32,
    pub device_pixel_ratio: f32,
    pub orientation: Orientation,
    pub surface_w: i32,
    pub surface_h: i32,
}

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
/// ```ignore
/// let fs = resolve_font_size(self.font_size, self.font_size_unit, ctx.dpi());
/// ```
pub fn resolve_font_size(base: f32, unit: Option<PhysicalUnit>, dpi: f32) -> f32 {
    unit.map(|u| u.to_dip(dpi)).unwrap_or(base)
}

mod shapes;
mod state;
