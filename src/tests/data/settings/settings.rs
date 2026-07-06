use super::*;

    // ── parse_json_flat：正常路径 ──────────────────────────────────────

    #[test]
    fn parse_empty_object() {
        let m = parse_json_flat("{}").unwrap();
        assert!(m.is_empty());
    }

    #[test]
    fn parse_multiple_pairs() {
        let m = parse_json_flat(r#"{"a": "1", "b": "2", "c": "3"}"#).unwrap();
        assert_eq!(m.len(), 3);
        assert_eq!(m.get("a").unwrap(), "1");
        assert_eq!(m.get("b").unwrap(), "2");
        assert_eq!(m.get("c").unwrap(), "3");
    }

    #[test]
    fn parse_extra_whitespace() {
        let m = parse_json_flat(r#"{  "key"  :  "val"  }"#).unwrap();
        assert_eq!(m.get("key").unwrap(), "val");
    }

    #[test]
    fn parse_newlines_and_tabs() {
        let input = "{\n  \"key\": \"val\"\n}";
        let m = parse_json_flat(input).unwrap();
        assert_eq!(m.get("key").unwrap(), "val");
    }

    // ── parse_json_flat：转义字符 ──────────────────────────────────────

    #[test]
    fn parse_escaped_quote_in_value() {
        let m = parse_json_flat(r#"{"key": "hello\"world"}"#).unwrap();
        assert_eq!(m.get("key").unwrap(), "hello\"world");
    }

    #[test]
    fn parse_escaped_backslash() {
        let m = parse_json_flat(r#"{"path": "C:\\Users"}"#).unwrap();
        assert_eq!(m.get("path").unwrap(), "C:\\Users");
    }

    #[test]
    fn parse_escaped_newline() {
        let m = parse_json_flat(r#"{"msg": "line1\nline2"}"#).unwrap();
        assert_eq!(m.get("msg").unwrap(), "line1\nline2");
    }

    #[test]
    fn parse_escaped_tab() {
        let m = parse_json_flat(r#"{"col": "a\tb"}"#).unwrap();
        assert_eq!(m.get("col").unwrap(), "a\tb");
    }

    #[test]
    fn parse_escaped_key() {
        let m = parse_json_flat(r#"{"he\"llo": "world"}"#).unwrap();
        assert_eq!(m.get("he\"llo").unwrap(), "world");
    }

    // ── parse_json_flat：错误路径 ──────────────────────────────────────

    #[test]
    fn parse_not_json_object_returns_error() {
        let r = parse_json_flat("null");
        assert!(r.is_err());
        assert_eq!(r.unwrap_err().code(), Errc::FormatError);
    }

    #[test]
    fn parse_array_returns_error() {
        let r = parse_json_flat("[\"a\"]");
        assert!(r.is_err());
    }

    #[test]
    fn parse_invalid_key_not_string() {
        let r = parse_json_flat("{123: \"val\"}");
        assert!(r.is_err());
    }

    #[test]
    fn parse_invalid_value_not_string() {
        let r = parse_json_flat(r#"{"key": 123}"#);
        assert!(r.is_err());
    }

    #[test]
    fn parse_unterminated_key() {
        let r = parse_json_flat(r#"{"key: "val"}"#);
        assert!(r.is_err());
    }

    #[test]
    fn parse_unterminated_value() {
        let r = parse_json_flat(r#"{"key": "val}"#);
        assert!(r.is_err());
    }

    #[test]
    fn parse_missing_colon() {
        let r = parse_json_flat(r#"{"key" "val"}"#);
        assert!(r.is_err());
    }

    #[test]
    fn parse_trailing_comma_is_tolerated() {
        // 多余逗号被当作分隔符跳过，解析器宽容处理
        let m = parse_json_flat(r#"{"a": "1",}"#).unwrap();
        assert_eq!(m.get("a").unwrap(), "1");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn parse_empty_input() {
        let r = parse_json_flat("");
        assert!(r.is_err());
    }

    #[test]
    fn parse_only_whitespace() {
        let r = parse_json_flat("   ");
        assert!(r.is_err());
    }

    // ── serialize_json_flat ────────────────────────────────────────────

    #[test]
    fn serialize_empty() {
        let map = HashMap::new();
        let json = serialize_json_flat(&map);
        assert_eq!(json, "{}");
    }

    #[test]
    fn serialize_escapes_special_chars() {
        let mut map = HashMap::new();
        map.insert("k".into(), "hello\"world\nnext".into());
        let json = serialize_json_flat(&map);
        assert!(json.contains(r#"hello\"world\nnext"#));
    }

    // ── 序列化 ↔ 反序列化 一致性 ───────────────────────────────────────

    #[test]
    fn roundtrip_identity() {
        let cases = vec![
            r#"{"a": "1"}"#,
            r#"{"a": "1", "b": "2"}"#,
            r#"{"key": "value"}"#,
            r#"{"x": "y"}"#,
        ];
        for input in cases {
            let map = parse_json_flat(input).unwrap();
            let json = serialize_json_flat(&map);
            let map2 = parse_json_flat(&json).unwrap();
            assert_eq!(map, map2, "roundtrip failed for: {}", input);
        }
    }

    // ════════════════════════════════════════════════════════════════════
    // SettingsService 原有测试
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_set_get() {
        let mut s = SettingsService::new();
        s.set("theme", "dark");
        assert_eq!(s.get("theme"), Some("dark"));
        assert!(s.dirty());
    }

    #[test]
    fn test_has_remove() {
        let mut s = SettingsService::new();
        s.set("key", "val");
        assert!(s.has("key"));
        s.remove("key");
        assert!(!s.has("key"));
    }

    #[test]
    fn test_save_load_roundtrip() {
        let p = std::env::temp_dir().join("uix_settings_test.json");
        let ps = p.to_string_lossy().to_string();

        {
            let mut s = SettingsService::new();
            s.set("key1", "value1");
            s.set("key2", "value2");
            s.load(&ps).unwrap();
            s.set("key3", "value3");
            s.save().unwrap();
        }

        {
            let mut s2 = SettingsService::new();
            s2.load(&ps).unwrap();
            assert_eq!(s2.get("key3"), Some("value3"));
            assert!(!s2.dirty());
        }

        std::fs::remove_file(&ps).ok();
    }

    #[test]
    fn test_loaded_path_after_load() {
        let p = std::env::temp_dir().join("uix_settings_loaded_path_test.json");
        let ps = p.to_string_lossy().to_string();
        std::fs::remove_file(&ps).ok();

        let mut s = SettingsService::new();
        assert_eq!(s.loaded_path(), None);
        s.load(&ps).unwrap();
        assert_eq!(s.loaded_path(), Some(ps.as_str()));
        assert!(!s.dirty());
    }

    #[test]
    fn test_save_skips_clean_settings() {
        let p = std::env::temp_dir().join("uix_settings_clean_save_test.json");
        let ps = p.to_string_lossy().to_string();
        std::fs::write(&ps, r#"{"theme": "light"}"#).unwrap();
        let before = std::fs::metadata(&ps).unwrap().modified().unwrap();

        let mut s = SettingsService::new();
        s.load(&ps).unwrap();
        s.save().unwrap();

        let after = std::fs::metadata(&ps).unwrap().modified().unwrap();
        assert_eq!(before, after);
        assert!(!s.dirty());
        std::fs::remove_file(&ps).ok();
    }
