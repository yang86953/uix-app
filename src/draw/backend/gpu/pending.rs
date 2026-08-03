//! GPU-native 待决命令载荷 — gpu 子模块。
//!
//! `Canvas2D` 调用被转换为 [`PendingNativeOp`] 载荷入队，present 时由
//! [`super::submit::NativeGpuCanvas2D::submit_native`] 一次性下发；载荷携带
//! 裁剪与变换折叠后的设备几何。

use crate::core::Rect;
use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::{BlendMode, ImageHandle, Transform};
use crate::native::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSector,
    GpuSolidMesh, GpuSolidRect, GpuStrokeRect,
};

pub(crate) struct StateSnapshot {
    pub(crate) clip_rect: Rect,
    pub(crate) opacity: f32,
    pub(crate) offset_x: f32,
    pub(crate) offset_y: f32,
    pub(crate) transform: Transform,
    pub(crate) blend_mode: BlendMode,
}

pub(crate) struct PendingNativeRect {
    pub(crate) rect: GpuSolidRect,
    /// Logical scissor AABB (x, y, w, h).
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeStroke {
    pub(crate) rect: GpuStrokeRect,
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

pub(crate) enum PendingNativeOp {
    SolidRect(PendingNativeRect),
    StrokeRect(PendingNativeStroke),
    Glyph(PendingNativeGlyph),
    LinearGradient(PendingNativeLinearGrad),
    RadialGradient(PendingNativeRadialGrad),
    Sector(PendingNativeSector),
    SolidMesh(PendingNativeMesh),
    BoxShadow(PendingNativeShadow),
    ImageBlit(PendingNativeImage),
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
            Self::SolidMesh(op) => op.scissor,
            Self::BoxShadow(op) => op.scissor,
            Self::ImageBlit(op) => op.scissor,
        }
    }

    pub(super) fn same_kind(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::SolidRect(_), Self::SolidRect(_))
                | (Self::StrokeRect(_), Self::StrokeRect(_))
                | (Self::Glyph(_), Self::Glyph(_))
                | (Self::LinearGradient(_), Self::LinearGradient(_))
                | (Self::RadialGradient(_), Self::RadialGradient(_))
                | (Self::Sector(_), Self::Sector(_))
                | (Self::SolidMesh(_), Self::SolidMesh(_))
                | (Self::BoxShadow(_), Self::BoxShadow(_))
                | (Self::ImageBlit(_), Self::ImageBlit(_))
        )
    }
}