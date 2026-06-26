use uix_core::Rect;

/// Corner radii.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radius {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl Radius {
    pub const fn uniform(r: f32) -> Self {
        Self {
            tl: r,
            tr: r,
            br: r,
            bl: r,
        }
    }
    pub const fn zero() -> Self {
        Self::uniform(0.0)
    }
}

impl Default for Radius {
    fn default() -> Self {
        Self::zero()
    }
}

/// Blend modes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    #[default]
    Alpha,
    SrcOver,
    Additive,
}

/// Gradient direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientDirection {
    Horizontal,
    Vertical,
    DiagonalTLBR,
    DiagonalBLTR,
}

/// Text horizontal alignment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Text vertical alignment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    #[default]
    Top,
    Middle,
    Bottom,
    Baseline,
}

/// 2D affine transform (3x2 matrix).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub m: [f32; 6],
}

impl Transform {
    pub const fn identity() -> Self {
        Self {
            m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        }
    }

    pub fn translate(tx: f32, ty: f32) -> Self {
        Self {
            m: [1.0, 0.0, tx, 0.0, 1.0, ty],
        }
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            m: [sx, 0.0, 0.0, 0.0, sy, 0.0],
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

/// 脏矩形合并阈值：超过该数量则合并为 bounds（避免 Vec 膨胀）。
const DIRTY_MERGE_THRESHOLD: usize = 16;

/// Dirty region tracking for incremental rendering.
///
/// 支持多矩形损伤（multi-rect damage）：当多个不重叠区域变脏时，
/// 分别记录各自矩形而非合并为一个巨大的 union，从而：
/// - 清理（clear）时只清理实际脏区域，节省像素填充
/// - 渲染遍历时更精确地判断 widget 是否需要绘制
///
/// 当矩形数量超过 `DIRTY_MERGE_THRESHOLD`（16）时自动合并为 bounds，
/// 避免极端场景下的性能退化。
#[derive(Debug, Clone, PartialEq)]
pub struct DirtyRegion {
    pub rects: Vec<Rect>,
    pub full_frame: bool,
    pub clear_required: bool,
}

impl DirtyRegion {
    /// 全帧脏区域（强制全部重绘）。
    pub fn full() -> Self {
        Self {
            rects: Vec::new(),
            full_frame: true,
            clear_required: true,
        }
    }

    /// 空区域（无脏内容）。
    pub fn empty() -> Self {
        Self {
            rects: Vec::new(),
            full_frame: false,
            clear_required: false,
        }
    }

    /// 从单个矩形创建脏区域。
    pub fn area(rect: Rect) -> Self {
        Self {
            rects: if rect.w > 0.0 && rect.h > 0.0 {
                vec![rect]
            } else {
                Vec::new()
            },
            full_frame: false,
            clear_required: true,
        }
    }

    /// 重置为空区域。
    pub fn reset(&mut self) {
        *self = Self::empty();
    }

    /// 是否无脏内容。
    pub fn is_empty(&self) -> bool {
        !self.full_frame && self.rects.is_empty() && !self.clear_required
    }

    /// 添加一个脏矩形（支持合并阈值）。
    pub fn add_rect(&mut self, rect: Rect) {
        if rect.w <= 0.0 || rect.h <= 0.0 || self.full_frame {
            return;
        }
        self.clear_required = true;
        if self.rects.len() >= DIRTY_MERGE_THRESHOLD {
            // 超过阈值，合并为一个 bounds
            let b = self.bounds().union(&rect);
            self.rects.clear();
            self.rects.push(b);
        } else {
            self.rects.push(rect);
        }
    }

    /// 所有脏矩形的外接 union 矩形。
    pub fn bounds(&self) -> Rect {
        if self.rects.is_empty() {
            return Rect::zero();
        }
        let mut b = self.rects[0];
        for &r in &self.rects[1..] {
            b = b.union(&r);
        }
        b
    }

    /// 判断给定矩形是否与任意脏矩形相交。
    pub fn intersects(&self, rect: Rect) -> bool {
        if self.full_frame {
            return true;
        }
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return false;
        }
        self.rects.iter().any(|&r| r.intersect(&rect).is_some())
    }

    /// 返回脏矩形切片。
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }
}

impl Default for DirtyRegion {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Radius ──

    #[test]
    fn radius_uniform() {
        let r = Radius::uniform(8.0);
        assert_eq!(r.tl, 8.0);
        assert_eq!(r.tr, 8.0);
        assert_eq!(r.br, 8.0);
        assert_eq!(r.bl, 8.0);
    }

    #[test]
    fn radius_zero() {
        let r = Radius::zero();
        assert_eq!(r.tl, 0.0);
        assert_eq!(r.tr, 0.0);
        assert_eq!(r.br, 0.0);
        assert_eq!(r.bl, 0.0);
    }

    #[test]
    fn radius_default_is_zero() {
        let r = Radius::default();
        assert_eq!(r, Radius::zero());
    }

    // ── Transform ──

