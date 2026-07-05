//! Phase 0 绘图层度量基线测试（不依赖 UI）。

use uix::draw::pipeline::{InvalidationSource, RenderMetrics};

#[test]
fn render_baseline_metrics_reset() {
    let mut m = RenderMetrics::default();
    m.record_layout();
    m.record_present(InvalidationSource::DirtyRegion);
    m.reset();
    assert_eq!(m.layout_calls, 0);
    assert_eq!(m.present_calls, 0);
    assert_eq!(m.last_invalidation, InvalidationSource::None);
}

#[test]
fn render_baseline_invalidation_labels_distinct() {
    let sources = [
        InvalidationSource::None,
        InvalidationSource::FirstFrame,
        InvalidationSource::DirtyRegion,
        InvalidationSource::AnimationPolling,
        InvalidationSource::LayoutEvent,
    ];
    let labels: Vec<_> = sources.iter().map(|s| s.label()).collect();
    let unique: std::collections::HashSet<_> = labels.iter().copied().collect();
    assert_eq!(unique.len(), sources.len());
}
