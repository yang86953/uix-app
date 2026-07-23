use crate::draw::resources::font::font_service::{
    CachedRaster, FontService, GlyphCache, GlyphCacheKey,
};
use crate::native::test_harness::fake_system_info::FakeSystemInfo;
use crate::native::traits::system::ISystemInfo;
use std::cell::Cell;
use std::sync::Arc;

/// 统计 `probe_cjk_font_paths` 调用，验证启动只走「主字体 + CJK」路径。
struct CountingSystemInfo {
    inner: FakeSystemInfo,
    cjk_probe_calls: Cell<usize>,
    default_paths: Vec<String>,
    family_path: Option<String>,
    cjk_paths: Vec<String>,
}

impl CountingSystemInfo {
    fn with_paths(paths: Vec<String>) -> Self {
        Self {
            inner: FakeSystemInfo::new(),
            cjk_probe_calls: Cell::new(0),
            default_paths: paths,
            family_path: None,
            cjk_paths: Vec::new(),
        }
    }

    fn with_family_path(path: String) -> Self {
        Self {
            family_path: Some(path),
            ..Self::with_paths(Vec::new())
        }
    }

    fn with_default_and_cjk_paths(default_path: String, cjk_path: String) -> Self {
        Self {
            cjk_paths: vec![cjk_path],
            ..Self::with_paths(vec![default_path])
        }
    }
}

impl ISystemInfo for CountingSystemInfo {
    fn os_info(&self) -> crate::native::traits::system::OsInfo {
        self.inner.os_info()
    }

    fn cpu_count(&self) -> u32 {
        self.inner.cpu_count()
    }

    fn memory_info(&self) -> crate::native::traits::system::MemoryInfo {
        self.inner.memory_info()
    }

    fn hostname(&self) -> String {
        self.inner.hostname()
    }

    fn username(&self) -> String {
        self.inner.username()
    }

    fn up_time(&self) -> u64 {
        self.inner.up_time()
    }

    fn default_font_paths(&self) -> Vec<String> {
        self.inner
            .default_font_calls
            .set(self.inner.default_font_calls.get() + 1);
        self.default_paths.clone()
    }

    fn probe_cjk_font_paths(&self) -> Vec<String> {
        self.cjk_probe_calls.set(self.cjk_probe_calls.get() + 1);
        self.cjk_paths.clone()
    }

    fn probe_family_font_path(&self, _family: &str) -> Option<String> {
        self.family_path.clone()
    }
}

#[test]
fn load_default_system_font_stops_after_first_primary_and_probes_cjk_once() {
    let lucide = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/lucide.ttf");
    // 同一主字体路径重复多次：旧实现会把后续全部当 fallback 同步装载。
    let info = CountingSystemInfo::with_paths(vec![
        lucide.to_string(),
        lucide.to_string(),
        lucide.to_string(),
    ]);

    let mut fonts = FontService::new();
    fonts.load_default_system_font(14.0, &info);

    assert_eq!(info.inner.default_font_calls.get(), 1);
    assert_eq!(info.cjk_probe_calls.get(), 1);
    // 主字体 1 + 未装 CJK；不得把重复路径再装成 fallback。
    assert_eq!(fonts.font_count(), 1);
    assert_eq!(fonts.fallback_count(), 0);
}

#[test]
fn configured_family_replaces_an_auxiliary_font_as_the_primary_handle() {
    let lucide = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/lucide.ttf");
    let info = CountingSystemInfo::with_family_path(lucide.to_owned());
    let mut fonts = FontService::new();
    let auxiliary = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load auxiliary font");

    fonts.set_font_family("Configured UI Font");
    fonts.load_default_system_font(14.0, &info);

    assert_ne!(fonts.loaded_font_handle, auxiliary);
    assert_eq!(
        fonts.font_family(&fonts.loaded_font_handle),
        Some("Configured UI Font")
    );
    assert_eq!(fonts.font_path(&fonts.loaded_font_handle), Some(lucide));
    assert_eq!(info.cjk_probe_calls.get(), 1);
}

