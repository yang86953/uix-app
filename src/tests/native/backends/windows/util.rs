use super::*;

    #[test]
    fn to_wide_empty_string_returns_null_terminated() {
        assert_eq!(to_wide(""), vec![0u16]);
    }

    #[test]
    fn to_wide_ascii_roundtrip() {
        assert_eq!(to_wide("Hello"), vec![72, 101, 108, 108, 111, 0]);
    }

    #[test]
    fn to_wide_cjk_roundtrip() {
        let result = to_wide("你好");
        assert!(result.len() >= 3);
        assert_eq!(result[0], 0x4F60);
        assert_eq!(result[1], 0x597D);
        assert_eq!(*result.last().unwrap(), 0);
    }

    #[test]
    fn to_utf8_empty_slice() {
        assert_eq!(to_utf8(&[]), "");
    }

    #[test]
    fn to_utf8_only_null_terminator() {
        assert_eq!(to_utf8(&[0u16]), "");
    }

    #[test]
    fn to_utf8_ascii_roundtrip() {
        let input = vec![72, 101, 108, 108, 111];
        assert_eq!(to_utf8(&input), "Hello");
    }

    #[test]
    fn to_utf8_with_null_terminator() {
        let input = vec![72, 101, 108, 108, 111, 0, 88];
        assert_eq!(to_utf8(&input), "Hello");
    }

    #[test]
    fn utf8_wide_roundtrip() {
        let input = "Hello UIX! 你好";
        let wide = to_wide(input);
        let back = to_utf8(&wide);
        assert_eq!(back, input);
    }

    #[test]
    fn to_utf8_cjk() {
        let input = vec![0x4F60, 0x597D];
        assert_eq!(to_utf8(&input), "你好");
    }

    #[test]
    fn family_name_to_filename_known_families() {
        assert_eq!(family_name_to_filename("Microsoft YaHei"), Some("msyh.ttc"));
        assert_eq!(
            family_name_to_filename("Microsoft JhengHei"),
            Some("msjh.ttc")
        );
        assert_eq!(family_name_to_filename("SimSun"), Some("simsun.ttc"));
        assert_eq!(
            family_name_to_filename("Microsoft Sans Serif"),
            Some("micross.ttf")
        );
        assert_eq!(family_name_to_filename("Tahoma"), Some("tahoma.ttf"));
        assert_eq!(family_name_to_filename("Arial"), Some("arial.ttf"));
        assert_eq!(family_name_to_filename("SimFang"), Some("simfang.ttf"));
        assert_eq!(family_name_to_filename("SimKai"), Some("simkai.ttf"));
        assert_eq!(family_name_to_filename("SimHei"), Some("simhei.ttf"));
        assert_eq!(family_name_to_filename("SimLi"), Some("simli.ttf"));
        assert_eq!(family_name_to_filename("SimYou"), Some("simyou.ttf"));
        assert_eq!(family_name_to_filename("MS Gothic"), Some("msgothic.ttc"));
        assert_eq!(family_name_to_filename("MS PGothic"), Some("msgothic.ttc"));
        assert_eq!(
            family_name_to_filename("MS UI Gothic"),
            Some("msgothic.ttc")
        );
    }

    #[test]
    fn family_name_to_filename_case_insensitive() {
        assert_eq!(family_name_to_filename("microsoft yahei"), Some("msyh.ttc"));
        assert_eq!(family_name_to_filename("MICROSOFT YAHEI"), Some("msyh.ttc"));
    }

    #[test]
    fn family_name_to_filename_ui_suffix_stripped() {
        assert_eq!(
            family_name_to_filename("Microsoft YaHei UI"),
            Some("msyh.ttc")
        );
        assert_eq!(
            family_name_to_filename("Microsoft JhengHei UI"),
            Some("msjh.ttc")
        );
    }

    #[test]
    fn family_name_to_filename_unknown_returns_none() {
        assert_eq!(family_name_to_filename("Comic Sans MS"), None);
        assert_eq!(family_name_to_filename(""), None);
        assert_eq!(family_name_to_filename("Noto Sans"), None);
    }

    #[test]
    fn get_last_error_string_when_no_error() {
        let msg = get_last_error_string();
        assert!(msg.contains("no error") || msg.contains("Windows error"));
    }

    #[test]
    fn windows_diag_produces_error() {
        let err = windows_diag(crate::native::Errc::PlatformError, "test context");
        assert!(err.what().contains("test context"));
    }
