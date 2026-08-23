// 复用本模块的检测函数与类型。
use super::*;

// 构造一次带默认现场的检测调用。
fn detect(diag: &mut super::super::FrameDiagnostics, frame_us: Duration) {
    // 只有总耗时参与判定，阶段耗时与现场字段不进入状态机。
    record_slow_frame(
        diag,
        WindowId::ROOT,
        None,
        frame_us,
        Duration::ZERO,
        Duration::ZERO,
        Duration::ZERO,
        Duration::ZERO,
        false,
        0.0,
        0,
        0,
        0.0,
        0,
        false,
        0,
        InvalidationSource::None,
    );
}

#[test]
fn explicit_threshold_parser_rejects_zero_and_invalid_values() {
    assert_eq!(
        parse_slow_frame_threshold(std::ffi::OsStr::new("16")),
        Some(Duration::from_millis(16))
    );
    assert_eq!(parse_slow_frame_threshold(std::ffi::OsStr::new("0")), None);
    assert_eq!(
        parse_slow_frame_threshold(std::ffi::OsStr::new("fast")),
        None
    );
}

// 验证正常帧不触发、慢帧开始、峰值跟踪、恢复结束的完整状态迁移。
#[test]
fn records_start_peak_and_end_across_threshold() {
    // 默认阈值 100ms 下构造诊断统计。
    let mut diag = super::super::FrameDiagnostics::default();
    // 正常帧不进入卡顿段。
    detect(&mut diag, Duration::from_millis(20));
    assert!(!diag.slow_active);
    // 第一帧慢帧进入卡顿段。
    detect(&mut diag, Duration::from_millis(500));
    assert!(diag.slow_active);
    // 更慢的帧更新峰值。
    detect(&mut diag, Duration::from_millis(900));
    assert!(diag.slow_active);
    assert_eq!(diag.slow_peak, Duration::from_millis(900));
    // 恢复帧结束卡顿段并复位峰值。
    detect(&mut diag, Duration::from_millis(20));
    assert!(!diag.slow_active);
    assert_eq!(diag.slow_peak, Duration::ZERO);
    assert!(diag.slow_since.is_none());
}

// 验证持续慢帧段内只保持激活状态，不重复开始。
#[test]
fn stays_active_during_continuous_slow_frames() {
    // 构造诊断统计。
    let mut diag = super::super::FrameDiagnostics::default();
    // 连续两帧均慢。
    detect(&mut diag, Duration::from_millis(400));
    assert!(diag.slow_active);
    detect(&mut diag, Duration::from_millis(450));
    assert!(diag.slow_active);
    // 峰值保持最大帧耗时。
    assert_eq!(diag.slow_peak, Duration::from_millis(450));
    // 开始时刻只记录一次。
    let since = diag.slow_since;
    detect(&mut diag, Duration::from_millis(300));
    assert_eq!(diag.slow_since, since);
}
