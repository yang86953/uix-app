//! `ui/theme/style/length.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

#[test]
fn percent_requires_reference_and_never_defaults_to_zero() {
    assert_eq!(StyleLength::Percent(50.0).resolve(Some(200.0)), Some(100.0));
    assert_eq!(StyleLength::Percent(50.0).resolve(None), None);
    assert_eq!(StyleLength::Percent(50.0).resolve(Some(f32::MAX)), None);
    assert_eq!(StyleLength::Percent(50.0).resolve(Some(f32::NAN)), None);
    assert_eq!(StyleLength::Percent(50.0).resolve(Some(0.0)), Some(0.0));
}

#[test]
fn calc_without_percent_resolves_without_reference() {
    assert_eq!(StyleLength::calc(24.0, 0.0).resolve(None), Some(24.0));
    assert_eq!(
        StyleLength::calc(-32.0, 100.0).resolve(Some(300.0)),
        Some(268.0)
    );
    assert_eq!(StyleLength::calc(-32.0, 100.0).resolve(None), None);
}

#[test]
fn bounds_keep_min_over_max_and_clamp_negative_to_zero() {
    let constraints = SizeConstraints {
        min_width: StyleLength::px(300.0),
        max_width: StyleLength::px(200.0),
        min_height: StyleLength::calc(-50.0, 10.0),
        max_height: StyleLength::Auto,
    };
    let bounds = constraints.resolve(PercentReference::definite(Size::new(400.0, 100.0)));
    assert_eq!(bounds.min, Size::new(300.0, 0.0));
    assert_eq!(bounds.max.w, 300.0);
    assert_eq!(bounds.max.h, f32::MAX);
    assert_eq!(bounds.clamp(Size::new(250.0, 40.0)), Size::new(300.0, 40.0));
}

// 拒绝路径（panic）由隔离的公开测试 style_s3_rejection_public_api 覆盖。
#[test]
fn legal_raw_fields_resolve_with_calc_results_clamped_to_zero() {
    let constraints = SizeConstraints {
        max_width: StyleLength::Calc {
            px: -80.0,
            percent: 50.0,
        },
        min_height: StyleLength::Percent(0.0),
        ..SizeConstraints::NONE
    };
    assert!(constraints.min_width.is_legal_size_bound());
    let bounds = constraints.resolve(PercentReference::definite(Size::new(100.0, 100.0)));
    assert_eq!(bounds.max.w, 0.0);
    assert_eq!(bounds.min.h, 0.0);
    assert_eq!(bounds.max.h, f32::MAX);
    // 参照未定的百分比不约束，px 仍生效。
    let unresolved = SizeConstraints {
        min_width: StyleLength::Percent(50.0),
        max_width: StyleLength::Px(120.0),
        ..SizeConstraints::NONE
    }
    .resolve(PercentReference::NONE);
    assert_eq!(unresolved.min.w, 0.0);
    assert_eq!(unresolved.max.w, 120.0);
}

// 拒绝路径（panic）由公开测试的 should_panic 覆盖，避免与全局 panic hook 单测并行干扰。
#[test]
fn legal_entries_keep_signed_px_and_negative_calc_terms() {
    assert_eq!(StyleLength::from(-5.0), StyleLength::Px(-5.0));
    assert_eq!(
        StyleLength::calc(-32.0, 100.0).expect_size_bound("max_width"),
        StyleLength::calc(-32.0, 100.0)
    );
    assert_eq!(
        StyleLength::Auto.expect_size_bound("max_height"),
        StyleLength::Auto
    );
    assert_eq!(
        StyleLength::Percent(0.0).expect_size_bound("min_width"),
        StyleLength::Percent(0.0)
    );
}
