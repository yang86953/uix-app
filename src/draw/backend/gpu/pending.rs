//! GPU-native 待决命令载荷 — gpu 子模块。
//!
//! `Canvas2D` 调用被转换为 [`PendingNativeOp`] 载荷入队，present 时由
//! 通用 RHI lowering 保序下发；载荷携带裁剪与变换折叠后的设备几何。

use crate::core::Rect;
use crate::draw::geometry::types::{BlendMode, Transform};
// 引入所属 graphics backend Module 的全部排队原语。
use super::{
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLineSegment, GpuLinearGradientRect,
    GpuRadialGradient, GpuSector, GpuSolidMesh, GpuSolidRect, GpuStrokeRect,
};

pub(crate) struct StateSnapshot {
    pub(crate) clip_rect: Rect,
    // 保存矩形/路径裁剪栈长度，restore 时回退到保存边界。
    pub(crate) clip_stack_len: usize,
    // 保存每个裁剪栈项是否为路径裁剪。
    pub(crate) clip_kind_stack_len: usize,
    // 标记 save 时是否已经存在 soft renderer 状态栈。
    pub(crate) soft_was_present: bool,
    pub(crate) opacity: f32,
    pub(crate) offset_x: f32,
    pub(crate) offset_y: f32,
    pub(crate) transform: Transform,
    pub(crate) blend_mode: BlendMode,
}
pub(crate) struct PendingNativeRect {
    pub(crate) rect: GpuSolidRect,
    // 保留通用 RHI shape pipeline 需要的目标混合语义。
    pub(crate) additive: bool,
    /// Logical scissor AABB (x, y, w, h).
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeStroke {
    pub(crate) rect: GpuStrokeRect,
    // 保留通用 RHI shape pipeline 需要的目标混合语义。
    pub(crate) additive: bool,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeGlyph {
    pub(crate) glyph: GpuGlyphBlit,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeLinearGrad {
    pub(crate) rect: GpuLinearGradientRect,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeRadialGrad {
    pub(crate) grad: GpuRadialGradient,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeSector {
    pub(crate) sector: GpuSector,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeLine {
    pub(crate) line: GpuLineSegment,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeMesh {
    pub(crate) mesh: GpuSolidMesh,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeShadow {
    pub(crate) shadow: GpuBoxShadow,
    pub(crate) scissor: (i32, i32, i32, i32),
}

/// 严格 GPU image blit 规划结果：可见入队 / 不可见跳过 / 需 soft 或 typed 失败。
pub(crate) enum DirectImageBlit {
    Ready(GpuImageBlit),
    Culled,
    Unsupported,
}

pub(crate) struct PendingNativeImage {
    pub(crate) blit: GpuImageBlit,
    pub(crate) scissor: (i32, i32, i32, i32),
}

// 保存原生 Canvas2D 的同纹理滚动边界，供 ordered RHI lowering 使用。
#[derive(Clone, Copy)]
pub(crate) struct PendingNativeScroll {
    // 保存逻辑坐标中的滚动视口。
    pub(crate) viewport: Rect,
    // 保存源区域相对视口的整数位移。
    pub(crate) dx: i32,
    // 保存源区域相对视口的整数位移。
    pub(crate) dy: i32,
}

pub(crate) enum PendingNativeOp {
    SolidRect(PendingNativeRect),
    StrokeRect(PendingNativeStroke),
    Glyph(PendingNativeGlyph),
    LinearGradient(PendingNativeLinearGrad),
    RadialGradient(PendingNativeRadialGrad),
    Sector(PendingNativeSector),
    Line(PendingNativeLine),
    SolidMesh(PendingNativeMesh),
    BoxShadow(PendingNativeShadow),
    ImageBlit(PendingNativeImage),
    // 目标相关的同纹理搬移必须作为独立 painter-order boundary。
    ScrollCopy(PendingNativeScroll),
}

impl PendingNativeOp {
    pub(super) fn scissor(&self) -> (i32, i32, i32, i32) {
        match self {
            Self::SolidRect(op) => op.scissor,
            Self::StrokeRect(op) => op.scissor,
            Self::Glyph(op) => op.scissor,
            Self::LinearGradient(op) => op.scissor,
            Self::RadialGradient(op) => op.scissor,
            Self::Sector(op) => op.scissor,
            Self::Line(op) => op.scissor,
            Self::SolidMesh(op) => op.scissor,
            Self::BoxShadow(op) => op.scissor,
            Self::ImageBlit(op) => op.scissor,
            // scroll 不参与普通 draw scissor；该值仅供兼容提交边界安全分组。
            Self::ScrollCopy(_) => (0, 0, 0, 0),
        }
    }
}