#[test]
fn missing_configured_family_tries_each_default_font_candidate() {
    let lucide = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/lucide.ttf");
    let info = CountingSystemInfo::with_paths(vec![
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/missing.ttf").to_owned(),
        lucide.to_owned(),
    ]);
    let mut fonts = FontService::new();
    fonts.set_font_family("Missing Family");

    fonts.load_default_system_font(14.0, &info);

    assert!(fonts.is_valid(&fonts.loaded_font_handle));
    assert_eq!(fonts.font_path(&fonts.loaded_font_handle), Some(lucide));
    assert_eq!(info.inner.default_font_calls.get(), 1);
}

#[test]
fn cjk_probe_rejects_a_candidate_without_the_required_glyphs() {
    let lucide = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/lucide.ttf").to_owned();
    let info = CountingSystemInfo::with_default_and_cjk_paths(lucide.clone(), lucide);
    let mut fonts = FontService::new();

    fonts.load_default_system_font(14.0, &info);

    assert_eq!(info.cjk_probe_calls.get(), 1);
    assert_eq!(fonts.fallback_count(), 0);
}

fn cache_key(glyph_id: u32) -> GlyphCacheKey {
    GlyphCacheKey {
        font_idx: 0,
        glyph_id,
        pixel_size: 16,
    }
}

fn cached_raster(bytes: usize) -> CachedRaster {
    CachedRaster {
        // 故意让元数据与 coverage 长度不一致：统计应采用 Arc 实际保留的分配。
        width: usize::MAX,
        height: usize::MAX,
        coverage: Arc::from(vec![7; bytes]),
        bearing_x: 0.0,
        bearing_y: 0.0,
        outline_mesh: None,
    }
}

#[test]
fn glyph_cache_evicts_one_entry_at_capacity_instead_of_clearing_everything() {
    let cache = GlyphCache::with_max_entries(2);
    let first = cache_key(1);
    let second = cache_key(2);
    let third = cache_key(3);
    cache.insert(first.clone(), cached_raster(3));
    cache.insert(second.clone(), cached_raster(5));
    assert!(cache.get(&first).is_some(), "first becomes the hot entry");
    cache.insert(third.clone(), cached_raster(7));

    assert_eq!(cache.len(), 2);
    assert!(cache.get(&third).is_some());
    assert!(cache.get(&first).is_some());
    assert!(
        cache.get(&second).is_none(),
        "least-recently-used entry evicted"
    );
}

#[test]
fn glyph_cache_memory_usage_counts_retained_coverage_without_overflow() {
    let cache = GlyphCache::with_max_entries(2);
    cache.insert(cache_key(1), cached_raster(3));
    cache.insert(cache_key(2), cached_raster(5));

    assert_eq!(
        cache.memory_usage(),
        2 * std::mem::size_of::<GlyphCacheKey>() + 8
    );
}

#[test]
fn glyph_cache_enforces_a_retained_byte_budget() {
    let key_bytes = std::mem::size_of::<GlyphCacheKey>();
    let budget = key_bytes * 2 + 8;
    let cache = GlyphCache::with_limits(10, budget);
    cache.insert(cache_key(1), cached_raster(3));
    cache.insert(cache_key(2), cached_raster(5));
    cache.insert(cache_key(3), cached_raster(7));

    assert!(cache.memory_usage() <= budget);
    assert!(cache.get(&cache_key(3)).is_some());

    cache.insert(cache_key(4), cached_raster(budget + 1));
    assert!(cache.get(&cache_key(4)).is_none());
    assert!(cache.memory_usage() <= budget);
}

#[test]
fn glyph_rasterization_rejects_non_finite_sizes_and_bounds_huge_tofu() {
    let service = FontService::new();
    let font = crate::draw::FontHandle::new(0);
    let tofu = crate::draw::resources::font::text_backend::TOFU_GLYPH_ID;

    for size in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0] {
        assert!(service
            .rasterize_glyph(&font, tofu, size)
            .coverage
            .is_empty());
    }

    let raster = service.rasterize_glyph(&font, tofu, f32::MAX);
    assert!(!raster.coverage.is_empty());
    assert!(raster.width <= 512);
    assert!(raster.height <= 512);
    assert_eq!(raster.coverage.len(), raster.width * raster.height);
}

#[test]
fn layout_text_wraps_a_single_font_run_within_the_requested_width() {
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let opts = crate::draw::resources::font::text_backend::TextLayoutOptions {
        max_width: 40.0,
        max_height: 0.0,
        line_height: 21.0,
        word_wrap: true,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Top,
        font_size: 14.0,
    };

    let layout = fonts.layout_text(&font, "WWWWWWWW", &opts);

    assert!(
        layout.lines.len() > 1,
        "a single-font paragraph must wrap instead of overflowing: {layout:?}"
    );
    assert!(
        layout.lines.iter().all(|line| line.width <= 40.01),
        "every wrapped line must stay within max_width: {:?}",
        layout.lines
    );
    assert_eq!(layout.height, layout.lines.len() as f32 * 21.0);
}

