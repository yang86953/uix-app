use super::*;

    #[test]
    fn neutral_roles_resolve_existing_tokens() {
        let tokens = DesignTokens::antd_light();

        assert_eq!(tokens.neutral(NeutralRole::Text), tokens.color_text);
        assert_eq!(tokens.neutral(NeutralRole::Border), tokens.color_border);
        assert_eq!(
            tokens.neutral(NeutralRole::BgContainer),
            tokens.color_bg_container
        );
        assert_eq!(tokens.neutral(NeutralRole::TextInverse), tokens.color_white);
    }

    #[test]
    fn neutral_text_inverse_follows_mode() {
        let tokens = DesignTokens::antd_dark();

        assert_eq!(tokens.neutral(NeutralRole::TextInverse), tokens.color_black);
    }
