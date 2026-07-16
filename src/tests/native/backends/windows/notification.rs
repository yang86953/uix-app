use crate::native::backends::windows::notification::{
    copy_notification_text, notification_owner_action, NotificationOwnerAction,
};

#[test]
fn notification_owner_action_tracks_the_registered_window() {
    assert_eq!(
        notification_owner_action(0, 0),
        NotificationOwnerAction::Ignore
    );
    assert_eq!(
        notification_owner_action(0, 11),
        NotificationOwnerAction::Add { owner: 11 }
    );
    assert_eq!(
        notification_owner_action(11, 11),
        NotificationOwnerAction::Modify { owner: 11 }
    );
    assert_eq!(
        notification_owner_action(11, 22),
        NotificationOwnerAction::Move {
            previous: 11,
            owner: 22,
        }
    );
}

#[test]
fn notification_text_reserves_a_null_terminator() {
    let mut destination = [u16::MAX; 4];

    copy_notification_text(&mut destination, "abcd");

    assert_eq!(destination, [b'a' as u16, b'b' as u16, b'c' as u16, 0]);
}

#[test]
fn notification_text_keeps_utf16_surrogate_pairs_whole() {
    let expected: Vec<_> = "😀".encode_utf16().chain(std::iter::once(0)).collect();
    let mut fitting = [u16::MAX; 3];
    let mut too_small = [u16::MAX; 2];

    copy_notification_text(&mut fitting, "😀after");
    copy_notification_text(&mut too_small, "😀");

    assert_eq!(fitting, expected.as_slice());
    assert_eq!(too_small, [0, 0]);
}

#[test]
fn notification_text_stops_at_an_embedded_null() {
    let mut destination = [u16::MAX; 8];

    copy_notification_text(&mut destination, "visible\0hidden");

    let decoded = String::from_utf16_lossy(
        &destination[..destination
            .iter()
            .position(|unit| *unit == 0)
            .expect("notification text terminator")],
    );
    assert_eq!(decoded, "visible");
}
