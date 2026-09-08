//! A font file is not a font face: preserve collection indices through discovery,
//! shaping and rasterization, without relying on fonts installed on the machine.
use ab_glyph::{Font, FontRef};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use uix::draw::resources::font::text_backend::TextLayoutOptions;
use uix::draw::resources::font::text_backends::ab_glyph::AbGlyphBackend;
use uix::draw::{FontService, HAlign, TextBackend, VAlign};
use uix::platform::services::{FontSystemInfo, SystemFontSource};

const FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf");

fn table(data: &[u8], tag: &[u8]) -> usize {
    let count = u16::from_be_bytes(data[4..6].try_into().unwrap()) as usize;
    (0..count)
        .map(|i| 12 + 16 * i)
        .find(|&i| &data[i..i + 4] == tag)
        .unwrap()
}

fn font_with_em(em: u16) -> Vec<u8> {
    let mut data = FONT.to_vec();
    let record = table(&data, b"head");
    let head = u32::from_be_bytes(data[record + 8..record + 12].try_into().unwrap()) as usize;
    data[head + 18..head + 20].copy_from_slice(&em.to_be_bytes());
    data
}

// Build a valid collection of two complete SFNT faces. Every table offset is
// relative to the collection, not to the start of its individual face.
fn collection() -> Vec<u8> {
    let mut data = b"ttcf\0\x01\0\0\0\0\0\x02\0\0\0\0\0\0\0\0".to_vec();
    for (index, mut face) in [font_with_em(2000), FONT.to_vec()].into_iter().enumerate() {
        while data.len() % 4 != 0 {
            data.push(0);
        }
        let base = data.len() as u32;
        data[12 + index * 4..16 + index * 4].copy_from_slice(&base.to_be_bytes());
        let count = u16::from_be_bytes(face[4..6].try_into().unwrap()) as usize;
        for i in 0..count {
            let offset = 12 + 16 * i + 8;
            let old = u32::from_be_bytes(face[offset..offset + 4].try_into().unwrap());
            face[offset..offset + 4].copy_from_slice(&(old + base).to_be_bytes());
        }
        data.extend_from_slice(&face);
    }
    data
}

// Reuse the bundled outlines but expose only ASCII in cmap, so the CJK fallback
// branch has a real Latin primary and remains independent of OS font installs.
fn latin_font() -> Vec<u8> {
    let font = FontRef::try_from_slice(FONT).unwrap();
    let mut data = FONT.to_vec();
    while data.len() % 4 != 0 {
        data.push(0);
    }
    let start = data.len() as u32;
    let mut cmap = vec![0, 0, 0, 1, 0, 3, 0, 10, 0, 0, 0, 12, 0, 12, 0, 0];
    cmap.extend_from_slice(&(16u32 + 95 * 12).to_be_bytes());
    cmap.extend_from_slice(&0u32.to_be_bytes());
    cmap.extend_from_slice(&95u32.to_be_bytes());
    for ch in 32u32..=126 {
        cmap.extend_from_slice(&ch.to_be_bytes());
        cmap.extend_from_slice(&ch.to_be_bytes());
        cmap.extend_from_slice(
            &u32::from(font.glyph_id(char::from_u32(ch).unwrap()).0).to_be_bytes(),
        );
    }
    let record = table(&data, b"cmap");
    data[record + 8..record + 12].copy_from_slice(&start.to_be_bytes());
    data[record + 12..record + 16].copy_from_slice(&(cmap.len() as u32).to_be_bytes());
    data.extend_from_slice(&cmap);
    data
}

struct Fixture {
    root: PathBuf,
    collection: String,
    latin: String,
}
impl Fixture {
    fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/font-collection-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&root).unwrap();
        let collection = root.join("two-faces.ttc");
        let latin = root.join("latin.otf");
        std::fs::write(&collection, collection_bytes()).unwrap();
        std::fs::write(&latin, latin_font()).unwrap();
        Self {
            root,
            collection: collection.to_string_lossy().into(),
            latin: latin.to_string_lossy().into(),
        }
    }
    fn source(&self, face_index: u32) -> SystemFontSource {
        SystemFontSource {
            path: self.collection.clone(),
            face_index,
        }
    }
    fn mapped(&self) -> memmap2::Mmap {
        let file = std::fs::File::open(&self.collection).unwrap();
        // SAFETY: each fixture owns an immutable file for the entire test.
        unsafe { memmap2::Mmap::map(&file) }.unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn collection_bytes() -> &'static [u8] {
    static DATA: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    DATA.get_or_init(collection)
}
fn options() -> TextLayoutOptions {
    TextLayoutOptions {
        max_width: f32::INFINITY,
        max_height: 0.0,
        line_height: 55.0,
        word_wrap: false,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
        font_size: 44.0,
    }
}

