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
    // Bits of a normalized positive finite em size, including its fraction.
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
    entries: HashMap<GlyphCacheKey, GlyphCacheEntry>,
    retained_bytes: usize,
    access_clock: u64,
}

#[derive(Debug)]
struct GlyphCacheEntry {
    // 缓存值以 Arc 持有，命中时仅做指针克隆，避免文本绘制热路径深拷贝。
    raster: Arc<CachedRaster>,
    retained_bytes: usize,
    last_access: u64,
}

/// 统一字形光栅缓存。
///
/// 由 FontService 统一持有，所有后端共享。后端自身不应再维护独立缓存。
/// 默认最多缓存 2048 个字形、保留 8 MiB，并按最近最少使用顺序逐项淘汰。
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

    // 测试目标保留按条目数配置缓存的构造器，供淘汰策略测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
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

    pub(crate) fn get(&self, key: &GlyphCacheKey) -> Option<Arc<CachedRaster>> {
        let mut state = self.inner.lock().ok()?;
        state.access_clock = state.access_clock.saturating_add(1);
        let access = state.access_clock;
        let entry = state.entries.get_mut(key)?;
        entry.last_access = access;
        // 命中时仅增加引用计数，不拷贝覆盖像素数据。
        Some(Arc::clone(&entry.raster))
    }

    pub(crate) fn insert(&self, key: GlyphCacheKey, raster: CachedRaster) {
        let retained_bytes = Self::retained_bytes(&raster);
        if retained_bytes > self.max_bytes {
            return;
        }

        if let Ok(mut state) = self.inner.lock() {
            if let Some(previous) = state.entries.remove(&key) {
                state.retained_bytes = state.retained_bytes.saturating_sub(previous.retained_bytes);
            }

            while state.entries.len() >= self.max_entries
                || state.retained_bytes > self.max_bytes.saturating_sub(retained_bytes)
            {
                let Some(evicted_key) = state
                    .entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_access)
                    .map(|(key, _)| key.clone())
                else {
                    break;
                };
                if let Some(evicted) = state.entries.remove(&evicted_key) {
                    state.retained_bytes =
                        state.retained_bytes.saturating_sub(evicted.retained_bytes);
                }
            }

            state.access_clock = state.access_clock.saturating_add(1);
            let access = state.access_clock;
            state.retained_bytes = state.retained_bytes.saturating_add(retained_bytes);
            state.entries.insert(
                key,
                GlyphCacheEntry {
                    // 进入缓存时统一包装为共享指针。
                    raster: Arc::new(raster),
                    retained_bytes,
                    last_access: access,
                },
            );
        }
    }

    fn retained_bytes(raster: &CachedRaster) -> usize {
        let mesh_bytes = raster
            .outline_mesh
            .as_ref()
            .map(|mesh| mesh.len().saturating_mul(std::mem::size_of::<f32>()))
            .unwrap_or(0);
        std::mem::size_of::<GlyphCacheKey>()
            .saturating_add(raster.coverage.len())
            .saturating_add(mesh_bytes)
    }

    pub(crate) fn remove_font(&self, font_idx: u32) {
        if let Ok(mut state) = self.inner.lock() {
            let removed_bytes = state
                .entries
                .iter()
                .filter(|(key, _)| key.font_idx == font_idx)
                .fold(0usize, |total, (_, entry)| {
                    total.saturating_add(entry.retained_bytes)
                });
            state.entries.retain(|key, _| key.font_idx != font_idx);
            state.retained_bytes = state.retained_bytes.saturating_sub(removed_bytes);
        }
    }

    pub(crate) fn clear(&self) {
        if let Ok(mut state) = self.inner.lock() {
            state.entries.clear();
            state.retained_bytes = 0;
            state.access_clock = 0;
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
