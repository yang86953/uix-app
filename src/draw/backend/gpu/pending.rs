//! GPU-native 待决命令载荷 — gpu 子模块。
//!
//! `Canvas2D` 调用被转换为 [`PendingNativeOp`] 载荷入队，present 时由
//! [`super::submit::NativeGpuCanvas2D::submit_native`] 一次性下发；载荷携带
//! 裁剪与变换折叠后的设备几何。

use crate::core::Rect;
use crate::draw::geometry::types::{BlendMode, Transform};
use crate::native::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSector,
    GpuSolidMesh, GpuSolidRect, GpuStrokeRect,
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
            Self::SolidMesh(op) => op.scissor,
            Self::BoxShadow(op) => op.scissor,
            Self::ImageBlit(op) => op.scissor,
            // scroll 不参与普通 draw scissor；该值仅供兼容提交边界安全分组。
            Self::ScrollCopy(_) => (0, 0, 0, 0),
        }
    }

    // 判断单个 pending 操作是否需要 retained RHI Additive shape pipeline。
    pub(super) fn is_additive_solid_rect(&self) -> bool {
        // 只接受显式标记的实心矩形，不把其他图元混合语义外推到这里。
        matches!(self, Self::SolidRect(rect) if rect.additive)
    }

    // 判断单个待决描边是否必须使用 retained RHI Additive shape pipeline。
    pub(super) fn is_additive_stroke_rect(&self) -> bool {
        // 只读取显式描边事实，禁止从同类批次首项推断后续操作。
        matches!(self, Self::StrokeRect(stroke) if stroke.additive)
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
                | (Self::ScrollCopy(_), Self::ScrollCopy(_))
        )
    }
}

// 验证 legacy 批次扫描不会漏掉非首项 Additive 矩形。
#[cfg(test)]
mod tests {
    // 引入当前 pending 类型与原生矩形载荷。
    use super::{PendingNativeOp, PendingNativeRect, PendingNativeStroke};
    // 引入最小实心与描边矩形 fixture 类型。
    use crate::native::present::{GpuSolidRect, GpuStrokeRect};

    // 创建带指定 blend 标记的有限实心矩形操作。
    fn solid_rect(additive: bool) -> PendingNativeOp {
        // 返回可参与同类批处理的 pending operation。
        PendingNativeOp::SolidRect(PendingNativeRect {
            // 使用有限正矩形，避免 fixture 混入几何无效因素。
            rect: GpuSolidRect {
                // 使用原点 x 坐标。
                x: 0.0,
                // 使用原点 y 坐标。
                y: 0.0,
                // 保持宽度为正。
                w: 1.0,
                // 保持高度为正。
                h: 1.0,
                // 使用不透明白色 premultiplied 常量。
                rgba: [1.0; 4],
                // fixture 不需要圆角。
                radius: [0.0; 4],
            },
            // 注入本次测试需要的目标 blend 标记。
            additive,
            // 使用完整覆盖 fixture 的逻辑裁剪。
            scissor: (0, 0, 1, 1),
        })
    }

    // 创建带指定 blend 标记的有限描边矩形操作。
    fn stroke_rect(additive: bool) -> PendingNativeOp {
        // 返回可参与同类批处理的 pending operation。
        PendingNativeOp::StrokeRect(PendingNativeStroke {
            // 使用有限正描边矩形，避免 fixture 混入几何无效因素。
            rect: GpuStrokeRect {
                // 使用原点 x 坐标。
                x: 0.0,
                // 使用原点 y 坐标。
                y: 0.0,
                // 保持宽度为正。
                w: 2.0,
                // 保持高度为正。
                h: 2.0,
                // 使用不透明白色常量。
                rgba: [1.0; 4],
                // fixture 不需要圆角。
                radius: [0.0; 4],
                // 使用正有限描边宽度。
                line_width: 1.0,
            },
            // 注入本次测试需要的目标 blend 标记。
            additive,
            // 使用完整覆盖 fixture 的逻辑裁剪。
            scissor: (0, 0, 2, 2),
        })
    }

    // 批尾 Additive 也必须阻止 legacy SrcOver 批提交。
    #[test]
    fn detects_additive_after_normal_solid_rect() {
        // 构造普通项在前、Additive 项在后的连续批次。
        let mixed = [solid_rect(false), solid_rect(true)];
        // 扫描整个批次必须发现第二项的 Additive 标记。
        assert!(mixed
            .iter()
            .any(PendingNativeOp::is_additive_solid_rect));
        // 构造完全普通的对照批次。
        let normal = [solid_rect(false), solid_rect(false)];
        // 对照批次不得被误判为 Additive。
        assert!(!normal
            .iter()
            .any(PendingNativeOp::is_additive_solid_rect));
    }

    // 批尾 Additive 描边也必须阻止 legacy SrcOver 批提交。
    #[test]
    fn detects_additive_stroke_after_normal_stroke_rect() {
        // 构造普通项在前、Additive 项在后的连续描边批次。
        let mixed = [stroke_rect(false), stroke_rect(true)];
        // 扫描整个批次必须发现第二项的 Additive 标记。
        assert!(mixed.iter().any(PendingNativeOp::is_additive_stroke_rect));
        // 构造完全普通的描边对照批次。
        let normal = [stroke_rect(false), stroke_rect(false)];
        // 对照批次不得被误判为 Additive。
        assert!(!normal.iter().any(PendingNativeOp::is_additive_stroke_rect));
    }
}