#[test]
fn indexed_owned_and_mapped_faces_share_shaping_and_raster_identity() {
    let fixture = Fixture::new();
    let mut backend = AbGlyphBackend::new();
    let reference = backend.load_font(FONT).unwrap();
    let zero = backend.load_font(collection_bytes()).unwrap();
    let owned = backend.load_font_index(collection_bytes(), 1).unwrap();
    let mapped = backend.load_font_mapped_index(fixture.mapped(), 1).unwrap();
    let expected = backend.layout_text(&reference, "修复桌 Hg", &options());
    assert!(backend.layout_text(&zero, "修复桌 Hg", &options()).width < expected.width * 0.6);
    for handle in [owned, mapped] {
        let actual = backend.layout_text(&handle, "修复桌 Hg", &options());
        assert_eq!(
            actual.width, expected.width,
            "shaping selected the wrong collection face"
        );
        for (a, e) in actual.glyphs.iter().zip(&expected.glyphs) {
            assert_eq!((a.glyph_id, a.x, a.y), (e.glyph_id, e.x, e.y));
            let a = backend.rasterize_glyph(&handle, a.glyph_id, 44.0);
            let e = backend.rasterize_glyph(&reference, e.glyph_id, 44.0);
            assert_eq!(
                (a.width, a.height, a.bearing_x, a.bearing_y),
                (e.width, e.height, e.bearing_x, e.bearing_y)
            );
            assert_eq!(
                a.coverage, e.coverage,
                "raster and shaping must select the same face"
            );
        }
    }
    backend.unload_font(&owned);
    assert!(!backend.is_valid(&owned));
    assert_eq!(
        backend.layout_text(&mapped, "修复桌 Hg", &options()).width,
        expected.width
    );
}

#[test]
fn invalid_collection_indices_fail_without_installing_a_wrong_face() {
    let fixture = Fixture::new();
    let mut backend = AbGlyphBackend::new();
    assert!(backend.load_font_index(collection_bytes(), 2).is_err());
    assert!(
        backend
            .load_font_mapped_index(fixture.mapped(), u32::MAX)
            .is_err()
    );
    assert!(backend.load_font_index(FONT, 1).is_err());
    let first = backend.load_font_index(FONT, 0).unwrap();
    assert_eq!(first.0, 0, "rejected faces must not allocate a live slot");
}

struct Provider<'a> {
    fixture: &'a Fixture,
    cjk: bool,
    reject_first: bool,
}
impl FontSystemInfo for Provider<'_> {
    fn default_font_paths(&self) -> uix::core::Result<Vec<String>> {
        Ok(vec![if self.cjk {
            self.fixture.latin.clone()
        } else {
            self.fixture.collection.clone()
        }])
    }
    fn probe_cjk_font_paths(&self) -> Vec<String> {
        vec![self.fixture.collection.clone()]
    }
    fn probe_family_font_path(&self, _: &str) -> Option<String> {
        Some(self.fixture.collection.clone())
    }
    fn scan_fallback_font_path(&self) -> Option<String> {
        None
    }
    fn default_font_sources(&self) -> uix::core::Result<Vec<SystemFontSource>> {
        Ok(if self.cjk {
            vec![self.fixture.latin.clone().into()]
        } else {
            self.probe_cjk_font_sources()
        })
    }
    fn probe_cjk_font_sources(&self) -> Vec<SystemFontSource> {
        let mut sources = Vec::new();
        if self.reject_first {
            sources.push(self.fixture.source(99));
        }
        sources.push(self.fixture.source(1));
        sources
    }
    fn probe_family_font_source(&self, _: &str) -> Option<SystemFontSource> {
        Some(self.fixture.source(1))
    }
}

#[test]
fn system_default_family_and_cjk_paths_preserve_nonzero_face_indices() {
    let fixture = Fixture::new();
    for (cjk, family, reject_first) in [
        (false, false, false),
        (false, true, false),
        (true, false, false),
        (true, false, true),
        (false, false, true),
    ] {
        let provider = Provider {
            fixture: &fixture,
            cjk,
            reject_first,
        };
        let mut service = FontService::new();
        if family {
            service.set_font_family("fixture second face");
        }
        service.load_default_system_font(44.0, &provider);
        assert_eq!(service.fallback_count(), usize::from(cjk));
        let handle = service.loaded_font_handle;
        let layout = service.layout_text_shared(&handle, "修复桌", &options());
        assert!(
            (layout.width - 132.0).abs() < 0.01,
            "wrong collection face: cjk={cjk}, family={family}, rejected={reject_first}, width={}",
            layout.width
        );
        let mut reference = AbGlyphBackend::new();
        let font = reference.load_font(FONT).unwrap();
        let expected = reference.layout_text(&font, "修复桌", &options());
        for (actual, expected) in layout.glyphs.iter().zip(&expected.glyphs) {
            let actual = service.rasterize_glyph(&actual.font, actual.glyph_id, 44.0);
            let expected = reference.rasterize_glyph(&font, expected.glyph_id, 44.0);
            assert_eq!(actual.coverage, expected.coverage);
        }
    }
}
