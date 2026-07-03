//! types — 平台层数据类型测试（枚举 / 结构体 / 默认值）。

use uix_platform::types::{
    ConsoleColor, ControlSize, CursorType, DisplayInfo, KeyCode, KeyMod,
    MemoryInfo, MouseButton, OsInfo, ScrollDirection, SpecialDir, StatusLevel,
    TerminalCapabilities,
};
use uix_platform::presenter::NullPresenter;
use uix_platform::api::traits::IPresenter;

#[test]
fn console_color_default_value() {
    assert_eq!(ConsoleColor::Default as u8, 0);
}

#[test]
fn console_color_ordering() {
    assert!((ConsoleColor::Default as u8) < (ConsoleColor::Fatal as u8));
}

#[test]
fn console_color_debug_and_clone() {
    let c = ConsoleColor::Warn;
    let c2 = c;
    assert_eq!(format!("{:?}", c2), "Warn");
}

#[test]
fn terminal_capabilities_construction() {
    let tc = TerminalCapabilities { has_color: true, has_raw_mode: false, has_cursor_control: true };
    assert!(tc.has_color);
    assert!(!tc.has_raw_mode);
    assert!(tc.has_cursor_control);
}

#[test]
fn display_info_default() {
    let info = DisplayInfo::default();
    assert_eq!(info.dpi_scale, 1.0);
    assert!(!info.is_primary);
}

#[test]
fn display_info_custom() {
    use uix_platform::geometry::Rect;
    let info = DisplayInfo {
        bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
        dpi_scale: 2.0,
        is_primary: true,
    };
    assert_eq!(info.dpi_scale, 2.0);
    assert!(info.is_primary);
    assert_eq!(info.bounds.w, 1920.0);
}

#[test]
fn mouse_button_variants() {
    assert_eq!(MouseButton::None as u32, 0);
    assert_eq!(MouseButton::Left as u32, 1);
    assert_eq!(MouseButton::Right as u32, 2);
    assert_eq!(MouseButton::Middle as u32, 3);
    assert_eq!(MouseButton::X1 as u32, 4);
    assert_eq!(MouseButton::X2 as u32, 5);
}

#[test]
fn cursor_type_has_all_variants() {
    let variants = [
        CursorType::Arrow, CursorType::IBeam, CursorType::Crosshair,
        CursorType::Hand, CursorType::ResizeH, CursorType::ResizeV,
        CursorType::ResizeNE, CursorType::ResizeNW, CursorType::Move,
        CursorType::Wait, CursorType::NotAllowed, CursorType::Custom,
    ];
    assert_eq!(variants.len(), 12);
}

#[test]
fn key_code_variants_exist() {
    assert_eq!(KeyCode::Unknown as u32, 0);
    let _ = KeyCode::A;
    let _ = KeyCode::Z;
    let _ = KeyCode::Num0;
    let _ = KeyCode::Num9;
    let _ = KeyCode::F1;
    let _ = KeyCode::F12;
    let _ = KeyCode::Super;
}

#[test]
fn key_mod_none_is_zero() {
    let none = KeyMod::NONE;
    assert!(!none.contains(KeyMod::SHIFT));
    assert!(!none.intersects(KeyMod::CTRL));
}

#[test]
fn key_mod_constants_have_correct_bits() {
    assert!(KeyMod::SHIFT.contains(KeyMod::SHIFT));
    assert!(!KeyMod::SHIFT.contains(KeyMod::CTRL));
    assert!(KeyMod::CTRL.contains(KeyMod::CTRL));
    assert!(KeyMod::ALT.contains(KeyMod::ALT));
    assert!(KeyMod::SUPER.contains(KeyMod::SUPER));
}

#[test]
fn key_mod_contains() {
    let mods = KeyMod::CTRL | KeyMod::SHIFT;
    assert!(mods.contains(KeyMod::CTRL));
    assert!(mods.contains(KeyMod::SHIFT));
    assert!(!mods.contains(KeyMod::ALT));
    assert!(!mods.contains(KeyMod::SUPER));
    assert!(KeyMod::NONE.contains(KeyMod::NONE));
}

