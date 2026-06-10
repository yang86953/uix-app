use crate::base::Rect;

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

/// Dirty region tracking for incremental rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct DirtyRegion {
    pub rect: Rect,
    pub full_frame: bool,
    pub clear_required: bool,
}

impl DirtyRegion {
    pub fn full() -> Self {
        Self {
            rect: Rect::zero(),
            full_frame: true,
            clear_required: true,
        }
    }
    pub fn empty() -> Self {
        Self {
            rect: Rect::zero(),
            full_frame: false,
            clear_required: false,
        }
    }
    pub fn area(rect: Rect) -> Self {
        Self {
            rect,
            full_frame: false,
            clear_required: true,
        }
    }
    pub fn reset(&mut self) {
        *self = Self::empty();
    }
}

impl Default for DirtyRegion {
    fn default() -> Self {
        Self::empty()
    }
}

/// Text layout options.
#[derive(Debug, Clone, PartialEq)]
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
pub struct FontHandle(pub(crate) u32);

impl FontHandle {
    /// Create a new font handle with the given index.
    pub const fn new(idx: u32) -> Self {
        Self(idx)
    }
}
