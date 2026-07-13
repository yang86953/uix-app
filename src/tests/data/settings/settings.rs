use std::fs;
use std::path::{Path, PathBuf};

use crate::data::settings::settings::*;
use crate::tests::common::*;

fn temp_settings_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("uix_settings_{name}_{}.json", std::process::id()))
}

fn remove_if_present(path: &Path) {
    let _ = fs::remove_file(path);
}

#[test]
fn parse_json_flat_accepts_supported_inputs() {
    let cases: &[(&str, &[(&str, &str)])] = &[
        ("{}", &[]),
        (
            r#"{"a": "1", "b": "2", "c": "3"}"#,
            &[("a", "1"), ("b", "2"), ("c", "3")],
        ),
        (r#"{  "key"  :  "val"  }"#, &[("key", "val")]),
        ("{\n  \"key\": \"val\"\n}", &[("key", "val")]),
        (r#"{"key": "hello\"world"}"#, &[("key", "hello\"world")]),
        (r#"{"path": "C:\\Users"}"#, &[("path", "C:\\Users")]),
        (r#"{"msg": "line1\nline2"}"#, &[("msg", "line1\nline2")]),
        (r#"{"col": "a\tb"}"#, &[("col", "a\tb")]),
        (r#"{"he\"llo": "world"}"#, &[("he\"llo", "world")]),
        (r#"{"a": "1",}"#, &[("a", "1")]),
    ];

    for &(input, expected) in cases {
        let parsed = parse_json_flat(input)
            .unwrap_or_else(|error| panic!("expected supported input {input:?}, got {error:?}"));
        assert_eq!(parsed.len(), expected.len(), "input: {input:?}");
        for &(key, value) in expected {
            assert_eq!(
                parsed.get(key).map(String::as_str),
                Some(value),
                "input: {input:?}"
            );
        }
    }
}

#[test]
fn parse_json_flat_rejects_invalid_inputs() {
    for input in [
        "null",
        "[\"a\"]",
        "{123: \"val\"}",
        r#"{"key": 123}"#,
        r#"{"key: "val"}"#,
        r#"{"key": "val}"#,
        r#"{"key" "val"}"#,
        "",
        "   ",
    ] {
        let error = parse_json_flat(input).unwrap_err();
        assert_eq!(error.code(), Errc::FormatError, "input: {input:?}");
    }
}

#[test]
fn serialize_json_flat_handles_empty_and_escaped_values() {
    assert_eq!(serialize_json_flat(&HashMap::new()), "{}");

    let mut map = HashMap::new();
    map.insert("k".into(), "hello\"world\nnext".into());
    assert!(serialize_json_flat(&map).contains(r#"hello\"world\nnext"#));
}

#[test]
fn parse_serialize_roundtrip_preserves_entries() {
    for input in [
        r#"{"a": "1"}"#,
        r#"{"a": "1", "b": "2"}"#,
        r#"{"key": "value"}"#,
    ] {
        let parsed = parse_json_flat(input).unwrap();
        let reparsed = parse_json_flat(&serialize_json_flat(&parsed)).unwrap();
        assert_eq!(parsed, reparsed, "input: {input}");
    }
}

#[test]
fn set_get_has_and_remove_share_one_state_contract() {
    let mut settings = SettingsService::new();
    settings.set("theme", "dark");

    assert_eq!(settings.get("theme"), Some("dark"));
    assert!(settings.has("theme"));
    assert!(settings.dirty());

    settings.remove("theme");
    assert!(!settings.has("theme"));
}

#[test]
fn save_load_roundtrip_persists_values() {
    let path = temp_settings_path("roundtrip");
    remove_if_present(&path);
    let path_string = path.to_string_lossy().to_string();

    let mut settings = SettingsService::new();
    settings.load(&path_string).unwrap();
    settings.set("key", "value");
    settings.save().unwrap();

    let mut loaded = SettingsService::new();
    loaded.load(&path_string).unwrap();
    assert_eq!(loaded.get("key"), Some("value"));
    assert!(!loaded.dirty());

    remove_if_present(&path);
}

#[test]
fn load_missing_file_resets_state_and_tracks_path() {
    let path = temp_settings_path("missing");
    remove_if_present(&path);
    let path_string = path.to_string_lossy().to_string();

    let mut settings = SettingsService::new();
    settings.set("stale", "value");
    settings.load(&path_string).unwrap();

    assert_eq!(settings.count(), 0);
    assert_eq!(settings.loaded_path(), Some(path_string.as_str()));
    assert!(!settings.dirty());
}

#[test]
fn load_whitespace_file_resets_state() {
    let path = temp_settings_path("whitespace");
    let path_string = path.to_string_lossy().to_string();
    fs::write(&path, "   \n\t").unwrap();

    let mut settings = SettingsService::new();
    settings.set("stale", "value");
    settings.load(&path_string).unwrap();

    assert_eq!(settings.count(), 0);
    assert_eq!(settings.loaded_path(), Some(path_string.as_str()));
    assert!(!settings.dirty());

    remove_if_present(&path);
}

#[test]
fn save_without_loaded_path_is_noop() {
    let mut settings = SettingsService::new();
    settings.set("theme", "dark");
    settings.save().unwrap();

    assert_eq!(settings.loaded_path(), None);
    assert_eq!(settings.get("theme"), Some("dark"));
    assert!(settings.dirty());
}

#[test]
fn save_uses_two_space_pretty_format() {
    let path = temp_settings_path("pretty");
    remove_if_present(&path);
    let path_string = path.to_string_lossy().to_string();

    let mut settings = SettingsService::new();
    settings.load(&path_string).unwrap();
    settings.set("theme", "dark");
    settings.save().unwrap();

    let saved = fs::read_to_string(&path).unwrap();
    assert!(saved.starts_with("{\n"));
    assert!(saved.ends_with("\n}"));
    assert!(saved
        .lines()
        .filter(|line| line.contains("\": \""))
        .all(|line| line.starts_with("  ") && !line.starts_with("   ")));

    remove_if_present(&path);
}

#[test]
fn save_skips_clean_settings() {
    let path = temp_settings_path("clean");
    let path_string = path.to_string_lossy().to_string();
    fs::write(&path, r#"{"theme": "light"}"#).unwrap();
    let before = fs::metadata(&path).unwrap().modified().unwrap();

    let mut settings = SettingsService::new();
    settings.load(&path_string).unwrap();
    settings.save().unwrap();

    let after = fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(before, after);
    assert!(!settings.dirty());

    remove_if_present(&path);
}
