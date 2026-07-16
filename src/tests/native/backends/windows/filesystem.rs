use crate::native::backends::windows::filesystem::{read_variable_wide_path, WindowsSpecialDirs};
use crate::native::shared::SpecialDirProvider;
use crate::native::traits::SpecialDir;

#[test]
fn windows_temp_directory_resolves_to_an_existing_directory() {
    let path = WindowsSpecialDirs.special_dir(SpecialDir::Temp);

    assert!(!path.is_empty());
    assert!(std::path::Path::new(&path).is_dir());
}

#[test]
fn variable_wide_path_retries_after_a_capacity_hint() {
    let expected = format!(r"C:\{}", "nested\\".repeat(48));
    let wide: Vec<u16> = expected.encode_utf16().collect();
    let mut calls = 0;

    let actual = read_variable_wide_path(|buffer| {
        calls += 1;
        if buffer.len() <= wide.len() {
            return wide.len() + 1;
        }
        buffer[..wide.len()].copy_from_slice(&wide);
        buffer[wide.len()] = 0;
        wide.len()
    });

    assert_eq!(actual, expected);
    assert_eq!(calls, 2, "long paths must retry exactly once");
}

#[test]
fn variable_wide_path_rejects_an_unbounded_capacity_hint() {
    let actual = read_variable_wide_path(|_| usize::MAX);

    assert!(actual.is_empty());
}
