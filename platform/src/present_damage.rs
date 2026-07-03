//! 呈现损伤区域 — 支持多矩形局部更新（Phase 8）。

/// CPU/GPU 像素呈现时的屏幕损伤描述。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PresentDamage {
    /// 全屏重绘。
    #[default]
    Full,
    /// 一个或多个局部矩形 `(x, y, w, h)`，设备像素坐标。
    Partial(Vec<(i32, i32, i32, i32)>),
}

impl PresentDamage {
    /// 单矩形局部损伤。
    pub fn single(x: i32, y: i32, w: i32, h: i32) -> Self {
        if w <= 0 || h <= 0 {
            Self::Full
        } else {
            Self::Partial(vec![(x, y, w, h)])
        }
    }

    /// 兼容旧 API：`None` = 全屏，`Some` = 单矩形。
    pub fn from_legacy(dirty_rect: Option<(i32, i32, i32, i32)>) -> Self {
        match dirty_rect {
            None => Self::Full,
            Some((x, y, w, h)) => Self::single(x, y, w, h),
        }
    }

    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
}