#[test]
fn key_mod_intersects() {
    let mods = KeyMod::CTRL | KeyMod::ALT;
    assert!(mods.intersects(KeyMod::CTRL));
    assert!(mods.intersects(KeyMod::ALT));
    assert!(mods.intersects(KeyMod::CTRL | KeyMod::ALT));
    assert!(!mods.intersects(KeyMod::SHIFT));
    assert!(!KeyMod::NONE.intersects(KeyMod::SHIFT));
}

#[test]
fn key_mod_bitor_combines() {
    let combo = KeyMod::CTRL | KeyMod::SHIFT;
    assert!(combo.contains(KeyMod::CTRL));
    assert!(combo.contains(KeyMod::SHIFT));
}

#[test]
fn key_mod_bitor_assign() {
    let mut combo = KeyMod::CTRL;
    combo |= KeyMod::ALT;
    assert!(combo.contains(KeyMod::CTRL));
    assert!(combo.contains(KeyMod::ALT));
}

#[test]
fn key_mod_debug_and_clone() {
    let a = KeyMod::CTRL | KeyMod::SHIFT;
    let b = a;
    assert_eq!(format!("{:?}", a), format!("{:?}", b));
}

#[test]
fn status_level_variants() {
    assert!((StatusLevel::Success as u8) < (StatusLevel::Error as u8));
}

#[test]
fn status_level_debug_clone_partial_eq() {
    assert_eq!(StatusLevel::Info, StatusLevel::Info);
    assert_ne!(StatusLevel::Success, StatusLevel::Error);
}

#[test]
fn control_size_variants() {
    assert_eq!(ControlSize::Small as u8, 0);
    assert_eq!(ControlSize::Medium as u8, 1);
    assert_eq!(ControlSize::Large as u8, 2);
}

#[test]
fn scroll_direction_can_scroll_x() {
    assert!(ScrollDirection::Horizontal.can_scroll_x());
    assert!(ScrollDirection::Both.can_scroll_x());
    assert!(!ScrollDirection::Vertical.can_scroll_x());
}

#[test]
fn scroll_direction_can_scroll_y() {
    assert!(ScrollDirection::Vertical.can_scroll_y());
    assert!(ScrollDirection::Both.can_scroll_y());
    assert!(!ScrollDirection::Horizontal.can_scroll_y());
}

#[test]
fn special_dir_variants() {
    assert_eq!(SpecialDir::Home as u8, 0);
    assert_eq!(SpecialDir::Temp as u8, 1);
    assert_eq!(SpecialDir::Executable as u8, 8);
}

#[test]
fn memory_info_construction() {
    let info = MemoryInfo {
        total_bytes: 16_000_000_000,
        available_bytes: 8_000_000_000,
        process_working_set: 500_000,
        process_private_bytes: 300_000,
    };
    assert_eq!(info.total_bytes, 16_000_000_000);
    assert_eq!(info.available_bytes, 8_000_000_000);
    assert_eq!(info.process_working_set, 500_000);
    assert_eq!(info.process_private_bytes, 300_000);
}

#[test]
fn os_info_construction() {
    let info = OsInfo {
        name: "Windows".into(),
        version: "10.0.22621".into(),
        build: "22621".into(),
        is_64bit: true,
    };
    assert_eq!(info.name, "Windows");
    assert_eq!(info.version, "10.0.22621");
    assert!(info.is_64bit);
}

#[test]
fn os_info_debug_and_clone() {
    let a = OsInfo {
        name: "Linux".into(),
        version: "6.8.0".into(),
        build: "generic".into(),
        is_64bit: true,
    };
    let b = a.clone();
    assert_eq!(a.name, b.name);
}

#[test]
fn null_presenter_new() {
    let p = NullPresenter::new();
    let _ = p;
}

#[test]
fn null_presenter_present_returns_ok() {
    let mut p = NullPresenter::new();
    let result = p.present(&[0u32; 100], 10, 10, Some((0, 0, 10, 10)));
    assert!(result.is_ok());
}

#[test]
fn null_presenter_present_without_dirty_rect() {
    let mut p = NullPresenter::new();
    let result = p.present(&[], 0, 0, None);
    assert!(result.is_ok());
}

#[test]
fn null_presenter_resize_returns_ok() {
    let mut p = NullPresenter::new();
    let result = p.resize(800, 600);
    assert!(result.is_ok());
}
