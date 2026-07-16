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

fn settings_sidecar(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{suffix}", path.to_string_lossy()))
}

fn remove_settings_artifacts(path: &Path) {
    remove_if_present(path);
    remove_if_present(&settings_sidecar(path, SETTINGS_TEMP_SUFFIX));
    remove_if_present(&settings_sidecar(path, SETTINGS_BACKUP_SUFFIX));
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
        (r#"{"ctrl": "\b\f"}"#, &[("ctrl", "\u{0008}\u{000c}")]),
        (r#"{"he\"llo": "world"}"#, &[("he\"llo", "world")]),
        (r#"{"emoji": "\ud83d\ude03"}"#, &[("emoji", "😃")]),
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
        r#"{"a": "1" "b": "2"}"#,
        r#"{"a": "1",}"#,
        r#"{"key": "first", "key": "second"}"#,
        r#"{"key": "first", "\u006bey": "second"}"#,
        r#"{"key": "bad\q"}"#,
        r#"{"key": "bad\u12xz"}"#,
        r#"{"key": "\ud83d"}"#,
        r#"{"key": "\ude03"}"#,
        "{\"key\": \"line\nfeed\"}",
        r#"{"key": "value"} trailing"#,
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
fn serialize_json_flat_orders_keys_deterministically() {
    let mut map = HashMap::new();
    map.insert("zeta".to_string(), "3".to_string());
    map.insert("alpha".to_string(), "1".to_string());
    map.insert("middle".to_string(), "2".to_string());

    assert_eq!(
        serialize_json_flat(&map),
        "{\n  \"alpha\": \"1\",\n  \"middle\": \"2\",\n  \"zeta\": \"3\"\n}"
    );
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
    let settings = SettingsService::new();
    settings.set("theme", "dark");

    assert_eq!(settings.get("theme"), Some("dark".to_string()));
    assert!(settings.has("theme"));
    assert!(settings.dirty());

    settings.remove("theme");
    assert!(!settings.has("theme"));
}

#[test]
fn typed_scalars_roundtrip_without_changing_the_storage_model() {
    let settings = SettingsService::new();

    settings.set_typed("launches", 42u32);
    settings.set_typed("scale", 1.25f64);
    settings.set_typed("enabled", true);
    settings.set_typed("theme", "dark");

    assert_eq!(settings.get("launches"), Some("42".to_string()));
    assert_eq!(settings.get_typed::<u32>("launches").unwrap(), Some(42));
    assert_eq!(settings.get_typed::<f64>("scale").unwrap(), Some(1.25));
    assert_eq!(settings.get_typed::<bool>("enabled").unwrap(), Some(true));
    assert_eq!(
        settings.get_typed::<String>("theme").unwrap(),
        Some("dark".to_string())
    );
}

#[test]
fn typed_reads_distinguish_absent_and_invalid_values() {
    let settings = SettingsService::new();
    settings.set("launches", "many");

    assert_eq!(settings.get_typed::<u32>("missing").unwrap(), None);
    assert_eq!(settings.get_typed_or("missing", 7u32).unwrap(), 7);

    let invalid = settings.get_typed::<u32>("launches").unwrap_err();
    assert_eq!(invalid.code(), Errc::ParseError);
    assert!(invalid.message().contains("launches"));
    assert!(invalid.message().contains("u32"));

    let missing = settings.require_typed::<u32>("missing").unwrap_err();
    assert_eq!(missing.code(), Errc::NotFound);
}

#[cfg(feature = "settings-serde")]
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct TestPreferences {
    theme: String,
    window_width: f64,
    recent_files: Vec<String>,
}

#[cfg(feature = "settings-serde")]
#[test]
fn structured_values_roundtrip_inside_one_flat_string_entry() {
    let settings = SettingsService::new();
    let expected = TestPreferences {
        theme: "dark".to_string(),
        window_width: 1280.0,
        recent_files: vec!["first.uix".to_string(), "second.uix".to_string()],
    };

    settings.set_struct("preferences", &expected).unwrap();

    assert_eq!(settings.count(), 1);
    assert!(settings
        .get("preferences")
        .is_some_and(|encoded| encoded.starts_with('{')));
    assert_eq!(
        settings
            .get_struct::<TestPreferences>("preferences")
            .unwrap(),
        Some(expected)
    );
}

#[cfg(feature = "settings-serde")]
#[test]
fn structured_reads_report_absent_and_invalid_values() {
    let settings = SettingsService::new();
    assert_eq!(
        settings
            .get_struct::<TestPreferences>("preferences")
            .unwrap(),
        None
    );

    let missing = settings
        .require_struct::<TestPreferences>("preferences")
        .unwrap_err();
    assert_eq!(missing.code(), Errc::NotFound);

    settings.set("preferences", "not-json");
    let invalid = settings
        .get_struct::<TestPreferences>("preferences")
        .unwrap_err();
    assert_eq!(invalid.code(), Errc::ParseError);
    assert!(invalid.message().contains("preferences"));
}

#[cfg(feature = "settings-serde")]
#[test]
fn structured_values_survive_explicit_save_and_load() {
    let path = temp_settings_path("structured-roundtrip");
    remove_settings_artifacts(&path);
    let path_string = path.to_string_lossy().to_string();
    let expected = TestPreferences {
        theme: "system".to_string(),
        window_width: 1440.0,
        recent_files: vec!["project.uix".to_string()],
    };

    let settings = SettingsService::new();
    settings.load(&path_string).unwrap();
    settings.set_struct("preferences", &expected).unwrap();
    settings.save().unwrap();

    let loaded = SettingsService::new();
    loaded.load(&path_string).unwrap();
    assert_eq!(
        loaded
            .require_struct::<TestPreferences>("preferences")
            .unwrap(),
        expected
    );

    remove_settings_artifacts(&path);
}

#[cfg(feature = "settings-serde")]
struct RejectSerialization;

#[cfg(feature = "settings-serde")]
impl serde::Serialize for RejectSerialization {
    fn serialize<S>(&self, _serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("rejected by test codec"))
    }
}

#[cfg(feature = "settings-serde")]
#[test]
fn failed_struct_serialization_keeps_the_previous_value() {
    let settings = SettingsService::new();
    settings.set("preferences", "stable");

    let error = settings
        .set_struct("preferences", &RejectSerialization)
        .unwrap_err();

    assert_eq!(error.code(), Errc::SerializationError);
    assert_eq!(settings.get("preferences"), Some("stable".to_string()));
}

#[test]
fn save_load_roundtrip_persists_values() {
    let path = temp_settings_path("roundtrip");
    remove_settings_artifacts(&path);
    let path_string = path.to_string_lossy().to_string();

    let settings = SettingsService::new();
    settings.load(&path_string).unwrap();
    settings.set("key", "value");
    settings.save().unwrap();

    let loaded = SettingsService::new();
    loaded.load(&path_string).unwrap();
    assert_eq!(loaded.get("key"), Some("value".to_string()));
    assert!(!loaded.dirty());

    remove_settings_artifacts(&path);
}

#[test]
fn save_replaces_existing_file_without_leaving_sidecars() {
    let path = temp_settings_path("replace-existing");
    remove_settings_artifacts(&path);
    fs::write(&path, r#"{"theme":"light"}"#).unwrap();
    let path_string = path.to_string_lossy().to_string();

    let settings = SettingsService::new();
    settings.load(&path_string).unwrap();
    settings.set("theme", "dark");
    settings.save().unwrap();

    assert_eq!(settings.get("theme"), Some("dark".to_string()));
    assert!(!settings.dirty());
    assert!(!settings_sidecar(&path, SETTINGS_TEMP_SUFFIX).exists());
    assert!(!settings_sidecar(&path, SETTINGS_BACKUP_SUFFIX).exists());

    let reloaded = SettingsService::new();
    reloaded.load(&path_string).unwrap();
    assert_eq!(reloaded.get("theme"), Some("dark".to_string()));

    remove_settings_artifacts(&path);
}

#[test]
fn load_restores_backup_and_discards_uncommitted_temp() {
    let path = temp_settings_path("restore-interrupted");
    remove_settings_artifacts(&path);
    let backup = settings_sidecar(&path, SETTINGS_BACKUP_SUFFIX);
    let temp = settings_sidecar(&path, SETTINGS_TEMP_SUFFIX);
    fs::write(&backup, r#"{"theme":"light"}"#).unwrap();
    fs::write(&temp, r#"{"theme":"dark"}"#).unwrap();

    let settings = SettingsService::new();
    settings.load(&path.to_string_lossy()).unwrap();

    assert_eq!(settings.get("theme"), Some("light".to_string()));
    assert!(path.exists());
    assert!(!backup.exists());
    assert!(!temp.exists());

    remove_settings_artifacts(&path);
}

#[test]
fn load_keeps_installed_target_and_cleans_stale_sidecars() {
    let path = temp_settings_path("clean-after-install");
    remove_settings_artifacts(&path);
    let backup = settings_sidecar(&path, SETTINGS_BACKUP_SUFFIX);
    let temp = settings_sidecar(&path, SETTINGS_TEMP_SUFFIX);
    fs::write(&path, r#"{"theme":"dark"}"#).unwrap();
    fs::write(&backup, r#"{"theme":"light"}"#).unwrap();
    fs::write(&temp, r#"{"theme":"pending"}"#).unwrap();

    let settings = SettingsService::new();
    settings.load(&path.to_string_lossy()).unwrap();

    assert_eq!(settings.get("theme"), Some("dark".to_string()));
    assert!(!backup.exists());
    assert!(!temp.exists());

    remove_settings_artifacts(&path);
}

#[test]
fn save_rejects_directory_target_without_moving_or_dirtying_disk() {
    let path = temp_settings_path("directory-target");
    remove_settings_artifacts(&path);
    let path_string = path.to_string_lossy().to_string();
    let settings = SettingsService::new();
    settings.load(&path_string).unwrap();
    settings.set("theme", "dark");
    fs::create_dir(&path).unwrap();

    let error = settings.save().unwrap_err();

    assert_eq!(error.code(), Errc::InvalidArgument);
    assert!(path.is_dir());
    assert!(settings.dirty());
    assert!(!settings_sidecar(&path, SETTINGS_TEMP_SUFFIX).exists());
    assert!(!settings_sidecar(&path, SETTINGS_BACKUP_SUFFIX).exists());

    fs::remove_dir(&path).unwrap();
}

#[test]
fn save_rejects_non_file_sidecar_without_touching_committed_target() {
    let path = temp_settings_path("directory-sidecar");
    remove_settings_artifacts(&path);
    let backup = settings_sidecar(&path, SETTINGS_BACKUP_SUFFIX);
    fs::write(&path, r#"{"theme":"light"}"#).unwrap();
    let settings = SettingsService::new();
    settings.load(&path.to_string_lossy()).unwrap();
    settings.set("theme", "dark");
    fs::create_dir(&backup).unwrap();

    let error = settings.save().unwrap_err();

    assert_eq!(error.code(), Errc::InvalidState);
    assert_eq!(fs::read_to_string(&path).unwrap(), r#"{"theme":"light"}"#);
    assert!(settings.dirty());
    assert!(backup.is_dir());
    assert!(!settings_sidecar(&path, SETTINGS_TEMP_SUFFIX).exists());

    fs::remove_dir(&backup).unwrap();
    remove_settings_artifacts(&path);
}

#[test]
fn load_missing_file_resets_state_and_tracks_path() {
    let path = temp_settings_path("missing");
    remove_if_present(&path);
    let path_string = path.to_string_lossy().to_string();

    let settings = SettingsService::new();
    settings.set("stale", "value");
    settings.load(&path_string).unwrap();

    assert_eq!(settings.count(), 0);
    assert_eq!(settings.loaded_path(), Some(path_string));
    assert!(!settings.dirty());
}

#[test]
fn load_rejects_empty_path_without_mutating_state() {
    let settings = SettingsService::new();
    settings.set("theme", "dark");

    for path in ["", "   ", "\t\r\n"] {
        let error = settings.load(path).unwrap_err();
        assert_eq!(error.code(), Errc::InvalidArgument);
    }

    assert_eq!(settings.loaded_path(), None);
    assert_eq!(settings.get("theme"), Some("dark".to_string()));
    assert!(settings.dirty());
}

#[test]
fn load_rejects_duplicate_keys_without_mutating_state() {
    let path = temp_settings_path("duplicate-keys");
    remove_settings_artifacts(&path);
    fs::write(&path, r#"{"theme":"light","theme":"dark"}"#).unwrap();
    let settings = SettingsService::new();
    settings.set("theme", "system");

    let error = settings.load(&path.to_string_lossy()).unwrap_err();

    assert_eq!(error.code(), Errc::FormatError);
    assert_eq!(settings.loaded_path(), None);
    assert_eq!(settings.get("theme"), Some("system".to_string()));
    assert!(settings.dirty());

    remove_settings_artifacts(&path);
}

#[test]
fn load_whitespace_file_resets_state() {
    let path = temp_settings_path("whitespace");
    let path_string = path.to_string_lossy().to_string();
    fs::write(&path, "   \n\t").unwrap();

    let settings = SettingsService::new();
    settings.set("stale", "value");
    settings.load(&path_string).unwrap();

    assert_eq!(settings.count(), 0);
    assert_eq!(settings.loaded_path(), Some(path_string));
    assert!(!settings.dirty());

    remove_if_present(&path);
}

#[test]
fn save_without_loaded_path_returns_typed_error_and_keeps_dirty_state() {
    let settings = SettingsService::new();
    settings.set("theme", "dark");
    let error = settings.save().unwrap_err();

    assert_eq!(error.code(), Errc::InvalidState);
    assert_eq!(settings.loaded_path(), None);
    assert_eq!(settings.get("theme"), Some("dark".to_string()));
    assert!(settings.dirty());
}

#[test]
fn cloned_service_shares_mutations_and_persistence_state() {
    let path = temp_settings_path("shared-clone");
    remove_if_present(&path);
    let path_string = path.to_string_lossy().to_string();

    let settings = SettingsService::new();
    settings.load(&path_string).unwrap();
    let clone = settings.clone();

    clone.set("theme", "dark");
    assert_eq!(settings.get("theme"), Some("dark".to_string()));
    assert!(settings.dirty());

    settings.save().unwrap();
    assert!(!clone.dirty());
    assert!(fs::read_to_string(&path).unwrap().contains("dark"));

    remove_if_present(&path);
}

#[test]
fn no_op_mutations_keep_clean_settings_clean() {
    let path = temp_settings_path("no-op-mutations");
    remove_if_present(&path);
    let path_string = path.to_string_lossy().to_string();

    let settings = SettingsService::new();
    settings.load(&path_string).unwrap();
    settings.set("theme", "dark");
    settings.save().unwrap();

    settings.set("theme", "dark");
    settings.remove("missing");
    assert!(!settings.dirty());

    settings.clear();
    settings.save().unwrap();
    settings.clear();
    assert!(!settings.dirty());

    remove_if_present(&path);
}

#[test]
fn save_uses_two_space_pretty_format() {
    let path = temp_settings_path("pretty");
    remove_if_present(&path);
    let path_string = path.to_string_lossy().to_string();

    let settings = SettingsService::new();
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

    let settings = SettingsService::new();
    settings.load(&path_string).unwrap();
    settings.save().unwrap();

    let after = fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(before, after);
    assert!(!settings.dirty());

    remove_if_present(&path);
}
