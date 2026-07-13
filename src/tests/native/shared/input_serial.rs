use crate::native::shared::input_serial::InputSerial;

#[test]
fn input_serial_has_no_fabricated_default_and_tracks_latest_event() {
    let mut serial = InputSerial::default();
    assert_eq!(serial.latest(), None);

    serial.record(41);
    assert_eq!(serial.latest(), Some(41));

    serial.record(73);
    assert_eq!(serial.latest(), Some(73));
}
