//! Frame/present timing probes for evidence-first perf diagnosis.
//!
//! Values are microseconds. Thread-local so the event loop can attribute
//! engine-managed present cost and record-path sub-stages.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default)]
pub struct PresentProbeSample {
    pub present_us: u128,
    pub upload_copy_us: u128,
    pub fence_wait_us: u128,
    pub submit_present_us: u128,
    pub pixels: u64,
    pub damage_full: u8,
    pub skipped: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PaintProbeSample {
    pub layer_build_us: u128,
    pub record_us: u128,
    pub execute_us: u128,
    pub end_frame_us: u128,
    pub strategy_full: u8,
    // record sub-stages (accumulated during layer_tree.render)
    pub picture_raster_us: u128,
    pub picture_blit_us: u128,
    pub direct_paint_us: u128,
    pub pictures_rasterized: u32,
    pub pictures_blit: u32,
    pub picture_pixels: u64,
    pub widgets_painted: u32,
    pub text_draws: u32,
    pub text_us: u128,
    pub cpu_flush_us: u128,
    pub cpu_flushes: u32,
}

thread_local! {
    static LAST_PRESENT: Cell<PresentProbeSample> = const { Cell::new(PresentProbeSample {
        present_us: 0,
        upload_copy_us: 0,
        fence_wait_us: 0,
        submit_present_us: 0,
        pixels: 0,
        damage_full: 0,
        skipped: 0,
    }) };
    static LAST_PAINT: Cell<PaintProbeSample> = const { Cell::new(PaintProbeSample {
        layer_build_us: 0,
        record_us: 0,
        execute_us: 0,
        end_frame_us: 0,
        strategy_full: 0,
        picture_raster_us: 0,
        picture_blit_us: 0,
        direct_paint_us: 0,
        pictures_rasterized: 0,
        pictures_blit: 0,
        picture_pixels: 0,
        widgets_painted: 0,
        text_draws: 0,
        text_us: 0,
        cpu_flush_us: 0,
        cpu_flushes: 0,
    }) };
    static RECORD_ACC: Cell<PaintProbeSample> = const { Cell::new(PaintProbeSample {
        layer_build_us: 0,
        record_us: 0,
        execute_us: 0,
        end_frame_us: 0,
        strategy_full: 0,
        picture_raster_us: 0,
        picture_blit_us: 0,
        direct_paint_us: 0,
        pictures_rasterized: 0,
        pictures_blit: 0,
        picture_pixels: 0,
        widgets_painted: 0,
        text_draws: 0,
        text_us: 0,
        cpu_flush_us: 0,
        cpu_flushes: 0,
    }) };
}

pub fn record_present(sample: PresentProbeSample) {
    LAST_PRESENT.with(|cell| cell.set(sample));
}

pub fn take_present() -> PresentProbeSample {
    LAST_PRESENT.with(|cell| {
        let sample = cell.get();
        cell.set(PresentProbeSample::default());
        sample
    })
}

pub fn record_paint(sample: PaintProbeSample) {
    LAST_PAINT.with(|cell| cell.set(sample));
}

pub fn take_paint() -> PaintProbeSample {
    LAST_PAINT.with(|cell| {
        let sample = cell.get();
        cell.set(PaintProbeSample::default());
        sample
    })
}

pub fn begin_record_acc() {
    RECORD_ACC.with(|cell| cell.set(PaintProbeSample::default()));
}

pub fn add_picture_raster(us: u128, pixels: u64) {
    RECORD_ACC.with(|cell| {
        let mut s = cell.get();
        s.picture_raster_us = s.picture_raster_us.saturating_add(us);
        s.pictures_rasterized = s.pictures_rasterized.saturating_add(1);
        s.picture_pixels = s.picture_pixels.saturating_add(pixels);
        cell.set(s);
    });
}

pub fn add_picture_blit(us: u128) {
    RECORD_ACC.with(|cell| {
        let mut s = cell.get();
        s.picture_blit_us = s.picture_blit_us.saturating_add(us);
        s.pictures_blit = s.pictures_blit.saturating_add(1);
        cell.set(s);
    });
}

pub fn add_direct_paint(us: u128) {
    RECORD_ACC.with(|cell| {
        let mut s = cell.get();
        s.direct_paint_us = s.direct_paint_us.saturating_add(us);
        cell.set(s);
    });
}

pub fn add_widget_painted() {
    RECORD_ACC.with(|cell| {
        let mut s = cell.get();
        s.widgets_painted = s.widgets_painted.saturating_add(1);
        cell.set(s);
    });
}

pub fn add_text_draw(us: u128) {
    RECORD_ACC.with(|cell| {
        let mut s = cell.get();
        s.text_draws = s.text_draws.saturating_add(1);
        s.text_us = s.text_us.saturating_add(us);
        cell.set(s);
    });
}

pub fn add_cpu_flush(us: u128) {
    RECORD_ACC.with(|cell| {
        let mut s = cell.get();
        s.cpu_flush_us = s.cpu_flush_us.saturating_add(us);
        s.cpu_flushes = s.cpu_flushes.saturating_add(1);
        cell.set(s);
    });
}

pub fn take_record_acc() -> PaintProbeSample {
    RECORD_ACC.with(|cell| {
        let sample = cell.get();
        cell.set(PaintProbeSample::default());
        sample
    })
}

pub fn perf_probe_enabled() -> bool {
    std::env::var_os("UIX_PERF_PROBE").is_some()
}

pub fn skip_present_enabled() -> bool {
    std::env::var_os("UIX_PERF_SKIP_PRESENT").is_some()
}

/// A/B: skip Picture offscreen raster (blit nothing / fall through). Proves
/// whether picture_raster dominates record time.
pub fn skip_picture_raster_enabled() -> bool {
    std::env::var_os("UIX_PERF_SKIP_PICTURE_RASTER").is_some()
}
