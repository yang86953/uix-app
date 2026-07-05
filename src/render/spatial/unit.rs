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
/// ```rust
/// use uix::render::spatial::PhysicalUnitExt;
/// let w = 10.0.mm();
/// let h = 5.0.cm();
/// let fs = 12.0.pt();
/// ```
pub trait PhysicalUnitExt {
    fn px(self) -> PhysicalUnit;
    fn mm(self) -> PhysicalUnit;
    fn cm(self) -> PhysicalUnit;
    fn m(self) -> PhysicalUnit;
    fn pt(self) -> PhysicalUnit;
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
/// ```rust
/// use uix::render::spatial::AngleExt;
/// let a = 45.0.deg();  // 度 → 弧度
/// let b = 0.5.rad();   // 弧度
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px_to_dip_is_identity() {
        let u = PhysicalUnit::Px(100.0);
        assert!((u.to_dip(96.0) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn mm_to_dip() {
        // 25.4mm = 1 inch = 96 dip (at 96 DPI)
        let u = PhysicalUnit::Mm(25.4);
        assert!((u.to_dip(96.0) - 96.0).abs() < 1e-6);
    }

    #[test]
    fn cm_to_dip() {
        // 2.54cm = 1 inch = 96 dip
        let u = PhysicalUnit::Cm(2.54);
        assert!((u.to_dip(96.0) - 96.0).abs() < 1e-6);
    }

    #[test]
    fn m_to_dip() {
        // 0.0254m = 1 inch = 96 dip
        let u = PhysicalUnit::M(0.0254);
        assert!((u.to_dip(96.0) - 96.0).abs() < 1e-4);
    }

    #[test]
    fn pt_to_dip() {
        // 72pt = 1 inch = 96 dip
        let u = PhysicalUnit::Pt(72.0);
        assert!((u.to_dip(96.0) - 96.0).abs() < 1e-6);
    }

    #[test]
    fn inch_to_dip() {
        let u = PhysicalUnit::Inch(1.0);
        assert!((u.to_dip(96.0) - 96.0).abs() < 1e-6);
    }

    #[test]
    fn high_dpi_scaling() {
        // 在同 DPI 下 pt 和 dip 的关系
        // 12pt @ 96dpi = 16px
        let pt = PhysicalUnit::Pt(12.0);
        assert!((pt.to_dip(96.0) - 16.0).abs() < 1e-6);
        // 12pt @ 192dpi = 32px
        assert!((pt.to_dip(192.0) - 32.0).abs() < 1e-6);
    }

    #[test]
    fn physical_unit_ext_f32() {
        let v = 10.0f32;
        match v.mm() {
            PhysicalUnit::Mm(x) => assert!((x - 10.0).abs() < 1e-10),
            _ => panic!("expected Mm"),
        }
    }

    #[test]
    fn physical_unit_ext_i32() {
        let v = 5i32;
        match v.cm() {
            PhysicalUnit::Cm(x) => assert!((x - 5.0).abs() < 1e-10),
            _ => panic!("expected Cm"),
        }
    }

    #[test]
    fn angle_deg_to_rad() {
        let r = 90.0f32.deg();
        assert!((r - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn angle_rad_is_identity() {
        let r = 0.5f32.rad();
        assert!((r - 0.5).abs() < 1e-10);
    }

    #[test]
    fn unit_name() {
        assert_eq!(PhysicalUnit::Px(1.0).unit_name(), "px");
        assert_eq!(PhysicalUnit::Mm(1.0).unit_name(), "mm");
        assert_eq!(PhysicalUnit::Cm(1.0).unit_name(), "cm");
        assert_eq!(PhysicalUnit::M(1.0).unit_name(), "m");
        assert_eq!(PhysicalUnit::Pt(1.0).unit_name(), "pt");
        assert_eq!(PhysicalUnit::Inch(1.0).unit_name(), "inch");
    }

    #[test]
    fn value() {
        assert!((PhysicalUnit::Cm(5.0).value() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn add_units() {
        let a = PhysicalUnit::Mm(10.0);
        let b = PhysicalUnit::Cm(1.0); // 10mm
        let c = a + b;
        match c {
            PhysicalUnit::Mm(v) => assert!((v - 20.0).abs() < 1e-4),
            _ => panic!("expected Mm"),
        }
    }

    #[test]
    fn to_px_with_dpr() {
        let u = PhysicalUnit::Px(100.0);
        assert!((u.to_px(96.0, 2.0) - 200.0).abs() < 1e-10);
    }
}
