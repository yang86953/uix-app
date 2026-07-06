use super::*;

#[test]
fn test_format_iso_ms() {
    let ts = Timestamp(1736937045123);
    assert_eq!(ts.format_iso_ms(), "2025-01-15T10:30:45.123Z");
}

#[test]
fn test_format_time_ms() {
    let ts = Timestamp(3661123);
    assert_eq!(ts.format_time_ms(), "01:01:01.123");
}

#[test]
fn test_format_datetime_utc() {
    let ts = Timestamp(1736937045123);
    assert_eq!(ts.format_datetime_utc(), "2025-01-15 10:30:45 UTC");
}

#[test]
fn test_from_system_time() {
    let st = SystemTime::now();
    let ts = Timestamp::from_system_time(st);
    let diff = (Timestamp::now().as_millis() as i64 - ts.as_millis() as i64).unsigned_abs();
    assert!(diff < 10_000, "conversion should be within 10 seconds");
}

#[test]
fn test_roundtrip_system_time() {
    let ts = Timestamp(1700000000000);
    let st: SystemTime = ts.into();
    let back = Timestamp::from_system_time(st);
    assert_eq!(ts, back);
}

#[test]
fn test_days_to_date_epoch() {
    let (y, m, d) = days_to_date(0);
    assert_eq!((y, m, d), (1970, 1, 1));
}

#[test]
fn test_days_to_date_known() {
    let (y, m, d) = days_to_date(20099);
    assert_eq!((y, m, d), (2025, 1, 11));
}