#[test]
fn layout_text_prefers_a_word_boundary_before_splitting_a_latin_word() {
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let mut opts = crate::draw::resources::font::text_backend::TextLayoutOptions {
        max_width: f32::MAX,
        max_height: 0.0,
        line_height: 21.0,
        word_wrap: false,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Top,
        font_size: 14.0,
    };
    let unwrapped = fonts.layout_text(&font, "WWWW WWWW", &opts);
    let second_word = unwrapped
        .glyphs
        .iter()
        .find(|glyph| glyph.char_index == 5)
        .expect("second word start glyph");
    let prefix_width = second_word.x;
    let word_width = unwrapped.width - second_word.x;
    opts.max_width = prefix_width.max(word_width) + 0.01;
    opts.word_wrap = true;

    let wrapped = fonts.layout_text(&font, "WWWW WWWW", &opts);

    assert_eq!(wrapped.lines.len(), 2, "unexpected wrapping: {wrapped:?}");
    assert_eq!(wrapped.lines[0].end_char, 5);
    assert_eq!(wrapped.lines[1].start_char, 5);
    assert_eq!(
        wrapped.glyphs[wrapped.lines[1].glyph_start].char_index, 5,
        "the second line must start at the whole word, not inside it"
    );
    assert!(wrapped
        .lines
        .iter()
        .all(|line| line.width <= opts.max_width + 0.01));
}

#[test]
fn layout_text_collapses_whitespace_at_an_automatic_line_boundary() {
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let mut opts = crate::draw::resources::font::text_backend::TextLayoutOptions {
        max_width: f32::MAX,
        max_height: 0.0,
        line_height: 21.0,
        word_wrap: false,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Top,
        font_size: 14.0,
    };
    let text = "WWWW     WWWW";
    let unwrapped = fonts.layout_text(&font, text, &opts);
    let whitespace_start = unwrapped
        .glyphs
        .iter()
        .find(|glyph| glyph.char_index == 4)
        .expect("whitespace start")
        .x;
    let second_word = unwrapped
        .glyphs
        .iter()
        .find(|glyph| glyph.char_index == 9)
        .expect("second word start");
    opts.max_width = second_word.x.max(unwrapped.width - second_word.x) + 0.01;
    opts.word_wrap = true;

    let wrapped = fonts.layout_text(&font, text, &opts);

    assert_eq!(wrapped.lines.len(), 2, "unexpected wrapping: {wrapped:?}");
    assert!(
        (wrapped.lines[0].width - whitespace_start).abs() < 0.01,
        "trailing wrap whitespace must not enlarge the visible line: {:?}",
        wrapped.lines
    );
    let second_line = wrapped.lines[1];
    let second_line_word = wrapped.glyphs[second_line.glyph_start..]
        .iter()
        .find(|glyph| glyph.char_index == 9)
        .expect("second-line word");
    assert!(second_line_word.x.abs() < 0.01);
}

#[test]
fn layout_text_does_not_create_a_whitespace_only_line_after_wrapping() {
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let mut opts = crate::draw::resources::font::text_backend::TextLayoutOptions {
        max_width: f32::MAX,
        max_height: 0.0,
        line_height: 21.0,
        word_wrap: false,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Top,
        font_size: 14.0,
    };
    let text = "WWWW     W";
    let unwrapped = fonts.layout_text(&font, text, &opts);
    opts.max_width = unwrapped
        .glyphs
        .iter()
        .find(|glyph| glyph.char_index == 4)
        .expect("whitespace start")
        .x
        + 0.01;
    opts.word_wrap = true;

    let wrapped = fonts.layout_text(&font, text, &opts);

    assert_eq!(wrapped.lines.len(), 2, "unexpected wrapping: {wrapped:?}");
    assert!(wrapped.lines.iter().all(|line| line.glyph_count > 0));
    let final_word = wrapped
        .glyphs
        .iter()
        .find(|glyph| glyph.char_index == 9)
        .expect("final word");
    assert!(final_word.x.abs() < 0.01);
}

