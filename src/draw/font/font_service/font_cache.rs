//! 字体元数据与字形光栅缓存。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::draw::FontHandle;

// ════════════════════════════════════════════════════════════════════════════
// 字体元数据
// ════════════════════════════════════════════════════════════════════════════

/// 已加载字体的元数据。
#[derive(Debug, Clone)]
pub struct FontFace {
    /// 字体族名称（如 "Noto Sans SC", "sans-serif"）。
    pub family: String,
    /// 字体文件路径（从文件加载时才有值）。
    pub path: Option<String>,
}

/// 注册表中一个字体槽位。
#[derive(Debug)]
pub(crate) struct FontSlot {
    pub(crate) handle: FontHandle,
    pub(crate) face: FontFace,
}

// ════════════════════════════════════════════════════════════════════════════
// 字形缓存（统一、线程安全）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub(crate) struct GlyphCacheKey {
    pub(crate) font_idx: u32,
    pub(crate) glyph_id: u32,
    pub(crate) pixel_size: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct CachedRaster {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) coverage: Arc<Vec<u8>>,
    pub(crate) bearing_x: f32,
    pub(crate) bearing_y: f32,
}

/// 统一字形光栅缓存。
///
/// 由 FontService 统一持有，所有后端共享。后端自身不应再维护独立缓存。
/// 默认最多缓存 8192 个字形的覆盖位图。
#[derive(Debug)]
pub struct GlyphCache {
    inner: Mutex<HashMap<GlyphCacheKey, CachedRaster>>,
    max_entries: usize,
}

impl GlyphCache {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            max_entries: 8192,
        }
    }

    pub(crate) fn get(&self, key: &GlyphCacheKey) -> Option<CachedRaster> {
        let map = self.inner.lock().ok()?;
        map.get(key).cloned()
    }

    pub(crate) fn insert(&self, key: GlyphCacheKey, raster: CachedRaster) {
        if let Ok(mut map) = self.inner.lock() {
            if map.len() >= self.max_entries {
                map.clear();
            }
            map.insert(key, raster);
        }
    }

    pub(crate) fn clear(&self) {
        if let Ok(mut map) = self.inner.lock() {
            map.clear();
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }

    pub(crate) fn memory_usage(&self) -> usize {
        self.inner
            .lock()
            .map(|map| {
                map.values()
                    .map(|r| std::mem::size_of::<GlyphCacheKey>() + r.width * r.height)
                    .sum()
            })
            .unwrap_or(0)
    }
}
