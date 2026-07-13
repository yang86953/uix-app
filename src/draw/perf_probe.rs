//! Frame/present timing probes for evidence-first perf diagnosis.
//!
//! Enabled always at INFO; values are microseconds. Thread-local so the event
//! loop can attribute engine-managed present cost that sits inside `paint_ms`.

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

pub fn perf_probe_enabled() -> bool {
    std::env::var_os("UIX_PERF_PROBE").is_some()
}

pub fn skip_present_enabled() -> bool {
    std::env::var_os("UIX_PERF_SKIP_PRESENT").is_some()
}
