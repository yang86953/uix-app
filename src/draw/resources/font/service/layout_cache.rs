//! 字体服务拥有的有界文本布局缓存。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use crate::draw::resources::font::text_backend::{TextLayout, TextLayoutOptions};
use crate::draw::{FontHandle, HAlign, VAlign};

// 页面切换只需要覆盖近期可见控件；固定上限避免动态文本导致无界增长。
const TEXT_LAYOUT_CACHE_CAPACITY: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct TextLayoutCacheKey {
    font: u32,
    text_id: u64,
    max_width: u32,
    max_height: u32,
    line_height: u32,
    word_wrap: bool,
    h_align: u8,
    v_align: u8,
    font_size: u32,
}

impl TextLayoutCacheKey {
    fn new(font: &FontHandle, text_id: u64, opts: &TextLayoutOptions) -> Self {
        Self {
            font: font.0,
            text_id,
            max_width: opts.max_width.to_bits(),
            max_height: opts.max_height.to_bits(),
            line_height: opts.line_height.to_bits(),
            word_wrap: opts.word_wrap,
            h_align: match opts.h_align {
                HAlign::Left => 0,
                HAlign::Center => 1,
                HAlign::Right => 2,
                HAlign::Justify => 3,
            },
            v_align: match opts.v_align {
                VAlign::Top => 0,
                VAlign::Middle => 1,
                VAlign::Bottom => 2,
                VAlign::Baseline => 3,
            },
            font_size: opts.font_size.to_bits(),
        }
    }
}

#[derive(Debug)]
struct TextIntern {
    id: u64,
    references: usize,
}

#[derive(Debug)]
struct TextLayoutCacheEntry {
    text: Arc<str>,
    layout: Arc<TextLayout>,
}

#[derive(Default)]
struct TextLayoutCacheState {
    entries: HashMap<TextLayoutCacheKey, TextLayoutCacheEntry>,
    insertion_order: VecDeque<TextLayoutCacheKey>,
    texts: HashMap<Arc<str>, TextIntern>,
    next_text_id: u64,
}

/// 线程安全的近期文本布局缓存。
pub(super) struct TextLayoutCache {
    state: Mutex<TextLayoutCacheState>,
}

impl TextLayoutCache {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(TextLayoutCacheState::default()),
        }
    }

    pub(super) fn get(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
    ) -> Option<Arc<TextLayout>> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Arc<str> 允许直接以借用的 str 查询，命中时不构造拥有型文本键。
        let text_id = state.texts.get(text)?.id;
        let key = TextLayoutCacheKey::new(font, text_id, opts);
        state
            .entries
            .get(&key)
            .map(|entry| Arc::clone(&entry.layout))
    }

    pub(super) fn insert(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
        layout: Arc<TextLayout>,
    ) -> Arc<TextLayout> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(text_id) = state.texts.get(text).map(|intern| intern.id) {
            let key = TextLayoutCacheKey::new(font, text_id, opts);
            if let Some(entry) = state.entries.get(&key) {
                return Arc::clone(&entry.layout);
            }
        }
        while state.entries.len() >= TEXT_LAYOUT_CACHE_CAPACITY {
            let Some(oldest) = state.insertion_order.pop_front() else {
                state.entries.clear();
                state.texts.clear();
                break;
            };
            let Some(entry) = state.entries.remove(&oldest) else {
                continue;
            };
            let remove_text = state
                .texts
                .get_mut(entry.text.as_ref())
                .is_some_and(|intern| {
                    intern.references = intern.references.saturating_sub(1);
                    intern.references == 0
                });
            if remove_text {
                state.texts.remove(entry.text.as_ref());
            }
        }
        // 淘汰可能释放了当前文本的最后一个布局，必须在淘汰后重新查询驻留表。
        let (text_id, shared_text) = if let Some((shared, intern)) = state.texts.get_key_value(text)
        {
            (intern.id, Arc::clone(shared))
        } else {
            let shared: Arc<str> = Arc::from(text);
            let text_id = state.next_text_id;
            state.next_text_id = state.next_text_id.wrapping_add(1);
            state.texts.insert(
                Arc::clone(&shared),
                TextIntern {
                    id: text_id,
                    references: 0,
                },
            );
            (text_id, shared)
        };
        let key = TextLayoutCacheKey::new(font, text_id, opts);
        if let Some(intern) = state.texts.get_mut(shared_text.as_ref()) {
            intern.references = intern.references.saturating_add(1);
        }
        state.insertion_order.push_back(key);
        state.entries.insert(
            key,
            TextLayoutCacheEntry {
                text: shared_text,
                layout: Arc::clone(&layout),
            },
        );
        layout
    }

    pub(super) fn clear(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.entries.clear();
        state.insertion_order.clear();
        state.texts.clear();
        state.next_text_id = 0;
    }

    pub(super) fn memory_usage(&self) -> usize {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let text_bytes = state
            .texts
            .keys()
            .fold(0usize, |bytes, text| bytes.saturating_add(text.len()));
        state.entries.values().fold(text_bytes, |bytes, entry| {
            bytes
                .saturating_add(
                    entry
                        .layout
                        .glyphs
                        .len()
                        .saturating_mul(std::mem::size_of::<
                            crate::draw::resources::font::text_backend::PositionedGlyph,
                        >()),
                )
                .saturating_add(entry.layout.lines.len().saturating_mul(std::mem::size_of::<
                    crate::draw::resources::font::text_backend::LineInfo,
                >()))
        })
    }
}
