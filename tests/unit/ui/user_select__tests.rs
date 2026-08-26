// 引入被测闭合策略值。
use super::UserSelect;

// 祖先 none 与 all 应支配子树，text 只传播到 auto 后代。
#[test]
fn resolves_parent_selection_boundaries() {
    // none 祖先拒绝后代重新启用 text。
    assert_eq!(
        // 解析显式 text 子节点。
        UserSelect::Text.resolve_with_parent(UserSelect::None),
        // 最终仍为禁止选择。
        UserSelect::None
    );
    // all 祖先拒绝后代缩小为 none。
    assert_eq!(
        // 解析显式 none 子节点。
        UserSelect::None.resolve_with_parent(UserSelect::All),
        // 最终仍属于祖先整体。
        UserSelect::All
    );
    // text 祖先应传播到 auto 子节点。
    assert_eq!(
        // 解析未覆盖子节点。
        UserSelect::Auto.resolve_with_parent(UserSelect::Text),
        // 最终启用普通文本选择。
        UserSelect::Text
    );
    // text 祖先允许后代显式关闭选择。
    assert_eq!(
        // 解析显式 none 子节点。
        UserSelect::None.resolve_with_parent(UserSelect::Text),
        // 最终建立新的禁止边界。
        UserSelect::None
    );
}