#[test]
fn layout_text_normalizes_crlf_and_lone_carriage_returns_to_line_breaks() {
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let opts = crate::draw::resources::font::text_backend::TextLayoutOptions {
        max_width: 200.0,
        max_height: 0.0,
        line_height: 21.0,
        word_wrap: true,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Top,
        font_size: 14.0,
    };

    let layout = fonts.layout_text(&font, "A\r\nB\rC", &opts);

    assert_eq!(layout.lines.len(), 3);
    assert_eq!(layout.lines[0].start_char, 0);
    assert_eq!(layout.lines[0].end_char, 1);
    assert_eq!(layout.lines[1].start_char, 3);
    assert_eq!(layout.lines[1].end_char, 4);
    assert_eq!(layout.lines[2].start_char, 5);
    assert_eq!(layout.lines[2].end_char, 6);
    assert_eq!(
        layout
            .glyphs
            .iter()
            .map(|glyph| glyph.char_index)
            .collect::<Vec<_>>(),
        vec![0, 3, 5]
    );
}

#[test]
fn layout_text_keeps_opening_punctuation_with_the_following_character() {
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let mut opts = crate::draw::resources::font::text_backend::TextLayoutOptions {
        max_width: f32::MAX,
        max_height: 0.0,
        line_height: 21.0,
        word_wrap: false,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Top,
        font_size: 14.0,
    };
    let unwrapped = fonts.layout_text(&font, "WW(A", &opts);
    let final_glyph = unwrapped
        .glyphs
        .iter()
        .find(|glyph| glyph.char_index == 3)
        .expect("final glyph");
    opts.max_width = final_glyph.x + final_glyph.width * 0.5;
    opts.word_wrap = true;

    let wrapped = fonts.layout_text(&font, "WW(A", &opts);

    assert_eq!(wrapped.lines.len(), 2, "unexpected wrapping: {wrapped:?}");
    assert_eq!(wrapped.lines[0].end_char, 2);
    assert_eq!(wrapped.lines[1].start_char, 2);
    assert_eq!(
        wrapped.glyphs[wrapped.lines[1].glyph_start].char_index, 2,
        "an opening parenthesis must move with the following character"
    );
}

#[test]
fn layout_text_keeps_closing_punctuation_with_the_previous_character() {
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let opts = crate::draw::resources::font::text_backend::TextLayoutOptions {
        max_width: 31.0,
        max_height: 0.0,
        line_height: 21.0,
        word_wrap: true,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Top,
        font_size: 14.0,
    };

    let layout = fonts.layout_text(&font, "WWWW，", &opts);

    assert_eq!(layout.lines.len(), 2);
    assert_eq!(layout.lines[0].end_char, 3);
    assert_eq!(layout.lines[1].start_char, 3);
    assert_eq!(
        layout.glyphs[layout.lines[1].glyph_start].char_index, 3,
        "the second line must start with the character before the punctuation"
    );
    assert!(layout.lines.iter().all(|line| line.width <= 31.01));
}

#[test]
fn layout_text_preserves_consecutive_and_trailing_explicit_lines() {
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let opts = crate::draw::resources::font::text_backend::TextLayoutOptions {
        max_width: 200.0,
        max_height: 0.0,
        line_height: 21.0,
        word_wrap: true,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Top,
        font_size: 14.0,
    };

    let layout = fonts.layout_text(&font, "A\n\nB\n", &opts);

    assert_eq!(layout.lines.len(), 4);
    assert_eq!(layout.lines[1].glyph_count, 0);
    assert_eq!(layout.lines[3].glyph_count, 0);
    assert_eq!(layout.height, 84.0);
}

#[test]
fn layout_text_applies_vertical_alignment_once_to_the_complete_layout() {
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let opts = crate::draw::resources::font::text_backend::TextLayoutOptions {
        max_width: 200.0,
        max_height: 100.0,
        line_height: 20.0,
        word_wrap: false,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Bottom,
        font_size: 14.0,
    };

    let layout = fonts.layout_text(&font, "AB", &opts);

    assert_eq!(layout.height, 100.0);
    assert_eq!(layout.lines.len(), 1);
    assert!((layout.lines[0].y - 80.0).abs() < 0.01);
    assert!(layout.glyphs.iter().all(|glyph| glyph.y >= 80.0));
}
