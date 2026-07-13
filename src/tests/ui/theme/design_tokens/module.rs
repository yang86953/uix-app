use crate::ui::theme::design_tokens::*;
use crate::ui::theme::NeutralRole;

#[test]
fn neutral_text_inverse_follows_mode() {
    let tokens = DesignTokens::antd_dark();

    assert_eq!(tokens.neutral(NeutralRole::TextInverse), tokens.color_black);
}
