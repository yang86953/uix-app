//! 字体服务拥有的有界文本布局缓存。

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use crate::draw::resources::font::text_backend::{TextLayout, TextLayoutOptions};
use crate::draw::{FontHandle, HAlign, VAlign};

// 页面切换只需要覆盖近期可见控件；固定上限避免动态文本导致无界增长。
const TEXT_LAYOUT_CACHE_CAPACITY: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct TextLayoutCacheKey {
    font: u32,
    text: Box<str>,
    max_width: u32,
    max_height: u32,
    line_height: u32,
    word_wrap: bool,
    h_align: u8,
    v_align: u8,
    font_size: u32,
}

impl TextLayoutCacheKey {
    fn new(font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> Self {
        Self {
            font: font.0,
            text: text.into(),
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

#[derive(Default)]
struct TextLayoutCacheState {
    entries: HashMap<TextLayoutCacheKey, TextLayout>,
    insertion_order: VecDeque<TextLayoutCacheKey>,
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
    ) -> Option<TextLayout> {
        let key = TextLayoutCacheKey::new(font, text, opts);
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.entries.get(&key).cloned()
    }

    pub(super) fn insert(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
        layout: TextLayout,
    ) {
        let key = TextLayoutCacheKey::new(font, text, opts);
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.entries.contains_key(&key) {
            return;
        }
        while state.entries.len() >= TEXT_LAYOUT_CACHE_CAPACITY {
            let Some(oldest) = state.insertion_order.pop_front() else {
                state.entries.clear();
                break;
            };
            state.entries.remove(&oldest);
        }
        state.insertion_order.push_back(key.clone());
        state.entries.insert(key, layout);
    }

    pub(super) fn clear(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.entries.clear();
        state.insertion_order.clear();
    }

    pub(super) fn memory_usage(&self) -> usize {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.entries.iter().fold(0usize, |bytes, (key, layout)| {
            bytes
                .saturating_add(key.text.len())
                .saturating_add(layout.glyphs.len().saturating_mul(std::mem::size_of::<
                    crate::draw::resources::font::text_backend::PositionedGlyph,
                >()))
                .saturating_add(layout.lines.len().saturating_mul(std::mem::size_of::<
                    crate::draw::resources::font::text_backend::LineInfo,
                >()))
        })
    }
}
