//! 物理单位系统——为 2D/3D 空间提供带单位的坐标值。
//!
//! 支持六种物理单位：像素(px)、毫米(mm)、厘米(cm)、米(m)、点(pt)、英寸(inch)。
//! 所有单位可转换为逻辑像素（dip），基于 DPI 换算。
//!
//! 3D 场景中建议使用米(m)作为世界单位，2D UI 中建议使用毫米(mm)或点(pt)。

use std::ops::Add;

/// 物理单位：描述一个带单位的标量值。
///
/// 在 3D 空间中 x/y/z 轴使用同一套单位转换规则。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhysicalUnit {
    /// 像素（设备无关）
    Px(f32),
    /// 毫米
    Mm(f32),
    /// 厘米
    Cm(f32),
    /// 米（3D 场景的世界单位）
    M(f32),
    /// 点（1pt = 1/72 英寸，印刷标准）
    Pt(f32),
    /// 英寸
    Inch(f32),
}

impl PhysicalUnit {
    /// 转换为逻辑像素（dip），基于给定 DPI。
    ///
    /// DPI 典型值：
    /// - 96    = 标准屏幕
    /// - 192   = 高 DPI 屏幕 (2x)
    /// - 72    = 传统屏幕/Web
    /// - 300   = 打印
    /// - 72    = PowerPoint / Office 默认
    pub fn to_dip(&self, dpi: f32) -> f32 {
        match self {
            PhysicalUnit::Px(v) => *v,
            PhysicalUnit::Mm(v) => v * (dpi / 25.4), // 1 inch = 25.4 mm
            PhysicalUnit::Cm(v) => v * (dpi / 2.54), // 1 inch = 2.54 cm
            PhysicalUnit::M(v) => v * (dpi / 0.0254), // 1 inch = 0.0254 m
            PhysicalUnit::Pt(v) => v * (dpi / 72.0), // 1 pt = 1/72 inch
            PhysicalUnit::Inch(v) => v * dpi,
        }
    }

    /// 转换为物理像素（考虑 device pixel ratio）。
    #[inline(always)]
    pub fn to_px(&self, dpi: f32, dpr: f32) -> f32 {
        self.to_dip(dpi) * dpr
    }

    /// 提取原始数值。
    pub fn value(&self) -> f32 {
        match self {
            PhysicalUnit::Px(v)
            | PhysicalUnit::Mm(v)
            | PhysicalUnit::Cm(v)
            | PhysicalUnit::M(v)
            | PhysicalUnit::Pt(v)
            | PhysicalUnit::Inch(v) => *v,
        }
    }

    /// 单位名称。
    pub fn unit_name(&self) -> &'static str {
        match self {
            PhysicalUnit::Px(_) => "px",
            PhysicalUnit::Mm(_) => "mm",
            PhysicalUnit::Cm(_) => "cm",
            PhysicalUnit::M(_) => "m",
            PhysicalUnit::Pt(_) => "pt",
            PhysicalUnit::Inch(_) => "inch",
        }
    }
}

impl Add for PhysicalUnit {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        // 单位相加时保持左侧单位类型
        match self {
            PhysicalUnit::Px(v) => PhysicalUnit::Px(v + rhs.to_dip(96.0)),
            PhysicalUnit::Mm(v) => PhysicalUnit::Mm(v + rhs.to_dip(25.4) / 25.4 * 25.4),
            PhysicalUnit::Cm(v) => PhysicalUnit::Cm(v + rhs.to_dip(2.54) / 2.54),
            PhysicalUnit::M(v) => PhysicalUnit::M(v + rhs.to_dip(0.0254) / 0.0254),
            PhysicalUnit::Pt(v) => PhysicalUnit::Pt(v + rhs.to_dip(72.0) / 72.0),
            PhysicalUnit::Inch(v) => PhysicalUnit::Inch(v + rhs.to_dip(1.0)),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════

/// 物理单位便捷构造器。
///
/// 允许使用字面量直接构造 PhysicalUnit：
pub trait PhysicalUnitExt {
    /// 把数值包装为设备无关像素单位。
    fn px(self) -> PhysicalUnit;
    /// 把数值包装为毫米单位。
    fn mm(self) -> PhysicalUnit;
    /// 把数值包装为厘米单位。
    fn cm(self) -> PhysicalUnit;
    /// 把数值包装为米单位。
    fn m(self) -> PhysicalUnit;
    /// 把数值包装为印刷点单位。
    fn pt(self) -> PhysicalUnit;
    /// 把数值包装为英寸单位。
    fn inch(self) -> PhysicalUnit;
}

impl PhysicalUnitExt for f32 {
    fn px(self) -> PhysicalUnit {
        PhysicalUnit::Px(self)
    }
    fn mm(self) -> PhysicalUnit {
        PhysicalUnit::Mm(self)
    }
    fn cm(self) -> PhysicalUnit {
        PhysicalUnit::Cm(self)
    }
    fn m(self) -> PhysicalUnit {
        PhysicalUnit::M(self)
    }
    fn pt(self) -> PhysicalUnit {
        PhysicalUnit::Pt(self)
    }
    fn inch(self) -> PhysicalUnit {
        PhysicalUnit::Inch(self)
    }
}

impl PhysicalUnitExt for i32 {
    fn px(self) -> PhysicalUnit {
        PhysicalUnit::Px(self as f32)
    }
    fn mm(self) -> PhysicalUnit {
        PhysicalUnit::Mm(self as f32)
    }
    fn cm(self) -> PhysicalUnit {
        PhysicalUnit::Cm(self as f32)
    }
    fn m(self) -> PhysicalUnit {
        PhysicalUnit::M(self as f32)
    }
    fn pt(self) -> PhysicalUnit {
        PhysicalUnit::Pt(self as f32)
    }
    fn inch(self) -> PhysicalUnit {
        PhysicalUnit::Inch(self as f32)
    }
}

// ════════════════════════════════════════════════════════════════════════════

/// 角度便捷构造器。
///
/// 允许使用度数或弧度构造角度：
pub trait AngleExt {
    /// 度 → 弧度
    fn deg(self) -> f32;
    /// 弧度（原样返回）
    fn rad(self) -> f32;
}

impl AngleExt for f32 {
    fn deg(self) -> f32 {
        self * std::f32::consts::PI / 180.0
    }
    fn rad(self) -> f32 {
        self
    }
}

impl AngleExt for i32 {
    fn deg(self) -> f32 {
        (self as f32) * std::f32::consts::PI / 180.0
    }
    fn rad(self) -> f32 {
        self as f32
    }
}

// ════════════════════════════════════════════════════════════════════════════
