use super::*;

#[test]
fn neutral_text_inverse_follows_mode() {
    let tokens = DesignTokens::antd_dark();

    assert_eq!(tokens.neutral(NeutralRole::TextInverse), tokens.color_black);
}