    #[test]
    fn transform_identity() {
        let t = Transform::identity();
        assert_eq!(t.m, [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn transform_translate() {
        let t = Transform::translate(10.0, 20.0);
        assert_eq!(t.m, [1.0, 0.0, 10.0, 0.0, 1.0, 20.0]);
    }

    #[test]
    fn transform_scale() {
        let t = Transform::scale(2.0, 3.0);
        assert_eq!(t.m, [2.0, 0.0, 0.0, 0.0, 3.0, 0.0]);
    }

    #[test]
    fn transform_default_is_identity() {
        assert_eq!(Transform::default(), Transform::identity());
    }

    // ── DirtyRegion ──

    #[test]
    fn dirty_region_empty() {
        let d = DirtyRegion::empty();
        assert!(!d.full_frame);
        assert!(!d.clear_required);
        assert!(d.rects.is_empty());
        assert!(d.is_empty());
    }

    #[test]
    fn dirty_region_full() {
        let d = DirtyRegion::full();
        assert!(d.full_frame);
        assert!(d.clear_required);
    }

    #[test]
    fn dirty_region_area() {
        let d = DirtyRegion::area(Rect::new(10.0, 20.0, 100.0, 50.0));
        assert!(!d.full_frame);
        assert!(d.clear_required);
        assert_eq!(d.rects.len(), 1);
        assert_eq!(d.rects[0], Rect::new(10.0, 20.0, 100.0, 50.0));
    }

    #[test]
    fn dirty_region_zero_area_rect_is_empty() {
        let d = DirtyRegion::area(Rect::new(0.0, 0.0, 0.0, 0.0));
        assert!(d.rects.is_empty());
    }

    #[test]
    fn dirty_region_add_rect() {
        let mut d = DirtyRegion::empty();
        d.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        d.add_rect(Rect::new(20.0, 20.0, 5.0, 5.0));
        assert_eq!(d.rects.len(), 2);
        assert!(d.clear_required);
    }

    #[test]
    fn dirty_region_bounds() {
        let mut d = DirtyRegion::empty();
        d.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        d.add_rect(Rect::new(20.0, 20.0, 10.0, 10.0));
        let b = d.bounds();
        assert_eq!(b.x, 0.0);
        assert_eq!(b.y, 0.0);
        assert_eq!(b.w, 30.0);
        assert_eq!(b.h, 30.0);
    }

    #[test]
    fn dirty_region_intersects() {
        let mut d = DirtyRegion::empty();
        d.add_rect(Rect::new(10.0, 10.0, 50.0, 50.0));
        assert!(d.intersects(Rect::new(20.0, 20.0, 5.0, 5.0)));
        assert!(!d.intersects(Rect::new(100.0, 100.0, 5.0, 5.0)));
    }

    #[test]
    fn dirty_region_full_intersects_all() {
        let d = DirtyRegion::full();
        assert!(d.intersects(Rect::new(0.0, 0.0, 1.0, 1.0)));
        assert!(d.intersects(Rect::new(9999.0, 9999.0, 1.0, 1.0)));
    }

    #[test]
    fn dirty_region_reset() {
        let mut d = DirtyRegion::full();
        d.reset();
        assert!(d.is_empty());
        assert!(!d.full_frame);
    }

    #[test]
    fn dirty_region_merge_threshold() {
        let mut d = DirtyRegion::empty();
        // Add exactly DIRTY_MERGE_THRESHOLD rects: still individual
        for i in 0..16 {
            d.add_rect(Rect::new(i as f32, 0.0, 1.0, 1.0));
        }
        assert_eq!(d.rects.len(), 16);
        // One more triggers merge
        d.add_rect(Rect::new(99.0, 0.0, 1.0, 1.0));
        assert_eq!(d.rects.len(), 1);
        // Subsequent adds stay merged
        d.add_rect(Rect::new(100.0, 0.0, 1.0, 1.0));
        assert_eq!(d.rects.len(), 2);
    }

    // ── Handle types ──

    #[test]
    fn image_handle_default() {
        let h = ImageHandle::default();
        assert_eq!(h.index, 0);
        assert_eq!(h.kind, HandleKind::Image);
    }

    #[test]
    fn font_handle_new() {
        let h = FontHandle::new(42);
        assert_eq!(h.0, 42);
    }

    #[test]
    fn font_handle_default() {
        assert_eq!(FontHandle::default(), FontHandle::new(0));
    }

    // ── TextLayoutOptions ──

    #[test]
    fn text_layout_defaults() {
        let opts = TextLayoutOptions::default();
        assert_eq!(opts.max_width, f32::MAX);
        assert!(opts.word_wrap);
        assert_eq!(opts.h_align, HAlign::Left);
        assert_eq!(opts.v_align, VAlign::Top);
        assert_eq!(opts.font_size, 14.0);
    }

    // ── BlendMode ──

    #[test]
    fn blend_mode_default_is_alpha() {
        assert_eq!(BlendMode::default(), BlendMode::Alpha);
    }
}

/// Text layout options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextLayoutOptions {
    pub max_width: f32,
    pub max_height: f32,
    pub line_height: f32,
    pub word_wrap: bool,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub font_size: f32,
}

impl Default for TextLayoutOptions {
    fn default() -> Self {
        Self {
            max_width: f32::MAX,
            max_height: 0.0,
            line_height: 0.0,
            word_wrap: true,
            h_align: HAlign::Left,
            v_align: VAlign::Top,
            font_size: 14.0,
        }
    }
}

/// Handle kind discriminator for resource pool lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandleKind {
    Image,
    Offscreen,
}

/// Opaque handle for image/offscreen resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageHandle {
    pub(crate) index: u32,
    pub(crate) kind: HandleKind,
}

impl ImageHandle {
    pub(crate) const fn new(index: u32, kind: HandleKind) -> Self {
        Self { index, kind }
    }
}

impl Default for ImageHandle {
    fn default() -> Self {
        Self {
            index: 0,
            kind: HandleKind::Image,
        }
    }
}

/// Opaque handle for font references.
/// Contains a font index for engine lookup (replacing pointer-identity matching).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FontHandle(pub u32);

impl FontHandle {
    /// Create a new font handle with the given index.
    pub const fn new(idx: u32) -> Self {
        Self(idx)
    }
}
