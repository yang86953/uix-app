use crate::tests::common::*;
use crate::ui::widgets::{QRCode, Transfer, TransferItem, Upload, UploadStatus};
use crate::ui::{AccessibilityRole, SnapshotTransferItem};

#[test]
fn qrcode_builds_standard_matrix_with_three_finder_patterns() {
    let code = QRCode::new("https://uix.dev").error_level(2);

    assert!(code.is_valid());
    assert!(code.encoding_error().is_none());
    assert!(code.module_count() >= 21);
    assert_eq!(code.module_count() % 4, 1);

    let width = code.module_count();
    for (origin_x, origin_y) in [(0, 0), (width - 7, 0), (0, width - 7)] {
        for y in 0..7 {
            for x in 0..7 {
                let expected = x == 0
                    || x == 6
                    || y == 0
                    || y == 6
                    || ((2..=4).contains(&x) && (2..=4).contains(&y));
                assert_eq!(code.module(origin_x + x, origin_y + y), Some(expected));
            }
        }
    }
}

#[test]
fn qrcode_reports_oversized_payload_without_panicking() {
    let code = QRCode::new(&"x".repeat(10_000)).error_level(3);

    assert!(!code.is_valid());
    assert_eq!(code.module_count(), 0);
    assert_eq!(code.encoding_error(), Some("data too long"));
}

#[test]
fn qrcode_normalizes_size_and_error_level() {
    let code = QRCode::new("uix").size(f32::NAN).error_level(u8::MAX);

    assert_eq!(
        code.measure(Constraints::unconstrained()),
        Size::new(160.0, 160.0)
    );
    assert!(matches!(
        code.snapshot_fields(),
        SnapshotFields::QRCode { error_level: 3, .. }
    ));
}

#[test]
fn transfer_pointer_uses_node_local_coordinates_and_snapshots_live_membership() {
    let mut transfer = Transfer::new().source(vec![TransferItem {
        key: "a".into(),
        title: "A".into(),
        selected: false,
    }]);
    transfer.set_frame_for_test(Rect::new(100.0, 50.0, 500.0, 200.0));
    let select = SystemEvent::PointerDown {
        pos: Point::new(10.0, 30.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    let move_right = SystemEvent::PointerDown {
        pos: Point::new(240.0, 85.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(transfer.on_event(&select), EventResult::Handled);
    assert!(transfer.source_items()[0].selected);
    assert_eq!(transfer.on_event(&move_right), EventResult::Handled);
    assert!(transfer.source_items().is_empty());
    assert_eq!(transfer.target_items()[0].key, "a");
    assert_eq!(
        transfer.snapshot_fields(),
        SnapshotFields::Transfer {
            source: vec![],
            target: vec![SnapshotTransferItem {
                key: "a".into(),
                title: "A".into(),
                selected: false,
            }],
        }
    );
    assert_eq!(
        transfer
            .semantic_event(ComponentId::new(4), &move_right)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("a".to_string())
    );
}

#[test]
fn transfer_keyboard_selects_and_moves_active_rows() {
    let mut transfer = Transfer::new().source(vec![
        TransferItem {
            key: "a".into(),
            title: "A".into(),
            selected: false,
        },
        TransferItem {
            key: "b".into(),
            title: "B".into(),
            selected: false,
        },
    ]);

    assert_eq!(WidgetComponent::tab_index(&transfer), 1);
    assert_eq!(
        transfer.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    for key in [KeyCode::Down, KeyCode::Space, KeyCode::Enter] {
        assert_eq!(
            transfer.on_event(&SystemEvent::KeyDown {
                key,
                mods: KeyMod::NONE,
            }),
            EventResult::Handled
        );
    }
    assert_eq!(transfer.active_index(), 0);
    assert_eq!(
        transfer
            .source_items()
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        vec!["a"]
    );
    assert_eq!(transfer.target_items()[0].key, "b");

    assert_eq!(
        transfer.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(transfer.target_is_active());
    let accessibility = transfer.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::List);
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("1 source; 1 target; 0 selected")
    );
}

#[test]
fn upload_queues_real_file_drops_and_reports_the_accepted_files() {
    let mut upload = Upload::new()
        .accept(".JPG,.png,.pdf")
        .multiple(true)
        .max_count(2);
    let drop = SystemEvent::FileDrop {
        files: vec![
            r"C:\photos\cover.jpg".into(),
            "/tmp/ignored.txt".into(),
            "/tmp/report.PDF".into(),
            "/tmp/overflow.png".into(),
        ],
        position: Point::new(12.0, 8.0),
    };

    assert_eq!(upload.on_event(&drop), EventResult::Handled);
    assert_eq!(
        upload
            .files()
            .iter()
            .map(|file| (file.name.as_str(), file.status))
            .collect::<Vec<_>>(),
        vec![
            ("cover.jpg", UploadStatus::Pending),
            ("report.PDF", UploadStatus::Pending),
        ]
    );
    assert_eq!(
        upload
            .semantic_event(ComponentId::new(7), &drop)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("cover.jpg:pending,report.PDF:pending".into())
    );
}

#[test]
fn upload_does_not_invent_files_for_clicks_or_disabled_dragging() {
    let mut upload = Upload::new().drag(false);
    let click = SystemEvent::PointerDown {
        pos: Point::new(10.0, 10.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    let drop = SystemEvent::FileDrop {
        files: vec!["real.pdf".into()],
        position: Point::new(10.0, 10.0),
    };

    assert_eq!(upload.on_event(&click), EventResult::NotHandled);
    assert_eq!(upload.on_event(&drop), EventResult::NotHandled);
    assert!(upload.files().is_empty());
}

#[test]
fn upload_single_mode_and_progress_stay_within_public_bounds() {
    let mut upload = Upload::new().accept("png");
    let drop = SystemEvent::FileDrop {
        files: vec!["first.png".into(), "second.png".into()],
        position: Point::zero(),
    };

    assert_eq!(upload.on_event(&drop), EventResult::Handled);
    assert_eq!(upload.file_count(), 1);
    upload.update_progress(0, 4.0);
    assert_eq!(upload.files()[0].progress, 1.0);
    upload.update_progress(0, f32::NAN);
    assert_eq!(upload.files()[0].progress, 0.0);
}
