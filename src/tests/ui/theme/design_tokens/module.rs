use crate::tests::common::*;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::design_tokens::*;

#[test]
fn neutral_text_inverse_follows_mode() {
    let tokens = DesignTokens::antd_dark();

    assert_eq!(tokens.neutral(NeutralRole::TextInverse), tokens.color_black);
}
