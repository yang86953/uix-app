use crate::draw::pipeline::metrics::*;
use crate::draw::pipeline::RenderMetrics;

#[test]
fn metrics_accumulates_counters() {
    let mut m = RenderMetrics::default();
    m.record_layout();
    m.record_layout();
    m.record_paint();
    m.record_present(InvalidationSource::DirtyRegion);
    m.record_idle();
    assert_eq!(m.layout_calls, 2);
    assert_eq!(m.paint_calls, 1);
    assert_eq!(m.present_calls, 1);
    assert_eq!(m.idle_frames, 1);
    assert_eq!(m.last_invalidation, InvalidationSource::None);
}

#[test]
fn invalidation_source_labels() {
    assert_eq!(InvalidationSource::FirstFrame.label(), "first_frame");
    assert_eq!(InvalidationSource::AnimationPolling.label(), "animation");
}
