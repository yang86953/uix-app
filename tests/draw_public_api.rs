use std::sync::Arc;

use uix::core::{Point, Rect, Result};
use uix::draw::resources::font::text_backend::TextLayoutOptions;
use uix::draw::{
    Color, FontHandle, FontService, GlyphRaster, LineMetrics, PathBuilder, PathSegment,
    PositionedGlyph, TextBackend, TextLayout,
};
use uix::platform::services::FontSystemInfo;

#[test]
fn draw_color_contract_preserves_channels_and_alpha() {
    let color = Color::hex("#33669980");
    assert_eq!(color, Color::from_rgba(0x33, 0x66, 0x99, 0x80));
    assert_eq!(color.with_alpha(0xff).a, 0xff);
    assert_eq!(Color::RED.mix(&Color::BLUE, 0.0), Color::RED);
}

#[test]
fn draw_path_contract_exposes_geometry_without_renderer_internals() {
    let mut builder = PathBuilder::new();
    builder.polygon(&[
        Point::new(10.0, 20.0),
        Point::new(40.0, 20.0),
        Point::new(40.0, 60.0),
    ]);
    let path = builder.build();

    assert!(!path.is_empty());
    assert_eq!(path.bounds(), Some(Rect::new(10.0, 20.0, 30.0, 40.0)));
    assert!(matches!(path.segments().last(), Some(PathSegment::Close)));

    let translated = path.translated(5.0, -5.0);
    assert_eq!(translated.bounds(), Some(Rect::new(15.0, 15.0, 30.0, 40.0)));
}

#[derive(Debug, Default)]
struct RasterProbeBackend {
    // 每个槽位记录该测试字体是否能够生成可见栅格。
    fonts: Vec<Option<bool>>,
}

impl TextBackend for RasterProbeBackend {
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle> {
        let handle = FontHandle::new(self.fonts.len() as u32);
        self.fonts.push(Some(data == b"visible"));
        Ok(handle)
    }

    fn unload_font(&mut self, handle: &FontHandle) {
        if let Some(slot) = self.fonts.get_mut(handle.0 as usize) {
            *slot = None;
        }
    }

    fn is_valid(&self, handle: &FontHandle) -> bool {
        self.fonts
            .get(handle.0 as usize)
            .is_some_and(Option::is_some)
    }

    fn has_glyph(&self, font: &FontHandle, ch: char) -> bool {
        self.is_valid(font) && matches!(ch, 'A' | '0')
    }

    fn layout_text(&self, font: &FontHandle, _text: &str, _opts: &TextLayoutOptions) -> TextLayout {
        TextLayout {
            glyphs: vec![PositionedGlyph {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
                glyph_id: 1,
                char_index: 0,
                char_end: 1,
                bidi_level: 0,
                font: *font,
            }],
            lines: Vec::new(),
            width: 1.0,
            height: 1.0,
        }
    }

    fn rasterize_glyph(&self, font: &FontHandle, _glyph_id: u32, _pixel_size: f32) -> GlyphRaster {
        if self
            .fonts
            .get(font.0 as usize)
            .and_then(|slot| *slot)
            .unwrap_or(false)
        {
            GlyphRaster {
                width: 1,
                height: 1,
                coverage: Arc::from([255]),
                bearing_x: 0.0,
                bearing_y: 0.0,
                outline_mesh: None,
            }
        } else {
            GlyphRaster {
                width: 0,
                height: 0,
                coverage: Arc::from([]),
                bearing_x: 0.0,
                bearing_y: 0.0,
                outline_mesh: None,
            }
        }
    }

    fn horizontal_line_metrics(&self, _font: &FontHandle, _pixel_size: f32) -> Option<LineMetrics> {
        None
    }
}

struct RasterProbeSystemInfo {
    paths: Vec<String>,
}

impl FontSystemInfo for RasterProbeSystemInfo {
    fn default_font_paths(&self) -> Result<Vec<String>> {
        Ok(self.paths.clone())
    }

    fn probe_cjk_font_paths(&self) -> Vec<String> {
        Vec::new()
    }

    fn probe_family_font_path(&self, _family: &str) -> Option<String> {
        None
    }

    fn scan_fallback_font_path(&self) -> Option<String> {
        None
    }
}

struct ProbeFiles(std::path::PathBuf);

impl Drop for ProbeFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn system_font_discovery_skips_mapped_font_without_visible_raster() -> Result<()> {
    let directory = std::env::temp_dir().join(format!(
        "uix-font-raster-probe-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("draw")
    ));
    std::fs::create_dir_all(&directory)?;
    let _cleanup = ProbeFiles(directory.clone());
    let empty_path = directory.join("mapped-but-empty.ttf");
    let visible_path = directory.join("visible.ttf");
    std::fs::write(&empty_path, b"empty")?;
    std::fs::write(&visible_path, b"visible")?;

    let system_info = RasterProbeSystemInfo {
        paths: vec![
            empty_path.to_string_lossy().into_owned(),
            visible_path.to_string_lossy().into_owned(),
        ],
    };
    let mut fonts = FontService::new().with_text_backend(Box::new(RasterProbeBackend::default()));
    fonts.load_default_system_font(16.0, &system_info);

    assert_eq!(
        fonts.font_path(&fonts.loaded_font_handle),
        Some(visible_path.to_string_lossy().as_ref())
    );
    assert_eq!(fonts.loaded_font_handle, FontHandle::new(1));
    Ok(())
}
