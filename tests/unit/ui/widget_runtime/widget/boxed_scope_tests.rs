// 引入生产绘制边界使用的恢复辅助函数。
use super::run_with_unwind_restore;
// 引入间距与字号令牌查询契约。
use crate::ui::theme::traits::{ISpacingTokens, ITypographyTokens};
// 引入真实主题作用域与令牌补丁类型。
use crate::ui::theme::{ScopedThemeTokens, Theme, TokenPatch};
// 引入补丁共享所有权类型。
use std::sync::Arc;

// 确认组件 panic 不会把临时令牌泄漏给后续兄弟节点。
#[test]
// 执行 TokenScope panic 展开回归。
fn panic_during_widget_scope_restores_sibling_tokens() {
    // 创建兄弟节点正常使用的根主题令牌。
    let root = Theme::antd_light().tokens_arc();
    // 记录根主题字号，作为 panic 后的期望值。
    let root_font_size = root.font_size();
    // 记录根主题背景模糊半径，作为 panic 后的期望值。
    let root_backdrop_blur_radius = root.backdrop_blur_radius();
    // 创建与 PaintContext 相同的作用域令牌容器。
    let mut tokens = ScopedThemeTokens::new(root);
    // 安装仅对当前组件生效的临时字号补丁。
    let previous_scope = tokens.replace_scope(
        // 保持根主题不变。
        None,
        // 设置一个与根主题明显不同的组件字号。
        Some(Arc::new(TokenPatch {
            // 使用独立字号辨认临时作用域。
            font_size: Some(31.0),
            // 使用独立背景模糊半径验证新增补丁字段。
            backdrop_blur_radius: Some(19.0),
            // 其余令牌继续继承根主题。
            ..TokenPatch::default() // 结束临时补丁构造。
        })),
        // 保存进入组件前的作用域快照。
    );
    // 在测试外层捕获生产辅助函数继续传播的 panic。
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 复用 BoxedWidget::render 的同一恢复边界。
        run_with_unwind_restore(
            // 传入临时令牌容器。
            &mut tokens,
            // 模拟组件读取补丁后在 render 中 panic。
            |tokens| -> () {
                // 确认 panic 前组件确实看到临时字号。
                assert_eq!(tokens.font_size(), 31.0);
                // 确认 panic 前组件确实看到临时背景模糊半径。
                assert_eq!(tokens.backdrop_blur_radius(), 19.0);
                // 模拟组件绘制失败。
                panic!("widget render panic");
                // 结束模拟组件操作。
            },
            // 使用进入组件前的快照恢复根作用域。
            |tokens| tokens.restore_scope(previous_scope),
            // 结束生产恢复边界调用。
        );
        // 结束外层 panic 捕获。
    }));
    // 确认组件 panic 仍按原语义向上传播。
    assert!(caught.is_err());
    // 确认后续兄弟节点重新读取根主题字号。
    assert_eq!(tokens.font_size(), root_font_size);
    // 确认后续兄弟节点重新读取根主题背景模糊半径。
    assert_eq!(tokens.backdrop_blur_radius(), root_backdrop_blur_radius);
    // 结束 panic 展开回归。
}
