//! `ui/widget_runtime/build_viewport.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

#[test]
fn boundaries_are_inclusive_and_unknown_viewport_counts_as_zero() {
    let range = MediaBreakpoint {
        min_width: Some(800.0),
        max_width: Some(1279.0),
    };
    assert!(!range.matches(799.999));
    assert!(range.matches(800.0));
    assert!(range.matches(1279.0));
    assert!(!range.matches(1279.001));
    assert!(!uix_media_matches(Some(1.0), None));
    assert!(uix_media_matches(None, Some(0.0)));
}

#[test]
fn capture_records_breakpoints_and_scope_restores_outer_width() {
    begin_media_capture();
    {
        let _scope = BuildViewportScope::enter(1000.0);
        assert!(uix_media_matches(Some(800.0), None));
        assert!(!uix_media_matches(None, Some(480.0)));
        {
            let _inner = BuildViewportScope::enter(400.0);
            assert!(uix_media_matches(None, Some(480.0)));
        }
        assert_eq!(current_build_viewport_width(), Some(1000.0));
    }
    assert_eq!(current_build_viewport_width(), None);
    let recorded = end_media_capture();
    assert_eq!(recorded.len(), 2, "同一阈值只登记一次：{recorded:?}");
}

#[test]
fn tree_breakpoints_only_cross_when_truth_value_flips() {
    let mut set = MediaBreakpoints::default();
    set.replace(
        vec![MediaBreakpoint {
            min_width: Some(800.0),
            max_width: None,
        }],
        Some(1000.0),
    );
    assert!(!set.crosses(900.0));
    assert!(!set.crosses(800.0));
    assert!(set.crosses(799.0));
    set.merge(Vec::new(), Some(799.0));
    assert!(set.crosses(800.0));
    assert!(!set.crosses(700.0));
}
