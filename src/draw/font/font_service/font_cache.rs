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
    pub(crate) coverage: Arc<[u8]>,
    pub(crate) bearing_x: f32,
    pub(crate) bearing_y: f32,
    pub(crate) outline_mesh: Option<Arc<[f32]>>,
}

#[derive(Debug, Default)]
struct GlyphCacheState {
    entries: HashMap<GlyphCacheKey, CachedRaster>,
    retained_bytes: usize,
}

/// 统一字形光栅缓存。
///
/// 由 FontService 统一持有，所有后端共享。后端自身不应再维护独立缓存。
/// 默认最多缓存 8192 个字形的覆盖位图。
#[derive(Debug)]
pub struct GlyphCache {
    inner: Mutex<GlyphCacheState>,
    max_entries: usize,
    max_bytes: usize,
}

impl GlyphCache {
    pub(crate) fn new() -> Self {
        Self::with_limits(2048, 8 * 1024 * 1024)
    }

    #[cfg(test)]
    pub(crate) fn with_max_entries(max_entries: usize) -> Self {
        Self::with_limits(max_entries, usize::MAX)
    }

    pub(crate) fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(GlyphCacheState::default()),
            max_entries: max_entries.max(1),
            max_bytes,
        }
    }

    pub(crate) fn get(&self, key: &GlyphCacheKey) -> Option<CachedRaster> {
        let state = self.inner.lock().ok()?;
        state.entries.get(key).cloned()
    }

    pub(crate) fn insert(&self, key: GlyphCacheKey, raster: CachedRaster) {
        let mesh_bytes = raster
            .outline_mesh
            .as_ref()
            .map(|mesh| mesh.len().saturating_mul(std::mem::size_of::<f32>()))
            .unwrap_or(0);
        let retained_bytes = std::mem::size_of::<GlyphCacheKey>()
            .saturating_add(raster.coverage.len())
            .saturating_add(mesh_bytes);
        if retained_bytes > self.max_bytes {
            return;
        }

        if let Ok(mut state) = self.inner.lock() {
            if let Some(previous) = state.entries.remove(&key) {
                let previous_mesh = previous
                    .outline_mesh
                    .as_ref()
                    .map(|mesh| mesh.len().saturating_mul(std::mem::size_of::<f32>()))
                    .unwrap_or(0);
                state.retained_bytes = state.retained_bytes.saturating_sub(
                    std::mem::size_of::<GlyphCacheKey>()
                        .saturating_add(previous.coverage.len())
                        .saturating_add(previous_mesh),
                );
            }

            while state.entries.len() >= self.max_entries
                || state.retained_bytes > self.max_bytes.saturating_sub(retained_bytes)
            {
                let Some(evicted_key) = state.entries.keys().next().cloned() else {
                    break;
                };
                if let Some(evicted) = state.entries.remove(&evicted_key) {
                    let evicted_mesh = evicted
                        .outline_mesh
                        .as_ref()
                        .map(|mesh| mesh.len().saturating_mul(std::mem::size_of::<f32>()))
                        .unwrap_or(0);
                    state.retained_bytes = state.retained_bytes.saturating_sub(
                        std::mem::size_of::<GlyphCacheKey>()
                            .saturating_add(evicted.coverage.len())
                            .saturating_add(evicted_mesh),
                    );
                }
            }

            state.retained_bytes = state.retained_bytes.saturating_add(retained_bytes);
            state.entries.insert(key, raster);
        }
    }

    pub(crate) fn clear(&self) {
        if let Ok(mut state) = self.inner.lock() {
            state.entries.clear();
            state.retained_bytes = 0;
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.inner
            .lock()
            .map(|state| state.entries.len())
            .unwrap_or(0)
    }

    pub(crate) fn memory_usage(&self) -> usize {
        self.inner
            .lock()
            .map(|state| state.retained_bytes)
            .unwrap_or(0)
    }
}
